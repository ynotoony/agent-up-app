// Input: Bound project directory, governance index, JSON ticket bodies and explicit ticket drafts.
// Output: Ticket cards and details; new drafts publish through the pinned project-root boundary.
// Pos: Governance ticket filesystem boundary for safe reads and atomic ticket creation.

use crate::{
    db,
    error::{ApiError, ApiResult},
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
#[cfg(not(unix))]
use std::fs;
#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(unix)]
use std::os::unix::{
    ffi::OsStrExt,
    fs::{MetadataExt, OpenOptionsExt},
};
use std::sync::{Mutex, OnceLock};
use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};

const INDEX_PATHS: [&str; 3] = [
    "facts/requirements/tickets/index.json",
    "docs/issues/index.json",
    "docs/project-management-mvp/issues/index.json",
];
const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024;
const MAX_BODY_BYTES: u64 = 512 * 1024;
const MAX_TOTAL_BODY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TICKETS: usize = 4000;
const TICKETS_README_PATH: &str = "facts/requirements/tickets/README.md";
const TICKETS_PROGRESS_PATH: &str = "facts/requirements/tickets/progress-current.md";
const CHANGES_LEDGER_PATH: &str = "facts/project/changes.jsonl";

#[derive(Clone)]
struct BodySummary {
    title: Option<String>,
    goal: Option<String>,
    blocked_by: Option<Value>,
    complete: bool,
}
type CachedBody = Result<BodySummary, String>;
static BODY_CACHE: OnceLock<Mutex<HashMap<String, ((u64, u64, u64, u128, u128), CachedBody)>>> =
    OnceLock::new();

fn file_identity(metadata: &std::fs::Metadata) -> (u64, u64) {
    #[cfg(unix)]
    {
        (metadata.dev(), metadata.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        (0, 0)
    }
}

fn file_change_time(metadata: &std::fs::Metadata) -> u128 {
    #[cfg(unix)]
    {
        (metadata.ctime() as i128 * 1_000_000_000 + metadata.ctime_nsec() as i128) as u128
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TicketDraft {
    pub title: String,
    pub complexity: String,
    pub profile: String,
    pub complexity_reason: String,
    pub priority_reason: String,
    pub goal: String,
    pub scope: Vec<String>,
    pub acceptance: Vec<String>,
    pub verification: String,
    #[serde(default)]
    pub blocked_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TicketDraftResult {
    pub draft: TicketDraft,
    pub runtime_id: String,
}

fn lane_for_status(status: &str) -> &'static str {
    match status {
        "ready" => "ready",
        "in_progress" => "in_progress",
        "review_ready" | "review_pass" | "review_fail" | "blocked" => "blocked",
        "done" => "done",
        "superseded" => "superseded",
        _ => "unknown",
    }
}
pub fn valid_ticket_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('-')
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

pub fn valid_formal_ticket_id(id: &str) -> bool {
    let Some((number, slug)) = id.split_once('-') else {
        return false;
    };
    number.len() >= 2
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && !slug.is_empty()
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

impl TicketDraft {
    pub fn validate(&self) -> ApiResult<()> {
        check_text(&self.title, 2, 300, "任务标题")?;
        if !matches!(self.complexity.as_str(), "C0" | "C1" | "C2" | "C3") {
            return Err(ApiError::bad_request("任务复杂度必须为 C0、C1、C2 或 C3"));
        }
        validate_profile(&self.profile)?;
        check_text(&self.complexity_reason, 1, 2000, "复杂度理由")?;
        check_text(&self.priority_reason, 1, 2000, "优先级理由")?;
        check_text(&self.goal, 4, 32 * 1024, "任务目标")?;
        check_strings(&self.scope, 100, 2000, "任务范围")?;
        check_strings(&self.acceptance, 100, 4000, "验收条件")?;
        check_text(&self.verification, 1, 4000, "验证口径")?;
        if self.blocked_by.len() > MAX_TICKETS {
            return Err(ApiError::bad_request("任务依赖数量超过限制"));
        }
        let mut seen = HashSet::new();
        for id in &self.blocked_by {
            if !valid_formal_ticket_id(id) || !seen.insert(id) {
                return Err(ApiError::bad_request("任务依赖包含无效或重复任务 ID"));
            }
        }
        Ok(())
    }
}

fn check_text(value: &str, minimum: usize, maximum: usize, label: &str) -> ApiResult<()> {
    let length = value.trim().chars().count();
    if length < minimum || length > maximum {
        return Err(ApiError::bad_request(format!(
            "{label}须为 {minimum} 到 {maximum} 个字符"
        )));
    }
    Ok(())
}

fn check_strings(
    values: &[String],
    maximum: usize,
    item_maximum: usize,
    label: &str,
) -> ApiResult<()> {
    if values.is_empty() || values.len() > maximum {
        return Err(ApiError::bad_request(format!(
            "{label}需包含 1 到 {maximum} 条内容"
        )));
    }
    for value in values {
        check_text(value, 1, item_maximum, label)?;
    }
    Ok(())
}

fn validate_profile(profile: &str) -> ApiResult<()> {
    let dimensions = ["D", "B", "I", "U", "S", "M", "O"];
    let allowed = ["Required", "Conditional", "Advisory", "N/A"];
    let fields: Vec<_> = profile.split(',').collect();
    if fields.len() != dimensions.len()
        || fields.iter().zip(dimensions).any(|(field, dimension)| {
            let Some((actual_dimension, value)) = field.split_once(':') else {
                return true;
            };
            actual_dimension != dimension || !allowed.contains(&value)
        })
    {
        return Err(ApiError::bad_request(
            "Profile 必须按 D、B、I、U、S、M、O 顺序使用受控枚举",
        ));
    }
    Ok(())
}

pub fn parse_ticket_draft(text: &str) -> ApiResult<TicketDraft> {
    if text.len() > 64 * 1024 {
        return Err(ApiError::bad_request("任务草案超过大小限制"));
    }
    let draft: TicketDraft = serde_json::from_str(text.trim()).map_err(|error| {
        ApiError::bad_request(format!("Agent 未返回有效任务草案 JSON：{error}"))
    })?;
    draft.validate()?;
    Ok(draft)
}

struct ProjectRoot {
    path: PathBuf,
    #[cfg(unix)]
    directory: File,
}

impl ProjectRoot {
    fn open(path: PathBuf) -> ApiResult<Self> {
        #[cfg(unix)]
        {
            let path = path.canonicalize()?;
            let directory = open_absolute_directory(&path)?;
            Ok(Self { path, directory })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(ApiError::bad_request(
                "当前平台无法安全打开治理票目录；为避免路径竞态，已拒绝读取",
            ))
        }
    }

    fn open_file(&self, relative: &str) -> ApiResult<Option<File>> {
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ApiError::bad_request("任务票路径不在项目内"));
        }
        #[cfg(unix)]
        {
            let names: Vec<CString> = path
                .components()
                .filter_map(|component| match component {
                    Component::Normal(name) => Some(
                        CString::new(name.as_bytes())
                            .map_err(|_| ApiError::bad_request("任务票路径包含 NUL 字符")),
                    ),
                    _ => None,
                })
                .collect::<ApiResult<_>>()?;
            if names.is_empty() {
                return Err(ApiError::bad_request("任务票路径不能为空"));
            }
            let mut parent = self.directory.try_clone()?;
            for name in &names[..names.len() - 1] {
                let fd = unsafe {
                    libc::openat(
                        parent.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() == std::io::ErrorKind::NotFound {
                        return Ok(None);
                    }
                    return Err(path_io_error(relative, error));
                }
                parent = unsafe { File::from_raw_fd(fd) };
            }
            let fd = unsafe {
                libc::openat(
                    parent.as_raw_fd(),
                    names.last().unwrap().as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                )
            };
            if fd < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::NotFound {
                    return Ok(None);
                }
                return Err(path_io_error(relative, error));
            }
            let file = unsafe { File::from_raw_fd(fd) };
            if !file.metadata()?.is_file() {
                return Err(ApiError::bad_request(format!(
                    "任务票来源不是普通文件：{relative}"
                )));
            }
            Ok(Some(file))
        }
        #[cfg(not(unix))]
        {
            let mut absolute = self.path.clone();
            for component in path.components() {
                absolute.push(component);
                match fs::symlink_metadata(&absolute) {
                    Ok(info) if info.file_type().is_symlink() => {
                        return Err(ApiError::bad_request(format!(
                            "任务票路径包含符号链接：{relative}"
                        )))
                    }
                    Ok(_) => (),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => return Err(path_io_error(relative, error)),
                }
            }
            if !absolute.is_file() || !fs::canonicalize(&absolute)?.starts_with(&self.path) {
                return Err(ApiError::bad_request(format!(
                    "任务票来源不是项目内普通文件：{relative}"
                )));
            }
            Ok(Some(File::open(absolute)?))
        }
    }

    fn open_directory(&self, relative: &str) -> ApiResult<File> {
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ApiError::bad_request("任务票目录不在项目内"));
        }
        #[cfg(unix)]
        {
            let names: Vec<CString> = path
                .components()
                .filter_map(|component| match component {
                    Component::Normal(name) => Some(
                        CString::new(name.as_bytes())
                            .map_err(|_| ApiError::bad_request("任务票目录包含 NUL 字符")),
                    ),
                    _ => None,
                })
                .collect::<ApiResult<_>>()?;
            if names.is_empty() {
                return Err(ApiError::bad_request("任务票目录不能为空"));
            }
            let mut parent = self.directory.try_clone()?;
            for name in names {
                let fd = unsafe {
                    libc::openat(
                        parent.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(path_io_error(relative, std::io::Error::last_os_error()));
                }
                parent = unsafe { File::from_raw_fd(fd) };
            }
            Ok(parent)
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(ApiError::bad_request(
                "当前平台无法安全打开任务票目录；为避免路径竞态，已拒绝写入",
            ))
        }
    }
}

#[cfg(unix)]
fn open_absolute_directory(path: &Path) -> ApiResult<File> {
    if !path.is_absolute() {
        return Err(ApiError::bad_request("项目目录路径无效"));
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let mut directory = options.open("/")?;
    for component in path.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| ApiError::bad_request("项目目录路径包含 NUL 字符"))?;
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(ApiError::bad_request(format!(
                "项目目录路径在打开期间发生变化：{}",
                std::io::Error::last_os_error()
            )));
        }
        directory = unsafe { File::from_raw_fd(fd) };
    }
    Ok(directory)
}

