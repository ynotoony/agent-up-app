// Input: 已确认任务的仓根、运行 ID、验证命令与已审查的完整变更指纹。
// Output: 隔离 worktree、真实检查与差异证据、仅本地快进提交。
// Pos: 原生交付 Git 边界；不覆盖用户改动、不推远端；变更同步根 README。

use crate::error::{ApiError, ApiResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::io::AsyncReadExt;

const PATCH_LIMIT: usize = 200 * 1024;
const OUTPUT_LIMIT: usize = 64 * 1024;
const COMMIT_SUBJECT: &str = "feat(agentup): complete isolated task";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    pub root: String,
    pub path: String,
    pub branch: String,
    pub base_head: String,
    pub base_branch: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCommand {
    pub program: String,
    pub args: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub program: String,
    pub args: Vec<String>,
    pub exit_code: Option<i32>,
    pub passed: bool,
    pub output: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEvidence {
    pub files: Vec<String>,
    pub stat: String,
    pub patch: String,
    pub fingerprint: String,
}

fn git_command(root: &Path) -> Command {
    let mut c = Command::new("git");
    c.current_dir(root).args([
        "--literal-pathspecs",
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.fsmonitor=false",
    ]);
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_EXTERNAL_DIFF",
    ] {
        c.env_remove(key);
    }
    c.env("GIT_TERMINAL_PROMPT", "0");
    c
}
fn git_output(root: &Path, args: &[&str]) -> ApiResult<Output> {
    Ok(git_command(root).args(args).output()?)
}
fn git(root: &Path, args: &[&str]) -> ApiResult<Vec<u8>> {
    let out = git_output(root, args)?;
    if !out.status.success() {
        return Err(ApiError::conflict(format!(
            "Git {} 失败：{}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(out.stdout)
}
fn git_text(root: &Path, args: &[&str]) -> ApiResult<String> {
    String::from_utf8(git(root, args)?)
        .map(|s| s.trim().to_owned())
        .map_err(|_| ApiError::bad_request("Git 路径须使用 UTF-8"))
}
fn canonical_root(path: &Path) -> ApiResult<PathBuf> {
    let root = fs::canonicalize(path)?;
    let reported = git_text(&root, &["rev-parse", "--show-toplevel"])?;
    if fs::canonicalize(reported)? != root
        || git_text(&root, &["rev-parse", "--is-bare-repository"])? != "false"
    {
        return Err(ApiError::bad_request("请选择非裸 Git 仓库的根目录"));
    }
    Ok(root)
}
fn text_path(path: &Path) -> ApiResult<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| ApiError::bad_request("项目路径须使用 UTF-8"))
}
fn paths(data: Vec<u8>) -> ApiResult<Vec<String>> {
    data.split(|b| *b == 0)
        .filter(|part| !part.is_empty())
        .map(|part| {
            String::from_utf8(part.to_vec())
                .map_err(|_| ApiError::bad_request("文件路径须使用 UTF-8"))
        })
        .collect()
}
fn expected_path(root: &Path, id: &str) -> ApiResult<PathBuf> {
    let name = root
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ApiError::bad_request("仓库路径没有名称"))?;
    Ok(root
        .parent()
        .ok_or_else(|| ApiError::bad_request("仓库缺少父目录"))?
        .join(format!("{name}-agentup-runs"))
        .join(id))
}
fn run_id(w: &Worktree) -> ApiResult<&str> {
    let id = w
        .branch
        .strip_prefix("codex/run-")
        .ok_or_else(|| ApiError::conflict("交付分支不是 App 创建的分支"))?;
    if !uuid::Uuid::parse_str(id).is_ok_and(|u| u.to_string() == id) {
        return Err(ApiError::conflict("运行 ID 无效"));
    }
    Ok(id)
}
fn validate(w: &Worktree, require_base: bool) -> ApiResult<()> {
    if ![40, 64].contains(&w.base_head.len()) || !w.base_head.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(ApiError::conflict("交付基线提交身份无效"));
    }
    let root = canonical_root(Path::new(&w.root))?;
    let path = canonical_root(Path::new(&w.path))?;
    if root != Path::new(&w.root)
        || path != Path::new(&w.path)
        || path != expected_path(&root, run_id(w)?)?
    {
        return Err(ApiError::conflict("交付工作目录身份不匹配"));
    }
    let common = |p: &Path| -> ApiResult<PathBuf> {
        Ok(fs::canonicalize(git_text(
            p,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?)?)
    };
    if common(&root)? != common(&path)?
        || git_text(&path, &["symbolic-ref", "--short", "HEAD"])? != w.branch
    {
        return Err(ApiError::conflict("交付 worktree 或分支已被替换"));
    }
    if require_base && git_text(&path, &["rev-parse", "HEAD"])? != w.base_head {
        return Err(ApiError::conflict(
            "执行 Agent 改变了提交基线，不能直接接收",
        ));
    }
    Ok(())
}

pub fn create(root: &Path, run_id: &str) -> ApiResult<Worktree> {
    if !uuid::Uuid::parse_str(run_id).is_ok_and(|u| u.to_string() == run_id) {
        return Err(ApiError::bad_request("运行 ID 必须是 UUID"));
    }
    let root = canonical_root(root)?;
    let base_head = git_text(&root, &["rev-parse", "--verify", "HEAD"])?;
    let base_branch = git_text(&root, &["symbolic-ref", "--short", "HEAD"])?;
    let path = expected_path(&root, run_id)?;
    let parent = path.parent().unwrap();
    if parent.exists()
        && (fs::symlink_metadata(parent)?.file_type().is_symlink()
            || fs::canonicalize(parent)? != parent)
    {
        return Err(ApiError::conflict("隔离目录的父目录不允许符号链接"));
    }
    fs::create_dir_all(parent)?;
    let branch = format!("codex/run-{run_id}");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-b",
            &branch,
            &text_path(&path)?,
            &base_head,
        ],
    )?;
    Ok(Worktree {
        root: text_path(&root)?,
        path: text_path(&path)?,
        branch,
        base_head,
        base_branch,
    })
}

