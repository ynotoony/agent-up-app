// Input: 用户选择的 Git 仓库、已识别来源和用户确认的来源路径。
// Output: 只读发现预览，或 .agentup-app/project.json 的最小 App 原生记录。
// Pos: 新项目接入路径；独立于旧 project_init，不导入任务、不调用 Agent；变更同步根 README 与产物登记。

use crate::error::{ApiError, ApiResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 4096;
const MAX_DIRECTORY_DEPTH: usize = 16;
const MAX_DIRECTORY_BYTES: u64 = 32 * 1024 * 1024;
const APP_DIR: &str = ".agentup-app";
const METADATA: &str = ".agentup-app/project.json";
const INDEX_PATHS: [&str; 3] = [
    "facts/requirements/tickets/index.json",
    "docs/issues/index.json",
    "docs/project-management-mvp/issues/index.json",
];
const CONTEXT_PATHS: [(&str, &str, &str); 15] = [
    ("README.md", "document", "context"),
    ("AGENTS.md", "agent_rules", "context"),
    ("CLAUDE.md", "agent_rules", "context"),
    ("CONTEXT.md", "document", "context"),
    ("facts/project/CONTEXT.md", "document", "context"),
    ("rules", "rules", "context"),
    ("facts", "documents", "context"),
    ("docs", "documents", "context"),
    ("facts/requirements/specs", "specifications", "context"),
    ("docs/specs", "specifications", "context"),
    ("specs", "specifications", "context"),
    ("facts/requirements/runs", "run_records", "history"),
    ("docs/runs", "run_records", "history"),
    ("facts/project/archive", "archive", "history"),
    ("docs/archive", "archive", "history"),
];

#[derive(Debug, Clone, Serialize)]
pub struct GitDiscovery {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagementSource {
    pub path: String,
    pub kind: String,
    pub category: String,
    pub item_count: usize,
    pub current_count: usize,
    pub history_count: usize,
    pub unknown_count: usize,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDiscovery {
    pub path: String,
    pub name: String,
    pub git: GitDiscovery,
    pub directories: Vec<String>,
    pub sources: Vec<ManagementSource>,
    pub warnings: Vec<String>,
    pub scanned_at: String,
    pub fingerprint: String,
    pub native_project: Option<ProjectImport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectImport {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub sources: Vec<ManagementSource>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportProjectInput {
    pub path: String,
    pub fingerprint: String,
    pub selected_sources: Vec<String>,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn git(root: &Path, args: &[&str]) -> ApiResult<Output> {
    let mut cmd = Command::new("git");
    // Repository discovery must not update the index or execute a configured fsmonitor.
    cmd.arg("--no-optional-locks")
        .args(["-c", "core.fsmonitor=false", "-C"])
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0");
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
    ] {
        cmd.env_remove(key);
    }
    cmd.output()
        .map_err(|e| ApiError::internal(format!("无法运行 Git: {e}")))
}

fn git_text(root: &Path, args: &[&str]) -> ApiResult<Option<String>> {
    let result = git(root, args)?;
    if !result.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8(result.stdout)
        .map_err(|_| ApiError::bad_request("Git 输出包含无法识别的路径编码"))?;
    let text = text.trim_end_matches(['\n', '\r']).to_owned();
    Ok((!text.is_empty()).then_some(text))
}

fn git_root(path: &str) -> ApiResult<PathBuf> {
    let root = Path::new(path)
        .canonicalize()
        .map_err(|_| ApiError::bad_request("项目目录不存在或无法读取"))?;
    if !root.is_dir() {
        return Err(ApiError::bad_request("项目路径不是目录"));
    }
    if git_text(&root, &["rev-parse", "--is-inside-work-tree"])?.as_deref() != Some("true") {
        return Err(ApiError::bad_request(
            "请选择已有的 Git 工作仓库；此路径不是有效 Git 仓库",
        ));
    }
    let top = git_text(&root, &["rev-parse", "--show-toplevel"])?
        .ok_or_else(|| ApiError::bad_request("无法读取 Git 仓库根目录"))?;
    let top = Path::new(&top).canonicalize()?;
    if root != top {
        return Err(ApiError::bad_request(format!(
            "请选择 Git 仓库根目录：{}",
            top.display()
        )));
    }
    Ok(root)
}

// Every component is checked before reading a known source; a directory symlink must
// not turn a harmless-looking relative source into an external file read/write.
fn source_metadata(root: &Path, relative: &str) -> ApiResult<Option<fs::Metadata>> {
    let mut path = root.to_path_buf();
    let components: Vec<_> = Path::new(relative).components().collect();
    for (i, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err(ApiError::bad_request("来源路径必须是项目内相对路径"));
        };
        path.push(name);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if metadata.file_type().is_symlink() {
            return Err(ApiError::bad_request(format!(
                "拒绝符号链接来源：{relative}"
            )));
        }
        if i + 1 < components.len() && !metadata.is_dir() {
            return Err(ApiError::bad_request(format!(
                "来源父路径不是目录：{relative}"
            )));
        }
        if i + 1 == components.len() {
            return Ok(Some(metadata));
        }
    }
    Err(ApiError::bad_request("来源路径不能为空"))
}

fn bounded_read(root: &Path, relative: &str) -> ApiResult<Vec<u8>> {
    let metadata = source_metadata(root, relative)?
        .ok_or_else(|| ApiError::bad_request(format!("来源已消失：{relative}")))?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "来源不是普通文件：{relative}"
        )));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(ApiError::bad_request(format!(
            "来源超过 2 MiB 扫描上限：{relative}"
        )));
    }
    let mut bytes = Vec::new();
    File::open(root.join(relative))?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ApiError::bad_request(format!(
            "来源超过 2 MiB 扫描上限：{relative}"
        )));
    }
    Ok(bytes)
}