fn path_io_error(relative: &str, error: std::io::Error) -> ApiError {
    #[cfg(unix)]
    if matches!(
        error.raw_os_error(),
        Some(libc::ELOOP) | Some(libc::ENOTDIR)
    ) {
        return ApiError::bad_request(format!("任务票路径包含符号链接或非目录组件：{relative}"));
    }
    ApiError::bad_request(format!("无法读取任务票路径 {relative}：{error}"))
}

fn read_object(
    root: &ProjectRoot,
    relative: &str,
    limit: u64,
    budget: &mut u64,
) -> ApiResult<Value> {
    let mut file = root
        .open_file(relative)?
        .ok_or_else(|| ApiError::not_found(format!("任务票文件不存在：{relative}")))?;
    let size = file.metadata()?.len();
    if size > limit || size > *budget {
        return Err(ApiError::bad_request(format!(
            "任务票来源超过读取大小限制：{relative}"
        )));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit.min(*budget) + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit || bytes.len() as u64 > *budget {
        return Err(ApiError::bad_request(format!(
            "任务票来源超过读取大小限制：{relative}"
        )));
    }
    *budget -= bytes.len() as u64;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::bad_request(format!("任务票不是有效 JSON：{relative}")))?;
    if !value.is_object() {
        return Err(ApiError::bad_request(format!(
            "任务票须为 JSON 对象：{relative}"
        )));
    }
    Ok(value)
}

fn read_ticket_body_summary_cached(
    root: &ProjectRoot,
    relative: &str,
    ticket_id: &str,
    budget: &mut u64,
) -> CachedBody {
    let mut file = root
        .open_file(relative)
        .map_err(|error| error.message)?
        .ok_or_else(|| format!("任务票文件不存在：{relative}"))?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err(format!("任务票来源不是普通文件：{relative}"));
    }
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos());
    let (device, inode) = file_identity(&metadata);
    let fingerprint = (
        device,
        inode,
        metadata.len(),
        modified,
        file_change_time(&metadata),
    );
    let key = format!("{}:{relative}", root.path.display());
    let cache = BODY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(result) = cache
        .lock()
        .map_err(|_| "任务票摘要缓存不可用".to_owned())?
        .get(&key)
        .filter(|(cached_fingerprint, _)| *cached_fingerprint == fingerprint)
        .map(|(_, result)| result.clone())
    {
        return result;
    }

    let result = (|| {
        if metadata.len() > MAX_BODY_BYTES || metadata.len() > *budget {
            return Err(format!("任务票来源超过读取大小限制：{relative}"));
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take(MAX_BODY_BYTES.min(*budget) + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_BODY_BYTES || bytes.len() as u64 > *budget {
            return Err(format!("任务票来源超过读取大小限制：{relative}"));
        }
        *budget -= bytes.len() as u64;
        let body: Value = serde_json::from_slice(&bytes)
            .map_err(|_| format!("任务票不是有效 JSON：{relative}"))?;
        if !body.is_object() {
            return Err(format!("任务票须为 JSON 对象：{relative}"));
        }
        if body
            .get("id")
            .is_some_and(|body_id| body_id.as_str() != Some(ticket_id))
        {
            return Err("票体 ID 与索引不一致".into());
        }
        Ok(summarize_body(&body))
    })();
    cache
        .lock()
        .map_err(|_| "任务票摘要缓存不可用".to_owned())?
        .insert(key, (fingerprint, result.clone()));
    result
}

fn summarize_body(body: &Value) -> BodySummary {
    BodySummary {
        title: body
            .get("title")
            .and_then(Value::as_str)
            .filter(|title| !title.trim().is_empty())
            .map(|title| title.chars().take(300).collect()),
        goal: body
            .get("goal")
            .and_then(Value::as_str)
            .map(|goal| goal.chars().take(300).collect()),
        blocked_by: body.get("blocked_by").cloned(),
        complete: body
            .get("goal")
            .and_then(Value::as_str)
            .is_some_and(|goal| !goal.trim().is_empty())
            && body
                .get("acceptance")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    !items.is_empty()
                        && items
                            .iter()
                            .all(|item| item.as_str().is_some_and(|value| !value.trim().is_empty()))
                }),
    }
}