fn changed_files(root: &Path, base: &str) -> ApiResult<Vec<String>> {
    let mut files: BTreeSet<String> = paths(git(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--name-only",
            "--no-renames",
            "-z",
            base,
            "--",
        ],
    )?)?
    .into_iter()
    .collect();
    files.extend(paths(git(
        root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?)?);
    Ok(files.into_iter().collect())
}
fn safe_file(root: &Path, name: &str) -> ApiResult<()> {
    let path = Path::new(name);
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || name == ".agentup-app"
        || name.starts_with(".agentup-app/")
        || path.components().any(|c| c.as_os_str() == ".git")
    {
        return Err(ApiError::conflict(format!(
            "Agent 不得改写应用记录或 Git 元数据：{name}"
        )));
    }
    let basename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if (basename == ".env" || basename.starts_with(".env."))
        && ![".env.example", ".env.sample", ".env.template"].contains(&basename)
        || ["id_rsa", "id_ed25519", "credentials.json"].contains(&basename)
    {
        return Err(ApiError::conflict(format!(
            "变更包含可能的凭证文件，请移除后重试：{name}"
        )));
    }
    let mut current = root.to_path_buf();
    for component in path.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(ApiError::conflict(format!(
                    "交付暂不支持符号链接变更：{name}"
                )))
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(e.into()),
        }
    }
    if root.join(path).exists() && !root.join(path).is_file() {
        return Err(ApiError::conflict(format!(
            "交付暂不支持子模块或特殊文件：{name}"
        )));
    }
    Ok(())
}
// Drain both streams while retaining bounded bytes; large output cannot deadlock Git.
fn bounded_git(root: &Path, args: &[&str], allow_one: bool) -> ApiResult<Vec<u8>> {
    let mut child = git_command(root)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    fn drain(mut r: impl Read, limit: usize) -> std::io::Result<(Vec<u8>, bool)> {
        let mut kept = Vec::new();
        let mut buf = [0u8; 8192];
        let mut overflow = false;
        loop {
            let n = r.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let take = n.min(limit.saturating_sub(kept.len()));
            kept.extend_from_slice(&buf[..take]);
            overflow |= take < n;
        }
        Ok((kept, overflow))
    }
    let out = std::thread::spawn(move || drain(stdout, PATCH_LIMIT));
    let err = std::thread::spawn(move || drain(stderr, OUTPUT_LIMIT));
    let status = child.wait()?;
    let (bytes, overflow) = out
        .join()
        .map_err(|_| ApiError::internal("读取差异失败"))??;
    let (errors, _) = err
        .join()
        .map_err(|_| ApiError::internal("读取 Git 错误失败"))??;
    if !status.success() && !(allow_one && status.code() == Some(1)) {
        return Err(ApiError::conflict(String::from_utf8_lossy(&errors)));
    }
    if overflow {
        return Err(ApiError::conflict(
            "变更超过 200KB 审查上限，请拆小任务后重新执行",
        ));
    }
    Ok(bytes)
}
fn evidence(w: &Worktree) -> ApiResult<DiffEvidence> {
    let root = Path::new(&w.path);
    let files = changed_files(root, &w.base_head)?;
    if files.is_empty() {
        return Err(ApiError::conflict("没有可交付的文件变更"));
    }
    let mut hash = Sha256::new();
    hash.update(b"agentup-delivery-v1\0");
    hash.update(w.base_head.as_bytes());
    let mut patch = bounded_git(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            &w.base_head,
            "--",
        ],
        false,
    )?;
    let untracked: BTreeSet<_> = paths(git(
        root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?)?
    .into_iter()
    .collect();
    for name in &files {
        safe_file(root, name)?;
        if untracked.contains(name) {
            patch.extend_from_slice(&bounded_git(
                root,
                &[
                    "diff",
                    "--no-index",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--",
                    "/dev/null",
                    name,
                ],
                true,
            )?);
        }
        if patch.len() > PATCH_LIMIT {
            return Err(ApiError::conflict(
                "变更超过 200KB 审查上限，请拆小任务后重新执行",
            ));
        }
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        match fs::File::open(root.join(name)) {
            Ok(mut file) => {
                hash.update(b"file\0");
                hash.update(file.metadata()?.len().to_be_bytes());
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    hash.update([u8::from(file.metadata()?.permissions().mode() & 0o111 != 0)]);
                }
                let mut buffer = [0u8; 65536];
                loop {
                    let len = file.read(&mut buffer)?;
                    if len == 0 {
                        break;
                    }
                    hash.update(&buffer[..len]);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                hash.update(b"deleted\0");
            }
            Err(e) => return Err(e.into()),
        }
    }
    let patch = String::from_utf8(patch)
        .map_err(|_| ApiError::conflict("变更包含非 UTF-8 内容，暂不能完整审查"))?;
    if patch.lines().any(|line| {
        line.starts_with("Binary files ")
            || line == "GIT binary patch"
            || line.contains("mode 160000")
    }) {
        return Err(ApiError::conflict("变更包含二进制文件，暂不能完整审查"));
    }
    let mut stat = git_text(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--stat",
            "--no-renames",
            &w.base_head,
            "--",
        ],
    )?;
    if !untracked.is_empty() {
        stat.push_str(&format!("\n新增未跟踪文件：{}", untracked.len()));
    }
    Ok(DiffEvidence {
        files,
        stat,
        patch,
        fingerprint: format!("{:x}", hash.finalize()),
    })
}