fn visible_directory(name: &str) -> bool {
    !name.starts_with('.')
        && !matches!(
            name,
            "node_modules" | "target" | "dist" | "build" | "vendor" | "coverage" | "out"
        )
}

fn directory_inventory(root: &Path, relative: &str) -> ApiResult<Vec<String>> {
    // Recursively fingerprint known project directories. File bodies are only read
    // for hashing (never returned); symlinks are represented and never followed.
    fn walk(
        path: &Path,
        prefix: &str,
        depth: usize,
        entries: &mut Vec<String>,
        bytes_read: &mut u64,
    ) -> ApiResult<()> {
        if depth > MAX_DIRECTORY_DEPTH {
            return Err(ApiError::bad_request(format!(
                "目录超过 {MAX_DIRECTORY_DEPTH} 层扫描上限：{prefix}"
            )));
        }
        let mut children = Vec::new();
        for entry in fs::read_dir(path)? {
            children.push(entry?);
        }
        children.sort_by_key(|entry| entry.file_name());
        for entry in children {
            if entries.len() >= MAX_DIRECTORY_ENTRIES {
                return Err(ApiError::bad_request(format!(
                    "目录超过 {MAX_DIRECTORY_ENTRIES} 项扫描上限：{prefix}"
                )));
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let child_prefix = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let metadata = fs::symlink_metadata(entry.path())?;
            let kind = metadata.file_type();
            if kind.is_symlink() {
                let target = fs::read_link(entry.path()).unwrap_or_default();
                entries.push(format!(
                    "{child_prefix}:link:{}:{}",
                    metadata.len(),
                    hash(target.to_string_lossy().as_bytes())
                ));
            } else if metadata.is_dir() {
                entries.push(format!("{child_prefix}:directory"));
                walk(&entry.path(), &child_prefix, depth + 1, entries, bytes_read)?;
            } else if metadata.is_file() {
                if metadata.len() > MAX_FILE_BYTES
                    || bytes_read.saturating_add(metadata.len()) > MAX_DIRECTORY_BYTES
                {
                    return Err(ApiError::bad_request(format!(
                        "目录文件超过扫描上限：{child_prefix}"
                    )));
                }
                let bytes = fs::read(entry.path())?;
                *bytes_read = bytes_read.saturating_add(bytes.len() as u64);
                entries.push(format!(
                    "{child_prefix}:file:{}:{}",
                    metadata.len(),
                    hash(&bytes)
                ));
            } else {
                entries.push(format!("{child_prefix}:other:{}", metadata.len()));
            }
        }
        Ok(())
    }
    let mut entries = Vec::new();
    let mut bytes_read = 0;
    walk(&root.join(relative), "", 0, &mut entries, &mut bytes_read)?;
    Ok(entries)
}

fn valid_source(source: &ManagementSource) -> bool {
    let known = INDEX_PATHS.contains(&source.path.as_str())
        || CONTEXT_PATHS.iter().any(|(p, _, _)| *p == source.path);
    let counts = source
        .current_count
        .checked_add(source.history_count)
        .and_then(|v| v.checked_add(source.unknown_count));
    known
        && matches!(
            source.category.as_str(),
            "current" | "history" | "context" | "mixed" | "unknown"
        )
        && !source.kind.is_empty()
        && source.fingerprint.len() == 64
        && source.fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
        && counts.is_some_and(|v| v <= source.item_count)
}

fn native_project(root: &Path) -> ApiResult<Option<ProjectImport>> {
    if let Some(metadata) = source_metadata(root, APP_DIR)? {
        if !metadata.is_dir() {
            return Err(ApiError::bad_request(".agentup-app 已存在但不是目录"));
        }
    }
    if source_metadata(root, METADATA)?.is_none() {
        return Ok(None);
    }
    let record: ProjectImport =
        serde_json::from_slice(&bounded_read(root, METADATA)?).map_err(|_| {
            ApiError::bad_request("已有 .agentup-app/project.json 无法识别，不会覆盖；请检查原记录")
        })?;
    let mut paths = BTreeSet::new();
    if record.schema_version != 1
        || uuid::Uuid::parse_str(&record.id).is_err()
        || record.name.trim().is_empty()
        || record.name.len() > 1024
        || chrono::DateTime::parse_from_rfc3339(&record.created_at).is_err()
        || record.sources.len() > INDEX_PATHS.len() + CONTEXT_PATHS.len()
        || record
            .sources
            .iter()
            .any(|s| !valid_source(s) || !paths.insert(&s.path))
    {
        return Err(ApiError::bad_request(
            "已有 App 项目记录的版本或内容不受支持，不会覆盖",
        ));
    }
    Ok(Some(record))
}

fn ticket_source(
    root: &Path,
    relative: &str,
    warnings: &mut Vec<String>,
) -> ApiResult<Option<ManagementSource>> {
    if source_metadata(root, relative)?.is_none() {
        return Ok(None);
    }
    let bytes = match bounded_read(root, relative) {
        Ok(bytes) => bytes,
        Err(error) => {
            warnings.push(error.message);
            return Ok(None);
        }
    };
    let mut source = ManagementSource {
        path: relative.into(),
        kind: "ticket_index".into(),
        category: "unknown".into(),
        item_count: 0,
        current_count: 0,
        history_count: 0,
        unknown_count: 0,
        fingerprint: hash(&bytes),
    };
    let value = serde_json::from_slice::<serde_json::Value>(&bytes);
    let rows = value.as_ref().ok().and_then(|v| {
        v.as_array()
            .or_else(|| v.get("issues").and_then(|v| v.as_array()))
    });
    let Some(rows) = rows else {
        warnings.push(format!(
            "{relative} 格式无法识别；任务数量未知，未按空列表处理"
        ));
        return Ok(Some(source));
    };
    source.item_count = rows.len();
    for row in rows {
        let id = row
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty());
        let status = row.get("status").and_then(|v| v.as_str());
        match (id, status) {
            (
                Some(_),
                Some(
                    "ready" | "ready-for-agent" | "in_progress" | "blocked" | "review_ready"
                    | "review_pass" | "review_fail" | "pending",
                ),
            ) => source.current_count += 1,
            (Some(_), Some("done" | "superseded")) => source.history_count += 1,
            _ => source.unknown_count += 1,
        }
    }
    source.category = match (
        source.current_count > 0,
        source.history_count > 0,
        source.unknown_count > 0,
    ) {
        (true, false, false) => "current",
        (false, true, false) => "history",
        (false, false, _) => "unknown",
        _ => "mixed",
    }
    .into();
    if source.unknown_count > 0 {
        warnings.push(format!(
            "{relative} 有 {} 条记录的标识或状态未知，不会猜测其任务状态",
            source.unknown_count
        ));
    }
    Ok(Some(source))
}