fn dependencies(value: Option<&Value>) -> Result<Vec<String>, &'static str> {
    let Some(value) = value else {
        return Ok(vec![]);
    };
    let array = value.as_array().ok_or("blocked_by 必须是任务 ID 数组")?;
    if array.len() > MAX_TICKETS {
        return Err("任务依赖数量超过限制");
    }
    let mut seen = HashSet::new();
    array
        .iter()
        .map(|value| {
            let id = value
                .as_str()
                .filter(|id| valid_ticket_id(id))
                .ok_or("依赖包含无效任务 ID")?;
            if !seen.insert(id) {
                return Err("依赖包含重复任务 ID");
            }
            Ok(id.to_owned())
        })
        .collect()
}

pub fn load_from_root(root: &Path, project_id: &str, load_bodies: bool) -> ApiResult<Value> {
    let root = root
        .canonicalize()
        .map_err(|_| ApiError::not_found("项目目录不存在或无法访问"))?;
    if !root.is_dir() {
        return Err(ApiError::not_found("项目目录不存在"));
    }
    let root = ProjectRoot::open(root)?;
    let mut selected = None;
    for path in INDEX_PATHS {
        if root.open_file(path)?.is_some() {
            selected = Some(path);
            break;
        }
    }
    let index_path = selected.ok_or_else(|| {
        ApiError::not_found(format!("未找到治理票索引（{}）", INDEX_PATHS.join("、")))
    })?;
    let mut index_budget = MAX_INDEX_BYTES;
    let index = read_object(&root, index_path, MAX_INDEX_BYTES, &mut index_budget)?;
    let entries = index
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad_request("治理票索引缺少 issues 数组"))?;
    if entries.len() > MAX_TICKETS {
        return Err(ApiError::bad_request(
            "治理票索引超过 4000 条，请归档历史票",
        ));
    }
    let mut statuses = HashMap::new();
    let mut skipped = 0;
    for entry in entries {
        let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        let status = entry
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !valid_ticket_id(id) || status.is_empty() || status.len() > 100 {
            skipped += 1;
            continue;
        }
        if statuses.insert(id, status).is_some() {
            return Err(ApiError::bad_request(format!(
                "治理票索引包含重复 ID：{id}"
            )));
        }
    }
    let issues_dir = index_path.strip_suffix("/index.json").unwrap();
    let mut body_budget = MAX_TOTAL_BODY_BYTES;
    let mut cards = Vec::new();
    let mut unknown_statuses = Map::new();
    for entry in entries {
        let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        let Some(&status) = statuses.get(id) else {
            continue;
        };
        if entry.get("status").and_then(Value::as_str).map(str::trim) != Some(status) {
            continue;
        }
        let lane = lane_for_status(status);
        if lane == "unknown" {
            unknown_statuses.insert(id.to_owned(), json!(status));
        }
        let source_path = format!("{issues_dir}/{id}.json");
        let (body, summary, body_error) = if load_bodies {
            match read_object(&root, &source_path, MAX_BODY_BYTES, &mut body_budget).and_then(
                |body| {
                    if body
                        .get("id")
                        .is_some_and(|body_id| body_id.as_str() != Some(id))
                    {
                        return Err(ApiError::bad_request("票体 ID 与索引不一致"));
                    }
                    Ok(body)
                },
            ) {
                Ok(body) => {
                    let summary = summarize_body(&body);
                    (Some(body), Some(summary), None)
                }
                Err(error) => (None, None, Some(error.message)),
            }
        } else {
            match read_ticket_body_summary_cached(&root, &source_path, id, &mut body_budget) {
                Ok(summary) => (None, Some(summary), None),
                Err(error) => (None, None, Some(error)),
            }
        };
        let title = entry
            .get("title")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .or_else(|| summary.as_ref().and_then(|body| body.title.as_deref()))
            .unwrap_or(id);
        let title: String = title.chars().take(300).collect();
        let goal_summary = summary.as_ref().and_then(|body| body.goal.clone());
        let dependency_result = dependencies(
            entry
                .get("blocked_by")
                .or_else(|| summary.as_ref().and_then(|b| b.blocked_by.as_ref())),
        );
        let (blocked_by, dependency_error) = match dependency_result {
            Ok(ids) => (ids, None),
            Err(message) => (vec![], Some(message.to_owned())),
        };
        let unresolved: Vec<_> = blocked_by
            .iter()
            .filter(|dependency| statuses.get(dependency.as_str()) != Some(&"done"))
            .cloned()
            .collect();
        let incomplete = summary.as_ref().is_none_or(|summary| !summary.complete);
        let readiness_reason = body_error.clone().or(dependency_error).or_else(|| {
            if incomplete {
                Some("票体缺少目标或验收条件".into())
            } else if status != "ready" {
                Some(format!("当前状态 {status} 不允许启动；仅 ready 任务可交接"))
            } else if !unresolved.is_empty() {
                Some(format!("依赖尚未完成或不存在：{}", unresolved.join("、")))
            } else {
                None
            }
        });
        let mut card = json!({
            "id": id, "title": title, "lane": lane, "governance_status": status,
            "blocked_by": blocked_by, "unresolved_dependencies": unresolved,
            "updated_at": entry.get("updated_at").and_then(Value::as_str),
            "complexity": entry.get("complexity").and_then(Value::as_str),
            "source": "governance", "source_path": source_path,
            "goal_summary": goal_summary, "body_error": body_error, "body_loaded": load_bodies, "ready": readiness_reason.is_none(), "readiness_reason": readiness_reason,
        });
        if load_bodies {
            if let Some(body) = body {
                card["body"] = body;
            }
        }
        cards.push(card);
    }
    cards.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Ok(
        json!({"project_id": project_id, "index_path": index_path, "tickets": cards, "total": cards.len(), "skipped": skipped, "unknown_statuses": unknown_statuses}),
    )
}