pub fn diff(w: &Worktree) -> ApiResult<DiffEvidence> {
    validate(w, true)?;
    evidence(w)
}

async fn drain_async(mut reader: impl tokio::io::AsyncRead + Unpin, data: Arc<Mutex<Vec<u8>>>) {
    let mut buffer = [0u8; 8192];
    loop {
        let Ok(n) = reader.read(&mut buffer).await else {
            break;
        };
        if n == 0 {
            break;
        }
        let mut kept = data.lock().unwrap();
        let take = n.min(OUTPUT_LIMIT.saturating_sub(kept.len()));
        kept.extend_from_slice(&buffer[..take]);
    }
}
fn captured(data: &Arc<Mutex<Vec<u8>>>) -> String {
    let kept = data.lock().unwrap();
    let mut text = String::from_utf8_lossy(&kept).into_owned();
    if kept.len() == OUTPUT_LIMIT {
        text.push_str("\n[输出达到 64KB，后续内容已省略]");
    }
    text
}
fn cargo_cache(w: &Worktree) -> ApiResult<PathBuf> {
    let path = expected_path(Path::new(&w.root), run_id(w)?)?;
    let parent = path
        .parent()
        .ok_or_else(|| ApiError::conflict("隔离构建缓存目录无效"))?;
    if fs::symlink_metadata(parent)?.file_type().is_symlink() || fs::canonicalize(parent)? != parent
    {
        return Err(ApiError::conflict("隔离构建缓存的父目录包含符号链接"));
    }
    let cache = parent.join(".cargo-target");
    match fs::symlink_metadata(&cache) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(ApiError::conflict("隔离构建缓存不是普通目录，不能写入")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&cache)?,
        Err(error) => return Err(error.into()),
    }
    if fs::canonicalize(&cache)? != cache
        || cache.starts_with(&w.root)
        || cache.starts_with(&w.path)
    {
        return Err(ApiError::conflict("隔离构建缓存越过了应用目录边界"));
    }
    Ok(cache)
}