pub fn discover(path: &str) -> ApiResult<ProjectDiscovery> {
    let root = git_root(path)?;
    let native_project = native_project(&root)?;
    let mut directories = Vec::new();
    for (count, entry) in fs::read_dir(&root)?.enumerate() {
        if count >= MAX_DIRECTORY_ENTRIES {
            return Err(ApiError::bad_request(
                "项目根目录过大，请减少顶层文件后重新扫描",
            ));
        }
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if visible_directory(&name) {
                directories.push(name);
            }
        }
    }
    directories.sort();
    let mut warnings = Vec::new();
    let mut sources = Vec::new();
    for relative in INDEX_PATHS {
        if let Some(source) = ticket_source(&root, relative, &mut warnings)? {
            sources.push(source);
        }
    }
    for (relative, kind, category) in CONTEXT_PATHS {
        let Some(metadata) = source_metadata(&root, relative)? else {
            continue;
        };
        let bytes = if metadata.is_dir() {
            match directory_inventory(&root, relative) {
                Ok(inventory) => serde_json::to_vec(&inventory)?,
                Err(error) => {
                    warnings.push(error.message);
                    continue;
                }
            }
        } else {
            match bounded_read(&root, relative) {
                Ok(bytes) => bytes,
                Err(error) => {
                    warnings.push(error.message);
                    continue;
                }
            }
        };
        sources.push(ManagementSource {
            path: relative.into(),
            kind: kind.into(),
            category: category.into(),
            item_count: 1,
            current_count: 0,
            history_count: 0,
            unknown_count: 0,
            fingerprint: hash(&bytes),
        });
    }
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    let branch = git_text(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    let head = git_text(&root, &["rev-parse", "--verify", "HEAD"])?;
    let status = git(
        &root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=all",
            "--",
            ".",
            ":(exclude).agentup-app",
            ":(exclude).agentup-app/**",
        ],
    )?;
    if !status.status.success() {
        return Err(ApiError::bad_request("无法读取 Git 工作区状态"));
    }
    let name = root
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::bad_request("项目目录名无法识别"))?
        .to_owned();
    let path = root
        .to_str()
        .ok_or_else(|| ApiError::bad_request("项目路径编码不受支持"))?
        .to_owned();
    let fingerprint = hash(&serde_json::to_vec(&(
        &path,
        &branch,
        &head,
        &status.stdout,
        &directories,
        &sources,
        &warnings,
    ))?);
    Ok(ProjectDiscovery {
        path,
        name,
        git: GitDiscovery {
            branch,
            head,
            dirty: !status.stdout.is_empty(),
        },
        directories,
        sources,
        warnings,
        scanned_at: Utc::now().to_rfc3339(),
        fingerprint,
        native_project,
    })
}