/// Load one selected ticket body for the inspector. The index is re-read to bind
/// the requested id to the currently selected source; unrelated bodies are untouched.
pub fn load_ticket_detail_from_root(
    root: &Path,
    project_id: &str,
    ticket_id: &str,
) -> ApiResult<Value> {
    if !valid_ticket_id(ticket_id) {
        return Err(ApiError::bad_request("任务票 ID 无效"));
    }
    let path = root
        .canonicalize()
        .map_err(|_| ApiError::not_found("项目目录不存在或无法访问"))?;
    let root = ProjectRoot::open(path)?;
    load_ticket_detail_from_project_root(&root, project_id, ticket_id)
}

fn load_ticket_detail_from_project_root(
    root: &ProjectRoot,
    project_id: &str,
    ticket_id: &str,
) -> ApiResult<Value> {
    if !valid_ticket_id(ticket_id) {
        return Err(ApiError::bad_request("任务票 ID 无效"));
    }
    let index_path = INDEX_PATHS
        .iter()
        .find_map(|relative| match root.open_file(relative) {
            Ok(Some(_)) => Some(Ok(*relative)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .transpose()?
        .ok_or_else(|| ApiError::not_found("未找到治理票索引"))?;
    let mut budget = MAX_INDEX_BYTES;
    let index = read_object(&root, index_path, MAX_INDEX_BYTES, &mut budget)?;
    let entries = index
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad_request("治理票索引缺少 issues 数组"))?;
    if entries.len() > MAX_TICKETS {
        return Err(ApiError::bad_request(
            "治理票索引超过 4000 条，请归档历史票",
        ));
    }
    let mut statuses = HashMap::new();
    for entry in entries {
        let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        let status = entry
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if valid_ticket_id(id) && !status.is_empty() && status.len() <= 100 {
            if statuses.insert(id, status).is_some() {
                return Err(ApiError::bad_request(format!(
                    "治理票索引包含重复 ID：{id}"
                )));
            }
        }
    }
    let entry = entries
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(ticket_id))
        .ok_or_else(|| ApiError::not_found(format!("任务票不存在：{ticket_id}")))?;
    let status = *statuses
        .get(ticket_id)
        .ok_or_else(|| ApiError::bad_request("任务票索引记录无效"))?;
    let issues_dir = index_path.strip_suffix("/index.json").unwrap();
    let source_path = format!("{issues_dir}/{ticket_id}.json");
    let mut body_budget = MAX_BODY_BYTES;
    let result =
        read_object(&root, &source_path, MAX_BODY_BYTES, &mut body_budget).and_then(|body| {
            if body
                .get("id")
                .is_some_and(|body_id| body_id.as_str() != Some(ticket_id))
            {
                return Err(ApiError::bad_request("票体 ID 与索引不一致"));
            }
            Ok(body)
        });
    let (body, body_error) = match result {
        Ok(body) => (Some(body), None),
        Err(error) => (None, Some(error.message)),
    };
    let dependency_result = dependencies(
        entry
            .get("blocked_by")
            .or_else(|| body.as_ref().and_then(|value| value.get("blocked_by"))),
    );
    let (blocked_by, dependency_error) = match dependency_result {
        Ok(ids) => (ids, None),
        Err(message) => (vec![], Some(message.to_owned())),
    };
    let unresolved: Vec<_> = blocked_by
        .iter()
        .filter(|dependency| statuses.get(dependency.as_str()) != Some(&"done"))
        .cloned()
        .collect();
    let incomplete = body.as_ref().is_some_and(|body| {
        body.get("goal")
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
            || body
                .get("acceptance")
                .and_then(Value::as_array)
                .is_none_or(|values| {
                    values.is_empty()
                        || values
                            .iter()
                            .any(|value| value.as_str().is_none_or(|s| s.trim().is_empty()))
                })
    });
    let readiness_reason = body_error.clone().or(dependency_error).or_else(|| {
        if incomplete {
            Some("票体缺少目标或验收条件".into())
        } else if status != "ready" {
            Some(format!("当前状态 {status} 不允许启动；仅 ready 任务可交接"))
        } else if !unresolved.is_empty() {
            Some(format!("依赖尚未完成或不存在：{}", unresolved.join("、")))
        } else {
            None
        }
    });
    let title = entry
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            body.as_ref()
                .and_then(|value| value.get("title"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or(ticket_id);
    let goal_summary = body
        .as_ref()
        .and_then(|value| value.get("goal"))
        .and_then(Value::as_str)
        .map(|value| value.chars().take(300).collect::<String>());
    let ticket = json!({
        "id": ticket_id,
        "title": title.chars().take(300).collect::<String>(),
        "lane": lane_for_status(status),
        "governance_status": status,
        "blocked_by": blocked_by,
        "unresolved_dependencies": unresolved,
        "updated_at": entry.get("updated_at").and_then(Value::as_str),
        "complexity": entry.get("complexity").and_then(Value::as_str),
        "source": "governance",
        "source_path": source_path,
        "goal_summary": goal_summary,
        "body_error": body_error,
        "body_loaded": true,
        "ready": readiness_reason.is_none(),
        "readiness_reason": readiness_reason,
        "body": body,
    });
    Ok(json!({ "project_id": project_id, "index_path": index_path, "ticket": ticket }))
}

pub fn create_ticket_from_root(
    root: &Path,
    project_id: &str,
    draft: &TicketDraft,
    ticket_ops_script: &Path,
) -> ApiResult<Value> {
    draft.validate()?;
    let path = root
        .canonicalize()
        .map_err(|_| ApiError::not_found("项目目录不存在或无法访问"))?;
    let root_handle = ProjectRoot::open(path.clone())?;
    #[cfg(unix)]
    let registration = create_ticket_unix(&root_handle, draft, ticket_ops_script)?;
    #[cfg(not(unix))]
    let registration = {
        let _ = (&root_handle, draft, ticket_ops_script);
        return Err(ApiError::bad_request(
            "当前平台无法安全写入治理票；为避免路径竞态，已拒绝写入",
        ));
    };
    let mut detail =
        load_ticket_detail_from_project_root(&root_handle, project_id, &registration.id)?;
    let ticket = detail
        .get_mut("ticket")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| ApiError::internal("新建任务详情缺少 ticket 对象"))?;
    ticket.insert(
        "creation_warning".to_owned(),
        registration
            .creation_warning
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    Ok(detail)
}

pub fn validate_ticket_dependencies_from_root(root: &Path, draft: &TicketDraft) -> ApiResult<()> {
    draft.validate()?;
    if draft.blocked_by.is_empty() {
        return Ok(());
    }
    let path = root
        .canonicalize()
        .map_err(|_| ApiError::not_found("项目目录不存在或无法访问"))?;
    let root = ProjectRoot::open(path)?;
    #[cfg(unix)]
    {
        let index_path = select_index_path(&root)?;
        let mut budget = MAX_INDEX_BYTES;
        let index = read_object(&root, index_path, MAX_INDEX_BYTES, &mut budget)?;
        let issues = index
            .get("issues")
            .and_then(Value::as_array)
            .ok_or_else(|| ApiError::bad_request("任务索引缺少 issues 数组"))?;
        validate_ticket_dependencies(issues, &draft.blocked_by)
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        Err(ApiError::bad_request(
            "当前平台无法安全读取治理票索引；依赖验证已拒绝",
        ))
    }
}

pub fn ticket_creation_capability(root: &Path, ticket_ops_script: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        let canonical = root
            .canonicalize()
            .map_err(|_| "项目目录不存在或无法访问".to_owned())?;
        let root = ProjectRoot::open(canonical).map_err(|error| error.message)?;
        let index_path = select_index_path(&root).map_err(|error| error.message)?;
        if index_path != INDEX_PATHS[0] {
            return Err(
                "当前任务索引为兼容读取来源；建票只支持 facts/requirements/tickets/index.json"
                    .to_owned(),
            );
        }
        let mut budget = MAX_INDEX_BYTES;
        let index = read_object(&root, index_path, MAX_INDEX_BYTES, &mut budget)
            .map_err(|error| error.message)?;
        if !index.get("issues").and_then(Value::as_array).is_some() {
            return Err("正式任务索引缺少 issues 数组".into());
        }
        for relative in [TICKETS_README_PATH, CHANGES_LEDGER_PATH] {
            let file = root
                .open_file(relative)
                .map_err(|error| error.message)?
                .ok_or_else(|| format!("开票前置文件缺失：{relative}"))?;
            if file.metadata().map_err(|error| error.to_string())?.len() > 2 * 1024 * 1024 {
                return Err(format!("开票前置文件超过读取限制：{relative}"));
            }
            if relative == TICKETS_README_PATH {
                let mut bytes = Vec::new();
                file.take(2 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|error| error.to_string())?;
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| "任务 README 不是有效 UTF-8".to_owned())?;
                if !text.lines().any(|line| line.starts_with("| `")) {
                    return Err("任务 README 缺少 ticket-ops 所需的目录清单锚点".into());
                }
            }
        }
        if root
            .open_file(TICKETS_PROGRESS_PATH)
            .map_err(|error| error.message)?
            .is_some()
        {
            // A projection is optional until the first ticket gives the generator rows.
        }
        ticket_ops_environment(ticket_ops_script)
            .map(|_| ())
            .map_err(|error| error.message)
    }
    #[cfg(not(unix))]
    {
        let _ = (root, ticket_ops_script);
        Err("当前平台不支持安全票务写入".into())
    }
}

#[cfg(unix)]
fn validate_ticket_dependencies(issues: &[Value], dependencies: &[String]) -> ApiResult<()> {
    let mut existing = HashSet::new();
    for entry in issues {
        if let Some(id) = entry.get("id").and_then(Value::as_str) {
            if valid_formal_ticket_id(id) && !existing.insert(id.to_owned()) {
                return Err(ApiError::bad_request(format!(
                    "治理票索引包含重复 ID：{id}"
                )));
            }
        }
    }
    for id in dependencies {
        if !existing.contains(id) {
            return Err(ApiError::bad_request(format!("依赖任务不存在于索引：{id}")));
        }
    }
    Ok(())
}

#[cfg(unix)]
#[derive(Debug)]
struct TicketCreationRegistration {
    id: String,
    creation_warning: Option<String>,
}

#[cfg(unix)]
fn create_ticket_unix(
    root: &ProjectRoot,
    draft: &TicketDraft,
    ticket_ops_script: &Path,
) -> ApiResult<TicketCreationRegistration> {
    create_ticket_unix_with_hook(root, draft, ticket_ops_script, || Ok(()))
}

#[cfg(unix)]
fn create_ticket_unix_with_hook<F>(
    root: &ProjectRoot,
    draft: &TicketDraft,
    ticket_ops_script: &Path,
    before_index_replace: F,
) -> ApiResult<TicketCreationRegistration>
where
    F: FnOnce() -> ApiResult<()>,
{
    use std::os::fd::AsRawFd;

    if unsafe { libc::flock(root.directory.as_raw_fd(), libc::LOCK_EX) } < 0 {
        return Err(ApiError::internal(format!(
            "无法锁定任务索引：{}",
            std::io::Error::last_os_error()
        )));
    }
    let _lock = RootDirectoryLock(&root.directory);

    let index_path = select_index_path(root)?;
    if index_path != INDEX_PATHS[0] {
        return Err(ApiError::bad_request(
            "安全开票要求 facts/requirements/tickets/index.json 正式索引；旧路径仅支持读取",
        ));
    }
    let issues_dir = index_path.strip_suffix("/index.json").unwrap();
    let directory = root.open_directory(issues_dir)?;
    let index_name = c_name("index.json")?;
    let mut index_file = open_file_at(&directory, &index_name, index_path)?
        .ok_or_else(|| ApiError::conflict("任务索引在创建期间消失，请刷新后重试"))?;
    let original_meta = index_file.metadata()?;
    let original_bytes = read_bounded_file(&mut index_file, MAX_INDEX_BYTES, index_path)?;
    let index: Value = serde_json::from_slice(&original_bytes)
        .map_err(|_| ApiError::bad_request(format!("任务票索引不是有效 JSON：{index_path}")))?;
    if !index.is_object() {
        return Err(ApiError::bad_request("治理票索引须为 JSON 对象"));
    }
    let issues = index
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad_request("任务索引缺少 issues 数组"))?;
    if issues.len() >= MAX_TICKETS {
        return Err(ApiError::bad_request("治理票索引已达 4000 条上限"));
    }
    let mut ids = HashSet::new();
    for entry in issues.iter() {
        if let Some(id) = entry.get("id").and_then(Value::as_str) {
            if valid_ticket_id(id) && !ids.insert(id.to_owned()) {
                return Err(ApiError::bad_request(format!(
                    "治理票索引包含重复 ID：{id}"
                )));
            }
        }
    }
    validate_ticket_dependencies(issues, &draft.blocked_by)?;
    let id = next_ticket_id(&ids, &draft.title)?;
    let body_name = c_name(&format!("{id}.json"))?;
    if path_entry_exists(&directory, &body_name)? {
        return Err(ApiError::conflict(format!(
            "任务票文件已存在但索引未登记：{id}"
        )));
    }

    let body = json!({
        "id": id,
        "kind": "task",
        "title": draft.title.trim(),
        "created_at": chrono::Utc::now().format("%Y-%m-%d").to_string(),
        "complexity": draft.complexity,
        "profile": draft.profile,
        "blocked_by": draft.blocked_by,
        "complexity_reason": draft.complexity_reason.trim(),
        "priority_reason": draft.priority_reason.trim(),
        "goal": draft.goal.trim(),
        "scope": draft.scope.iter().map(|item| item.trim()).collect::<Vec<_>>(),
        "acceptance": draft.acceptance.iter().map(|item| item.trim()).collect::<Vec<_>>(),
        "verification": draft.verification.trim(),
    });
    let body_bytes = serde_json::to_vec_pretty(&body)?;
    if body_bytes.len() as u64 > MAX_BODY_BYTES {
        return Err(ApiError::bad_request("任务票内容超过大小限制"));
    }
    let (body_temp, body_temp_name) = write_temp_file(&directory, &body_bytes)?;
    let body_meta = match body_temp.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            let _ = unlink_name(&directory, &body_temp_name);
            return Err(ApiError::internal(format!(
                "无法检查任务票临时文件：{error}"
            )));
        }
    };
    drop(body_temp);
    if unsafe {
        libc::linkat(
            directory.as_raw_fd(),
            body_temp_name.as_ptr(),
            directory.as_raw_fd(),
            body_name.as_ptr(),
            0,
        )
    } < 0
    {
        let error = std::io::Error::last_os_error();
        let _ = unlink_name(&directory, &body_temp_name);
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(ApiError::conflict(format!("任务票 ID 已被占用：{id}")));
        }
        return Err(path_io_error(&format!("{issues_dir}/{id}.json"), error));
    }
    let _ = unlink_name(&directory, &body_temp_name);

    let command_result = (|| {
        before_index_replace()?;
        verify_unchanged_index(
            &directory,
            &index_name,
            index_path,
            &original_meta,
            &original_bytes,
        )?;
        run_ticket_ops(root, ticket_ops_script, &id, draft)
    })();
    let registered = index_has_ticket(&directory, &index_name, index_path, &id)?;
    let mut creation_warning = match (command_result, registered) {
        (Ok(()), true) => None,
        (Ok(()), false) => {
            remove_own_unregistered_body(&directory, &body_name, &body_meta)?;
            return Err(ApiError::conflict(
                "ticket-ops 返回成功但票号未登记；已清理未登记票体",
            ));
        }
        (Err(error), true) => Some(ticket_creation_warning(&id, &error.message)),
        (Err(error), false) => {
            remove_own_unregistered_body(&directory, &body_name, &body_meta)?;
            return Err(ApiError::conflict(format!(
                "ticket-ops 失败且索引未登记；已清理本次票体：{}",
                error.message
            )));
        }
    };
    if unsafe { libc::fsync(directory.as_raw_fd()) } < 0 {
        let sync_error = std::io::Error::last_os_error();
        let warning = ticket_creation_warning(
            &id,
            &format!("任务目录同步失败，落盘状态需核查：{sync_error}"),
        );
        creation_warning = Some(match creation_warning {
            Some(existing) => format!("{existing} {warning}"),
            None => warning,
        });
    }
    Ok(TicketCreationRegistration {
        id,
        creation_warning,
    })
}