/// Commands are supplied by App policy and shown before execution, never inferred from agent text.
pub async fn run_checks(w: &Worktree, commands: &[VerificationCommand]) -> Vec<CheckResult> {
    let mut results = Vec::new();
    for command in commands {
        let mut result = CheckResult {
            program: command.program.clone(),
            args: command.args.clone(),
            exit_code: None,
            passed: false,
            output: String::new(),
        };
        if let Err(error) = validate(w, true) {
            result.output = error.message;
            results.push(result);
            break;
        }
        if command.program.trim().is_empty() {
            result.output = "验证命令不能为空".into();
            results.push(result);
            continue;
        }
        let mut process = tokio::process::Command::new(&command.program);
        process
            .args(&command.args)
            .current_dir(&w.path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for key in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"] {
            process.env_remove(key);
        }
        let is_cargo = Path::new(&command.program)
            .file_name()
            .is_some_and(|name| name == "cargo");
        let timeout_seconds = if is_cargo { 600 } else { 180 };
        if is_cargo {
            match cargo_cache(w) {
                Ok(cache) => {
                    process.env("CARGO_TARGET_DIR", cache);
                }
                Err(error) => {
                    result.output = error.message;
                    results.push(result);
                    continue;
                }
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            process.as_std_mut().process_group(0);
        }
        let mut child = match process.spawn() {
            Ok(child) => child,
            Err(e) => {
                result.output = format!("无法启动验证命令：{e}");
                results.push(result);
                continue;
            }
        };
        let pid = child.id();
        let stdout = Arc::new(Mutex::new(Vec::new()));
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let out_task = tokio::spawn(drain_async(child.stdout.take().unwrap(), stdout.clone()));
        let err_task = tokio::spawn(drain_async(child.stderr.take().unwrap(), stderr.clone()));
        match tokio::time::timeout(Duration::from_secs(timeout_seconds), child.wait()).await {
            Ok(Ok(status)) => {
                result.exit_code = status.code();
                result.passed = status.success();
            }
            Ok(Err(e)) => result.output = format!("等待验证结果失败：{e}\n"),
            Err(_) => {
                #[cfg(unix)]
                if let Some(pid) = pid {
                    let _ = Command::new("/bin/kill")
                        .args(["-KILL", &format!("-{pid}")])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
                let _ = child.kill().await;
                result.output = format!("验证超过 {timeout_seconds} 秒，已停止进程。\n");
            }
        }
        #[cfg(unix)]
        if let Some(pid) = pid {
            if Command::new("/bin/kill")
                .args(["-KILL", &format!("-{pid}")])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
            {
                result.passed = false;
                result
                    .output
                    .push_str("验证结束后仍有运行的子进程，已停止。\n");
            }
        }
        for mut task in [out_task, err_task] {
            if tokio::time::timeout(Duration::from_secs(1), &mut task)
                .await
                .is_err()
            {
                task.abort();
                result.passed = false;
                result
                    .output
                    .push_str("验证留下未退出的子进程，输出不完整。\n");
            }
        }
        result.output.push_str(&captured(&stdout));
        result.output.push_str(&captured(&stderr));
        results.push(result);
    }
    results
}

fn check_main(w: &Worktree, files: &[String], accepted: Option<&str>) -> ApiResult<()> {
    let root = Path::new(&w.root);
    if git_text(root, &["symbolic-ref", "--short", "HEAD"])? != w.base_branch {
        return Err(ApiError::conflict("项目当前分支已改变，请重新执行任务"));
    }
    let head = git_text(root, &["rev-parse", "HEAD"])?;
    if head != w.base_head && accepted != Some(head.as_str()) {
        return Err(ApiError::conflict(
            "项目已新增提交，当前交付基线过期，请重新执行任务",
        ));
    }
    if !git_output(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--cached",
            "--quiet",
            "--",
        ],
    )?
    .status
    .success()
    {
        return Err(ApiError::conflict(
            "项目存在用户暂存的改动，请先处理，App 不会替换暂存区",
        ));
    }
    let dirty = changed_files(root, "HEAD")?;
    if dirty.iter().any(|local| {
        files.iter().any(|file| {
            local == file
                || local.starts_with(&format!("{file}/"))
                || file.starts_with(&format!("{local}/"))
        })
    }) {
        return Err(ApiError::conflict(
            "项目中的未提交改动与任务变更重叠，请先处理；原文件保持不变",
        ));
    }
    Ok(())
}
fn prepared_commit(w: &Worktree, fingerprint: &str) -> ApiResult<Option<String>> {
    let path = Path::new(&w.path);
    let head = git_text(path, &["rev-parse", "HEAD"])?;
    if head == w.base_head {
        return Ok(None);
    }
    let body = git_text(path, &["log", "-1", "--format=%B"])?;
    let expected = format!(
        "{COMMIT_SUBJECT}\n\nAgentUp-Run: {}\nAgentUp-Fingerprint: {fingerprint}",
        run_id(w)?
    );
    if git_text(path, &["rev-list", "--parents", "-n", "1", "HEAD"])?
        != format!("{head} {}", w.base_head)
        || body != expected
    {
        return Err(ApiError::conflict(
            "交付分支包含非 App 收口的提交，请重新执行任务",
        ));
    }
    if !changed_files(path, "HEAD")?.is_empty() {
        return Err(ApiError::conflict(
            "提交后工作目录又有变化，请重新执行验证和审查",
        ));
    }
    Ok(Some(head))
}

pub fn accept(w: &Worktree, expected_fingerprint: &str) -> ApiResult<String> {
    validate(w, false)?;
    let prepared = prepared_commit(w, expected_fingerprint)?;
    let evidence = evidence(w)?;
    if evidence.fingerprint != expected_fingerprint {
        return Err(ApiError::conflict("文件已在验证后变化，必须重新验证和审查"));
    }
    check_main(w, &evidence.files, prepared.as_deref())?;
    let path = Path::new(&w.path);
    let commit = if let Some(commit) = prepared {
        commit
    } else {
        let staged = paths(git(
            path,
            &[
                "diff",
                "--cached",
                "--name-only",
                "--no-renames",
                "-z",
                "--",
            ],
        )?)?;
        if staged.iter().any(|name| !evidence.files.contains(name)) {
            return Err(ApiError::conflict("隔离暂存区含未审查变更，请重新验证"));
        }
        let mut args = vec!["add", "--all", "--"];
        args.extend(evidence.files.iter().map(String::as_str));
        git(path, &args)?;
        let message = format!(
            "{COMMIT_SUBJECT}\n\nAgentUp-Run: {}\nAgentUp-Fingerprint: {expected_fingerprint}",
            run_id(w)?
        );
        git(path, &["commit", "-m", &message])?;
        git_text(path, &["rev-parse", "HEAD"])?
    };
    if git_text(Path::new(&w.root), &["rev-parse", "HEAD"])? != commit {
        check_main(w, &evidence.files, Some(&commit))?;
        if let Err(error) = git(
            Path::new(&w.root),
            &["merge", "--ff-only", "--no-edit", &commit],
        ) {
            return Err(ApiError::conflict(format!(
                "任务提交 {commit} 已保留在隔离分支；本地合并失败，可处理后重试接收：{}",
                error.message
            )));
        }
    }
    Ok(commit)
}

pub fn cleanup(w: &Worktree) -> ApiResult<()> {
    validate(w, false)?;
    let head = git_text(Path::new(&w.path), &["rev-parse", "HEAD"])?;
    if !git_output(
        Path::new(&w.root),
        &["merge-base", "--is-ancestor", &head, "HEAD"],
    )?
    .status
    .success()
        || !changed_files(Path::new(&w.path), "HEAD")?.is_empty()
    {
        return Err(ApiError::conflict("隔离目录仍有未接收内容，已保留"));
    }
    git(Path::new(&w.root), &["worktree", "remove", &w.path])?;
    git(Path::new(&w.root), &["branch", "-d", &w.branch])?;
    Ok(())
}

/// Verify a persisted receipt against current Git history; no surviving worktree is required.
pub fn verify_receipt(w: &Worktree, commit: &str, fingerprint: &str) -> ApiResult<()> {
    if ![40, 64].contains(&commit.len())
        || !commit.bytes().all(|b| b.is_ascii_hexdigit())
        || ![40, 64].contains(&w.base_head.len())
        || !w.base_head.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(ApiError::conflict("已接收记录的提交身份无效"));
    }
    let root = canonical_root(Path::new(&w.root))?;
    if root != Path::new(&w.root) {
        return Err(ApiError::conflict("已接收记录的项目路径已变化"));
    }
    let expected = format!(
        "{COMMIT_SUBJECT}\n\nAgentUp-Run: {}\nAgentUp-Fingerprint: {fingerprint}",
        run_id(w)?
    );
    if git_text(&root, &["log", "-1", "--format=%B", commit])? != expected
        || git_text(&root, &["rev-list", "--parents", "-n", "1", commit])?
            != format!("{commit} {}", w.base_head)
        || !git_output(&root, &["merge-base", "--is-ancestor", commit, "HEAD"])?
            .status
            .success()
    {
        return Err(ApiError::conflict(
            "已接收任务的提交不在当前项目历史中，不能作为完成依据",
        ));
    }
    Ok(())
}