/// Confirmation deliberately accepts no client-provided metadata, counts, or status.
/// Re-read first, then write exclusively in the App-owned directory.
pub fn confirm(input: &ImportProjectInput) -> ApiResult<ProjectImport> {
    let fresh = discover(&input.path)?;
    if input.fingerprint != fresh.fingerprint {
        return Err(ApiError::conflict("项目在预览后发生变化，请重新扫描并确认"));
    }
    let requested: BTreeSet<_> = input.selected_sources.iter().collect();
    if requested.len() != input.selected_sources.len()
        || requested
            .iter()
            .any(|path| !fresh.sources.iter().any(|source| &source.path == *path))
    {
        return Err(ApiError::bad_request("确认的来源不属于此次扫描预览"));
    }
    if let Some(existing) = fresh.native_project {
        return Ok(existing);
    }
    let root = Path::new(&fresh.path);
    let record = ProjectImport {
        schema_version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        name: fresh.name,
        created_at: Utc::now().to_rfc3339(),
        sources: fresh
            .sources
            .into_iter()
            .filter(|s| requested.contains(&s.path))
            .collect(),
    };
    let mut bytes = serde_json::to_vec_pretty(&record)?;
    bytes.push(b'\n');
    match fs::create_dir(root.join(APP_DIR)) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = source_metadata(root, APP_DIR)?
                .ok_or_else(|| ApiError::conflict("项目目录已变化，请重新扫描"))?;
            if !metadata.is_dir() {
                return Err(ApiError::bad_request(".agentup-app 不是目录，不会覆盖"));
            }
        }
        Err(error) => return Err(error.into()),
    }
    if let Some(existing) = native_project(root)? {
        return Ok(existing);
    }
    let final_path = root.join(METADATA);
    if source_metadata(root, METADATA)?.is_some() {
        return native_project(root)?
            .ok_or_else(|| ApiError::conflict("项目记录已变化，请重新扫描"));
    }
    // Publish via a same-directory, create_new temporary file. hard_link is the
    // portable no-replace atomic publish primitive available here: unlike rename,
    // it cannot overwrite a concurrently-created final record. The temp file is
    // removed on every failure path, so a partial write is never presented as the
    // canonical project.json.
    let temp_path = root
        .join(APP_DIR)
        .join(format!(".project.json.tmp-{}", uuid::Uuid::new_v4()));
    let publish = (|| -> ApiResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        match fs::hard_link(&temp_path, &final_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(ApiError::conflict("项目记录已变化，请重新扫描"))
            }
            Err(error) => Err(error.into()),
        }
    })();
    let _ = fs::remove_file(&temp_path);
    publish?;
    Ok(record)
}