#[cfg(unix)]
fn ticket_creation_warning(id: &str, cause: &str) -> String {
    format!(
        "任务 {id} 已登记，票体和索引已保留；后续登记未完整完成。请勿重复创建或启动此任务。先检查并修复 facts/requirements/tickets/README.md、facts/project/changes.jsonl 和 facts/requirements/tickets/progress-current.md，再确认后续处理。原因：{cause}"
    )
}

#[cfg(unix)]
struct RootDirectoryLock<'a>(&'a File);

#[cfg(unix)]
impl Drop for RootDirectoryLock<'_> {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(unix)]
fn select_index_path(root: &ProjectRoot) -> ApiResult<&'static str> {
    for path in INDEX_PATHS {
        if root.open_file(path)?.is_some() {
            return Ok(path);
        }
    }
    Err(ApiError::not_found("未找到任务索引，请先接入项目"))
}

#[cfg(unix)]
fn read_bounded_file(file: &mut File, limit: u64, path: &str) -> ApiResult<Vec<u8>> {
    let size = file.metadata()?.len();
    if size > limit {
        return Err(ApiError::bad_request(format!(
            "任务票来源超过读取大小限制：{path}"
        )));
    }
    let mut bytes = Vec::new();
    file.by_ref().take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(ApiError::bad_request(format!(
            "任务票来源超过读取大小限制：{path}"
        )));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn verify_unchanged_index(
    directory: &File,
    index_name: &CString,
    index_path: &str,
    original_meta: &std::fs::Metadata,
    original_bytes: &[u8],
) -> ApiResult<()> {
    let mut current = open_file_at(directory, index_name, index_path)?
        .ok_or_else(|| ApiError::conflict("任务索引在创建期间消失，请刷新后重试"))?;
    let current_meta = current.metadata()?;
    let current_bytes = read_bounded_file(&mut current, MAX_INDEX_BYTES, index_path)
        .map_err(|_| ApiError::conflict("任务索引在创建期间发生变化，请刷新后重试"))?;
    if current_meta.dev() != original_meta.dev()
        || current_meta.ino() != original_meta.ino()
        || current_bytes != original_bytes
    {
        return Err(ApiError::conflict(
            "任务索引在创建期间发生变化，请刷新后重试",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn index_has_ticket(
    directory: &File,
    index_name: &CString,
    index_path: &str,
    ticket_id: &str,
) -> ApiResult<bool> {
    let mut file = open_file_at(directory, index_name, index_path)?
        .ok_or_else(|| ApiError::conflict("ticket-ops 执行后任务索引消失；保留票体现场"))?;
    let bytes = read_bounded_file(&mut file, MAX_INDEX_BYTES, index_path)?;
    let index: Value = serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::conflict("ticket-ops 执行后索引损坏；保留票体现场"))?;
    let issues = index
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::conflict("ticket-ops 执行后索引缺少 issues；保留票体现场"))?;
    Ok(issues
        .iter()
        .any(|entry| entry.get("id").and_then(Value::as_str) == Some(ticket_id)))
}

#[cfg(unix)]
fn remove_own_unregistered_body(
    directory: &File,
    body_name: &CString,
    body_meta: &std::fs::Metadata,
) -> ApiResult<()> {
    if path_entry_matches(directory, body_name, body_meta)? {
        unlink_name(directory, body_name)?;
        if unsafe { libc::fsync(directory.as_raw_fd()) } < 0 {
            return Err(ApiError::internal(format!(
                "未登记票体已删除，但目录同步失败：{}",
                std::io::Error::last_os_error()
            )));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn run_ticket_ops(
    root: &ProjectRoot,
    ticket_ops_script: &Path,
    id: &str,
    draft: &TicketDraft,
) -> ApiResult<()> {
    use std::os::unix::process::CommandExt;

    if !valid_formal_ticket_id(id) {
        return Err(ApiError::bad_request("生成的任务票 ID 不符合正式票号格式"));
    }
    if draft
        .title
        .chars()
        .any(|ch| matches!(ch, '|' | '`' | '\n' | '\r' | '\t' | '"' | '\\'))
    {
        return Err(ApiError::bad_request(
            "任务标题含 ticket-ops README 或账本不可安全表示的字符",
        ));
    }
    let (script, skill_root, child_path) = ticket_ops_environment(ticket_ops_script)?;

    let ledger_line = format!(
        "{{\"date\":\"{}\",\"kind\":\"change\",\"scope\":\"{}\",\"decision\":\"用户确认创建任务票\",\"evidence_ref\":\"facts/requirements/tickets/{}.json\"}}",
        chrono::Utc::now().format("%Y-%m-%d"),
        draft.title.trim(),
        id
    );
    let blocked_by = draft.blocked_by.join(",");
    let root_fd = root.directory.as_raw_fd();
    verify_root_path(root)?;
    // The child changes directory to the pinned root fd in pre_exec, so a relative root
    // argument keeps ticket-ops anchored even if the selected path is renamed.
    let root_arg = ".";
    let mut command = std::process::Command::new("/bin/sh");
    command
        .arg(&script)
        .arg(root_arg)
        .arg("open")
        .arg("--id")
        .arg(id)
        .arg("--complexity")
        .arg(&draft.complexity)
        .arg("--title")
        .arg(draft.title.trim())
        .arg("--ledger-line")
        .arg(ledger_line)
        .current_dir("/")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if !blocked_by.is_empty() {
        command.arg("--blocked-by").arg(blocked_by);
    }
    command
        .env("AGENTUP_SKILL_ROOT", skill_root)
        .env("PATH", child_path);
    unsafe {
        command.pre_exec(move || {
            let flags = libc::fcntl(root_fd, libc::F_GETFD);
            if flags < 0 || libc::fcntl(root_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::fchdir(root_fd) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = command
        .output()
        .map_err(|error| ApiError::internal(format!("无法启动 ticket-ops：{error}")))?;
    verify_root_path(root)?;
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(ApiError::internal(format!(
        "ticket-ops 退出 {:?}：{}{}",
        output.status.code(),
        stdout.chars().take(1200).collect::<String>(),
        stderr.chars().take(1200).collect::<String>()
    )))
}

#[cfg(unix)]
fn ticket_ops_environment(
    ticket_ops_script: &Path,
) -> ApiResult<(PathBuf, PathBuf, std::ffi::OsString)> {
    let script_meta = std::fs::symlink_metadata(ticket_ops_script)
        .map_err(|error| ApiError::internal(format!("无法读取 ticket-ops resource：{error}")))?;
    if !script_meta.is_file() || script_meta.file_type().is_symlink() {
        return Err(ApiError::bad_request("ticket-ops resource 不是普通文件"));
    }
    let script = ticket_ops_script
        .canonicalize()
        .map_err(|error| ApiError::internal(format!("ticket-ops resource 不可用：{error}")))?;
    let skill_root = script
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| ApiError::internal("ticket-ops resource 目录结构无效"))?
        .to_path_buf();
    let script_directory = script
        .parent()
        .ok_or_else(|| ApiError::internal("ticket-ops resource 缺少脚本目录"))?;
    let git = resolve_tool("git")
        .ok_or_else(|| ApiError::bad_request("开票依赖 git 不可用；未写入索引"))?;
    let python = resolve_tool("python3")
        .ok_or_else(|| ApiError::bad_request("开票依赖 python3 不可用；未写入索引"))?;
    let child_path = ticket_ops_path(&git, &python)?;
    if !script_directory.join("generate-progress.sh").is_file() {
        return Err(ApiError::bad_request(
            "ticket-ops resource 缺少 generate-progress.sh；未写入索引",
        ));
    }
    if !skill_root
        .join("references/schemas/ticket-record.schema.json")
        .is_file()
    {
        return Err(ApiError::bad_request(
            "ticket-ops resource 缺少正式票体 schema；未写入索引",
        ));
    }
    Ok((script, skill_root, child_path))
}

#[cfg(unix)]
fn verify_root_path(root: &ProjectRoot) -> ApiResult<()> {
    let current = open_absolute_directory(&root.path)
        .map_err(|_| ApiError::conflict("项目路径在开票期间发生变化；未继续执行"))?;
    if current.metadata()?.dev() != root.directory.metadata()?.dev()
        || current.metadata()?.ino() != root.directory.metadata()?.ino()
    {
        return Err(ApiError::conflict(
            "项目路径已不再指向开票时固定的目录；请刷新后重试",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn resolve_tool(binary: &str) -> Option<PathBuf> {
    let mut directories: Vec<_> = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .collect();
    directories.extend(match binary {
        "git" => vec![
            PathBuf::from("/usr/bin"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ],
        "python3" => vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
        ],
        _ => vec![],
    });
    directories
        .into_iter()
        .map(|directory| directory.join(binary))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(unix)]
fn ticket_ops_path(git: &Path, python: &Path) -> ApiResult<std::ffi::OsString> {
    let inherited: Vec<_> = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .collect();
    let mut directories = vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")];
    for executable in [git, python] {
        if let Some(parent) = executable.parent() {
            if !directories.iter().any(|directory| directory == parent) {
                directories.push(parent.to_path_buf());
            }
        }
    }
    for directory in inherited {
        if !directories.iter().any(|existing| existing == &directory) {
            directories.push(directory);
        }
    }
    std::env::join_paths(directories)
        .map_err(|error| ApiError::internal(format!("无法配置 ticket-ops 工具 PATH：{error}")))
}

#[cfg(unix)]
fn open_file_at(directory: &File, name: &CString, path: &str) -> ApiResult<Option<File>> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(path_io_error(path, error));
    }
    let file = unsafe { File::from_raw_fd(fd) };
    if !file.metadata()?.is_file() {
        return Err(ApiError::bad_request(format!(
            "任务票来源不是普通文件：{path}"
        )));
    }
    Ok(Some(file))
}

#[cfg(unix)]
fn next_ticket_id(ids: &HashSet<String>, title: &str) -> ApiResult<String> {
    let next = ids
        .iter()
        .filter_map(|id| id.split_once('-'))
        .filter(|(prefix, _)| prefix.len() >= 2 && prefix.bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|(prefix, _)| prefix.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| ApiError::conflict("无法生成新的任务票编号"))?;
    let mut number = next;
    loop {
        let id = format!("{number:02}-{}", ticket_slug(title));
        if !ids.contains(&id) {
            return Ok(id);
        }
        number = number
            .checked_add(1)
            .ok_or_else(|| ApiError::conflict("无法生成新的任务票编号"))?;
    }
}

fn ticket_slug(title: &str) -> String {
    let mut slug = String::new();
    let mut separator_pending = false;
    for byte in title.bytes() {
        if byte.is_ascii_alphanumeric() {
            if separator_pending && !slug.is_empty() && slug.len() < 31 {
                slug.push('-');
            }
            separator_pending = false;
            if slug.len() < 32 {
                slug.push((byte as char).to_ascii_lowercase());
            }
        } else {
            separator_pending = true;
        }
        if slug.len() >= 32 {
            break;
        }
    }
    if slug.is_empty() {
        "task".to_owned()
    } else {
        slug.trim_end_matches('-').to_owned()
    }
}

#[cfg(unix)]
fn c_name(name: &str) -> ApiResult<CString> {
    CString::new(name).map_err(|_| ApiError::bad_request("任务票文件名无效"))
}

#[cfg(unix)]
fn path_entry_exists(directory: &File, name: &CString) -> ApiResult<bool> {
    use std::os::fd::AsRawFd;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let result = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::NotFound {
        Ok(false)
    } else {
        Err(ApiError::bad_request(format!(
            "无法检查任务票路径：{error}"
        )))
    }
}

#[cfg(unix)]
fn path_entry_matches(
    directory: &File,
    name: &CString,
    expected: &std::fs::Metadata,
) -> ApiResult<bool> {
    use std::os::fd::AsRawFd;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let result = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(false);
        }
        return Err(ApiError::bad_request(format!(
            "无法检查任务票路径：{error}"
        )));
    }
    let stat = unsafe { stat.assume_init() };
    Ok(stat.st_dev as u64 == expected.dev() && stat.st_ino as u64 == expected.ino())
}

#[cfg(unix)]
fn write_temp_file(directory: &File, bytes: &[u8]) -> ApiResult<(File, CString)> {
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd};
    for _ in 0..8 {
        let name = c_name(&format!(".agentup-ticket-{}.tmp", uuid::Uuid::new_v4()))?;
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                continue;
            }
            return Err(ApiError::internal(format!(
                "无法创建任务票临时文件：{error}"
            )));
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
            let _ = unlink_name(directory, &name);
            return Err(ApiError::internal(format!(
                "无法写入任务票临时文件：{error}"
            )));
        }
        return Ok((file, name));
    }
    Err(ApiError::conflict("无法分配唯一的任务票临时文件名"))
}

#[cfg(unix)]
fn unlink_name(directory: &File, name: &CString) -> ApiResult<()> {
    use std::os::fd::AsRawFd;
    if unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) } < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(ApiError::internal(format!(
                "无法清理任务票临时文件：{error}"
            )));
        }
    }
    Ok(())
}

pub fn load_governance_tickets(
    conn: &Connection,
    project_id: &str,
    load_bodies: bool,
) -> ApiResult<Value> {
    let root = db::get_project_path(conn, project_id)?
        .ok_or_else(|| ApiError::bad_request("项目未绑定目录，无法读取治理票"))?;
    load_from_root(Path::new(&root), project_id, load_bodies)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{fs, os::unix::fs::symlink};
    use tempfile::TempDir;

    #[test]
    fn opened_root_fd_is_stable_when_its_path_is_replaced() {
        let parent = TempDir::new().unwrap();
        let root_path = parent.path().join("project");
        fs::create_dir(&root_path).unwrap();
        let index = json!({"issues":[{"id":"original","status":"ready"}]});
        fs::create_dir_all(root_path.join("docs/issues")).unwrap();
        fs::write(
            root_path.join("docs/issues/index.json"),
            serde_json::to_vec(&index).unwrap(),
        )
        .unwrap();
        let root = ProjectRoot::open(root_path.clone()).unwrap();

        let moved = parent.path().join("moved-project");
        fs::rename(&root_path, &moved).unwrap();
        let outside = parent.path().join("attacker");
        fs::create_dir_all(outside.join("docs/issues")).unwrap();
        fs::write(
            outside.join("docs/issues/index.json"),
            br#"{"issues":[{"id":"outside","status":"ready"}]}"#,
        )
        .unwrap();
        symlink(&outside, &root_path).unwrap();

        let mut budget = MAX_INDEX_BYTES;
        assert_eq!(
            read_object(
                &root,
                "docs/issues/index.json",
                MAX_INDEX_BYTES,
                &mut budget
            )
            .unwrap(),
            index
        );
    }

    #[test]
    fn ticket_slug_is_ascii_bounded_and_uses_task_for_non_ascii_titles() {
        assert_eq!(
            ticket_slug("New implementation task"),
            "new-implementation-task"
        );
        assert_eq!(
            ticket_slug("  A very long title with punctuation!  "),
            "a-very-long-title-with-punctuati"
        );
        assert_eq!(ticket_slug("修复中文标题"), "task");
        assert_eq!(
            next_ticket_id(&HashSet::from(["01-same-title".to_owned()]), "Same title").unwrap(),
            "02-same-title"
        );
    }

    #[test]
    fn changed_index_after_ticket_publish_rolls_back_body_and_temporary_files() {
        let root_dir = TempDir::new().unwrap();
        fs::create_dir_all(root_dir.path().join("facts/requirements/tickets")).unwrap();
        let index_path = root_dir
            .path()
            .join("facts/requirements/tickets/index.json");
        fs::write(&index_path, br#"{"issues":[]}"#).unwrap();
        let root = ProjectRoot::open(root_dir.path().to_path_buf()).unwrap();

        let result = create_ticket_unix_with_hook(
            &root,
            &TicketDraft {
                title: "Safe creation".into(),
                complexity: "C1".into(),
                profile: "D:Required,B:Advisory,I:Required,U:Required,S:N/A,M:N/A,O:N/A".into(),
                complexity_reason: "One bounded slice.".into(),
                priority_reason: "Requested by the user.".into(),
                goal: "Create a ticket safely".into(),
                scope: vec!["src/".into()],
                acceptance: vec!["Tests pass".into()],
                verification: "cargo test".into(),
                blocked_by: vec![],
            },
            Path::new("unused-ticket-ops.sh"),
            || {
                fs::write(&index_path, br#"{"issues":[],"external_change":true}"#)?;
                Ok(())
            },
        );

        assert_eq!(result.unwrap_err().code, 409);
        assert!(!root_dir
            .path()
            .join("facts/requirements/tickets/01-safe-creation.json")
            .exists());
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&index_path).unwrap()).unwrap()
                ["external_change"],
            true
        );
        let remaining: Vec<_> = fs::read_dir(root_dir.path().join("facts/requirements/tickets"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(remaining, [std::ffi::OsString::from("index.json")]);
    }
}
