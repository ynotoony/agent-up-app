// Input: 用户选择的本机目录、只读预览指纹与明确确认。
// Output: 三类仓库接入计划；确认后仅补 Git、空任务索引和项目档案并登记本机项目。
// Pos: 接入边界；不生成任务、不启动 Agent、不覆盖已有资料，src-tauri/ 登记豁免。
use crate::error::{ApiError, ApiResult};
use crate::project_discovery::{
    GitDiscovery, ImportProjectResult, ManagementSource, ProjectDiscovery, ProjectImport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::{
    ffi::{OsStrExt, OsStringExt},
    fs::OpenOptionsExt,
};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;

const INDEX: &str = "facts/requirements/tickets/index.json";
const TICKETS_README: &str = "facts/requirements/tickets/README.md";
const CHANGES_LEDGER: &str = "facts/project/changes.jsonl";
const INDEX_PATHS: [&str; 3] = [
    INDEX,
    "docs/issues/index.json",
    "docs/project-management-mvp/issues/index.json",
];
static ONBOARDING: Mutex<()> = Mutex::new(());

#[derive(Debug, Serialize)]
pub struct WorkspaceWrite {
    pub path: String,
    pub action: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct WorkspacePreview {
    pub path: String,
    pub name: String,
    pub mode: String,
    pub git: Option<GitDiscovery>,
    pub index_path: Option<String>,
    pub discovery: Option<ProjectDiscovery>,
    pub writes: Vec<WorkspaceWrite>,
    pub fingerprint: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfirmInput {
    pub path: String,
    pub fingerprint: String,
}

fn git(root: &ProjectRoot, args: &[&str]) -> ApiResult<Output> {
    let mut command = Command::new("git");
    command
        .args(["--no-optional-locks", "-c", "core.fsmonitor=false"])
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
        command.env_remove(key);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let directory_fd = root.directory.as_raw_fd();
        unsafe {
            command.pre_exec(move || {
                if libc::fchdir(directory_fd) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        }
    }
    #[cfg(not(unix))]
    command.current_dir(&root.path);
    command
        .output()
        .map_err(|error| ApiError::internal(format!("无法运行 Git：{error}")))
}

fn git_metadata(root: &ProjectRoot, is_git: bool) -> ApiResult<Option<GitDiscovery>> {
    if !is_git {
        return Ok(None);
    }
    let branch = git(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    let head = git(root, &["rev-parse", "--verify", "HEAD"])?;
    let status = git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=all",
            "--",
            ".",
        ],
    )?;
    if !status.status.success() {
        return Err(ApiError::bad_request("无法读取 Git 工作区状态"));
    }
    Ok(Some(GitDiscovery {
        branch: branch
            .status
            .success()
            .then(|| String::from_utf8_lossy(&branch.stdout).trim().to_owned())
            .filter(|value| !value.is_empty()),
        head: head
            .status
            .success()
            .then(|| String::from_utf8_lossy(&head.stdout).trim().to_owned())
            .filter(|value| !value.is_empty()),
        dirty: !status.stdout.is_empty(),
    }))
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
                "当前平台无法安全接入目录；为避免路径竞态，已拒绝操作",
            ))
        }
    }

    fn open_entry(&self, relative: &str) -> ApiResult<Option<File>> {
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ApiError::bad_request("接入路径无效"));
        }
        #[cfg(unix)]
        {
            let names: Vec<CString> = path
                .components()
                .filter_map(|component| match component {
                    Component::Normal(name) => Some(
                        CString::new(name.as_bytes())
                            .map_err(|_| ApiError::bad_request("接入路径包含 NUL 字符")),
                    ),
                    _ => None,
                })
                .collect::<ApiResult<_>>()?;
            if names.is_empty() {
                return Err(ApiError::bad_request("接入路径不能为空"));
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
                    return Err(source_error(relative, error));
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
                return Err(source_error(relative, error));
            }
            Ok(Some(unsafe { File::from_raw_fd(fd) }))
        }
        #[cfg(not(unix))]
        {
            let mut absolute = self.path.clone();
            for component in path.components() {
                absolute.push(component);
                match fs::symlink_metadata(&absolute) {
                    Ok(info) if info.file_type().is_symlink() => {
                        return Err(ApiError::bad_request(format!(
                            "来源为符号链接，不能接入：{relative}"
                        )))
                    }
                    Ok(_) => (),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => return Err(source_error(relative, error)),
                }
            }
            Ok(Some(File::open(absolute)?))
        }
    }

    fn metadata(&self, relative: &str) -> ApiResult<Option<fs::Metadata>> {
        self.open_entry(relative)?
            .map(|file| file.metadata().map_err(Into::into))
            .transpose()
    }

    fn still_at_path(&self) -> ApiResult<bool> {
        let path_metadata = match fs::symlink_metadata(&self.path) {
            Ok(metadata) if !metadata.file_type().is_symlink() => metadata,
            _ => return Ok(false),
        };
        let opened_metadata = self.directory_metadata()?;
        #[cfg(unix)]
        {
            Ok(path_metadata.dev() == opened_metadata.dev()
                && path_metadata.ino() == opened_metadata.ino())
        }
        #[cfg(not(unix))]
        {
            let _ = (path_metadata, opened_metadata);
            Ok(fs::canonicalize(&self.path).is_ok_and(|path| path == self.path))
        }
    }

    fn directory_metadata(&self) -> ApiResult<fs::Metadata> {
        #[cfg(unix)]
        {
            Ok(self.directory.metadata()?)
        }
        #[cfg(not(unix))]
        {
            Ok(fs::metadata(&self.path)?)
        }
    }

    fn root_entries(
        &self,
        limit: usize,
    ) -> ApiResult<Vec<(String, u64, bool, Option<std::time::SystemTime>)>> {
        #[cfg(unix)]
        {
            struct Directory(*mut libc::DIR);
            impl Drop for Directory {
                fn drop(&mut self) {
                    unsafe {
                        libc::closedir(self.0);
                    }
                }
            }
            let duplicate = unsafe { libc::dup(self.directory.as_raw_fd()) };
            if duplicate < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let stream = unsafe { libc::fdopendir(duplicate) };
            if stream.is_null() {
                unsafe {
                    libc::close(duplicate);
                }
                return Err(std::io::Error::last_os_error().into());
            }
            let directory = Directory(stream);
            let mut entries = Vec::new();
            loop {
                let entry = unsafe { libc::readdir(directory.0) };
                if entry.is_null() {
                    break;
                }
                let bytes =
                    unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
                if bytes == b"." || bytes == b".." {
                    continue;
                }
                if entries.len() >= limit {
                    return Err(ApiError::bad_request("目录条目过多，请选择具体项目目录"));
                }
                let name_c =
                    CString::new(bytes).map_err(|_| ApiError::bad_request("目录名称无效"))?;
                let fd = unsafe {
                    libc::openat(
                        self.directory.as_raw_fd(),
                        name_c.as_ptr(),
                        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                    )
                };
                if fd < 0 {
                    let error = std::io::Error::last_os_error();
                    if matches!(
                        error.raw_os_error(),
                        Some(libc::ELOOP) | Some(libc::ENOTDIR)
                    ) {
                        continue;
                    }
                    return Err(error.into());
                }
                let file = unsafe { File::from_raw_fd(fd) };
                let metadata = file.metadata()?;
                entries.push((
                    std::ffi::OsString::from_vec(bytes.to_vec())
                        .to_string_lossy()
                        .into_owned(),
                    metadata.len(),
                    metadata.is_dir(),
                    metadata.modified().ok(),
                ));
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(entries)
        }
        #[cfg(not(unix))]
        {
            let mut entries = Vec::new();
            for (count, entry) in fs::read_dir(&self.path)?.enumerate() {
                if count >= limit {
                    return Err(ApiError::bad_request("目录条目过多，请选择具体项目目录"));
                }
                let entry = entry?;
                let metadata = entry.metadata()?;
                entries.push((
                    entry.file_name().to_string_lossy().into_owned(),
                    metadata.len(),
                    metadata.is_dir(),
                    metadata.modified().ok(),
                ));
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(entries)
        }
    }

    fn directory_entries(&self, relative: &str, limit: usize) -> ApiResult<Option<Vec<String>>> {
        let Some(directory) = self.open_entry(relative)? else {
            return Ok(None);
        };
        if !directory.metadata()?.is_dir() {
            return Err(ApiError::bad_request(format!(
                "治理标记路径不是目录：{relative}"
            )));
        }
        #[cfg(unix)]
        {
            struct Directory(*mut libc::DIR);
            impl Drop for Directory {
                fn drop(&mut self) {
                    unsafe {
                        libc::closedir(self.0);
                    }
                }
            }
            let duplicate = unsafe { libc::dup(directory.as_raw_fd()) };
            if duplicate < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let stream = unsafe { libc::fdopendir(duplicate) };
            if stream.is_null() {
                unsafe {
                    libc::close(duplicate);
                }
                return Err(std::io::Error::last_os_error().into());
            }
            let directory = Directory(stream);
            let mut entries = Vec::new();
            loop {
                let entry = unsafe { libc::readdir(directory.0) };
                if entry.is_null() {
                    break;
                }
                let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
                if name == b"." || name == b".." {
                    continue;
                }
                if entries.len() >= limit {
                    return Err(ApiError::bad_request(format!(
                        "治理标记目录条目过多，无法安全判定：{relative}"
                    )));
                }
                entries.push(
                    std::ffi::OsString::from_vec(name.to_vec())
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            entries.sort();
            Ok(Some(entries))
        }
        #[cfg(not(unix))]
        {
            let mut entries = Vec::new();
            for (count, entry) in fs::read_dir(self.path.join(relative))?.enumerate() {
                if count >= limit {
                    return Err(ApiError::bad_request(format!(
                        "治理标记目录条目过多，无法安全判定：{relative}"
                    )));
                }
                entries.push(entry?.file_name().to_string_lossy().into_owned());
            }
            entries.sort();
            Ok(Some(entries))
        }
    }

    fn read_regular(&self, relative: &str, limit: u64) -> ApiResult<Option<Vec<u8>>> {
        let Some(file) = self.open_entry(relative)? else {
            return Ok(None);
        };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > limit {
            return Err(ApiError::bad_request(format!(
                "来源不是小型普通文件：{relative}"
            )));
        }
        let mut bytes = Vec::new();
        file.take(limit + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(ApiError::bad_request(format!(
                "来源超过大小限制：{relative}"
            )));
        }
        Ok(Some(bytes))
    }

    fn ensure_directory(&self, relative: &str) -> ApiResult<File> {
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ApiError::bad_request("接入目录路径无效"));
        }
        #[cfg(unix)]
        {
            let mut current = self.directory.try_clone()?;
            for component in path.components() {
                let Component::Normal(name) = component else {
                    unreachable!()
                };
                let name = CString::new(name.as_bytes())
                    .map_err(|_| ApiError::bad_request("接入目录包含 NUL 字符"))?;
                let made = unsafe { libc::mkdirat(current.as_raw_fd(), name.as_ptr(), 0o755) };
                if made < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err(source_error(relative, error));
                    }
                }
                let fd = unsafe {
                    libc::openat(
                        current.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(source_error(relative, std::io::Error::last_os_error()));
                }
                current = unsafe { File::from_raw_fd(fd) };
            }
            Ok(current)
        }
        #[cfg(not(unix))]
        {
            let target = self.path.join(path);
            let mut current = self.path.clone();
            for component in path.components() {
                let Component::Normal(name) = component else {
                    unreachable!()
                };
                current.push(name);
                match fs::symlink_metadata(&current) {
                    Ok(info) if info.is_dir() && !info.file_type().is_symlink() => (),
                    Ok(_) => return Err(ApiError::bad_request("接入目录包含符号链接或非目录")),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        fs::create_dir(&current)?;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            File::open(target).map_err(Into::into)
        }
    }

    fn create_empty_file(&self, relative: &str, contents: &[u8]) -> ApiResult<()> {
        let path = Path::new(relative);
        let parent = path
            .parent()
            .and_then(Path::to_str)
            .filter(|parent| !parent.is_empty())
            .ok_or_else(|| ApiError::bad_request("接入文件缺少父目录"))?;
        let directory = self.ensure_directory(parent)?;
        let filename = path
            .file_name()
            .ok_or_else(|| ApiError::bad_request("接入文件名无效"))?;
        #[cfg(unix)]
        {
            let final_name = CString::new(filename.as_bytes())
                .map_err(|_| ApiError::bad_request("接入文件名包含 NUL 字符"))?;
            let temporary_name = CString::new(format!(".index.tmp-{}", uuid::Uuid::new_v4()))
                .expect("UUID has no NUL");
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    temporary_name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let mut file = unsafe { File::from_raw_fd(fd) };
            let result = (|| -> ApiResult<()> {
                file.write_all(contents)?;
                file.sync_all()?;
                if unsafe {
                    libc::linkat(
                        directory.as_raw_fd(),
                        temporary_name.as_ptr(),
                        directory.as_raw_fd(),
                        final_name.as_ptr(),
                        0,
                    )
                } < 0
                {
                    return Err(ApiError::conflict(format!(
                        "接入文件已变化或无法创建，请重新预览：{}",
                        std::io::Error::last_os_error()
                    )));
                }
                if unsafe { libc::unlinkat(directory.as_raw_fd(), temporary_name.as_ptr(), 0) } < 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
                directory.sync_all()?;
                Ok(())
            })();
            if result.is_err() {
                unsafe { libc::unlinkat(directory.as_raw_fd(), temporary_name.as_ptr(), 0) };
            }
            result
        }
        #[cfg(not(unix))]
        {
            let target = self.path.join(path);
            let temporary = target.with_file_name(format!(".index.tmp-{}", uuid::Uuid::new_v4()));
            let result = (|| -> ApiResult<()> {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)?;
                file.write_all(contents)?;
                file.sync_all()?;
                fs::hard_link(&temporary, &target).map_err(|error| {
                    ApiError::conflict(format!("接入文件已变化或无法创建：{error}"))
                })?;
                fs::remove_file(&temporary)?;
                Ok(())
            })();
            if result.is_err() {
                let _ = fs::remove_file(temporary);
            }
            result
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
            return Err(ApiError::conflict(format!(
                "项目目录在打开期间发生变化：{}",
                std::io::Error::last_os_error()
            )));
        }
        directory = unsafe { File::from_raw_fd(fd) };
    }
    Ok(directory)
}

fn source_error(relative: &str, error: std::io::Error) -> ApiError {
    #[cfg(unix)]
    if matches!(
        error.raw_os_error(),
        Some(libc::ELOOP) | Some(libc::ENOTDIR)
    ) {
        return ApiError::bad_request(format!(
            "来源包含符号链接或非目录组件，不能接入：{relative}"
        ));
    }
    ApiError::bad_request(format!("无法读取接入路径 {relative}：{error}"))
}

fn metadata(root: &ProjectRoot, relative: &str) -> ApiResult<Option<fs::Metadata>> {
    root.metadata(relative)
}

fn unrecognized_task_material(root: &ProjectRoot) -> ApiResult<Option<String>> {
    // These paths are specific task/governance locations. Generic docs and facts
    // files are deliberately ignored to avoid classifying ordinary project prose.
    const TASK_DIRECTORIES: [&str; 8] = [
        "facts/requirements/tickets",
        "docs/issues",
        "docs/project-management-mvp/issues",
        "tasks",
        "tickets",
        "issues",
        "docs/tasks",
        "docs/tickets",
    ];
    for directory in TASK_DIRECTORIES {
        let Some(entries) = root.directory_entries(directory, 4096)? else {
            continue;
        };
        if let Some(entry) = entries
            .into_iter()
            .find(|entry| entry != ".DS_Store" && entry != ".gitkeep")
        {
            return Ok(Some(format!("{directory}/{entry}")));
        }
    }
    for marker in [
        TICKETS_README,
        CHANGES_LEDGER,
        "facts/requirements/tickets/progress-current.md",
    ] {
        if metadata(root, marker)?.is_some() {
            return Ok(Some(marker.to_owned()));
        }
    }
    Ok(None)
}

pub fn preview(path: &str) -> ApiResult<WorkspacePreview> {
    let path = Path::new(path)
        .canonicalize()
        .map_err(|_| ApiError::bad_request("请选择存在且可读取的项目目录"))?;
    if !path.is_dir() {
        return Err(ApiError::bad_request("项目路径不是目录"));
    }
    let root = ProjectRoot::open(path)?;
    let canonical = root
        .path
        .to_str()
        .ok_or_else(|| ApiError::bad_request("项目路径编码不受支持"))?
        .to_owned();
    let probe = git(&root, &["rev-parse", "--is-inside-work-tree"])?;
    let is_git = probe.status.success() && String::from_utf8_lossy(&probe.stdout).trim() == "true";
    if !is_git {
        let bare = git(&root, &["rev-parse", "--is-bare-repository"])?;
        if bare.status.success() && String::from_utf8_lossy(&bare.stdout).trim() == "true" {
            return Err(ApiError::bad_request(
                "请选择带工作目录的 Git 仓库；裸仓库不能接入任务工作台",
            ));
        }
    }
    if is_git {
        let prefix = git(&root, &["rev-parse", "--show-prefix"])?;
        if !prefix.status.success() || !String::from_utf8_lossy(&prefix.stdout).trim().is_empty() {
            return Err(ApiError::bad_request(
                "请选择 Git 仓库根目录；不能在子目录中嵌套接入",
            ));
        }
    }
    if !is_git && root.metadata(".git")?.is_some() {
        return Err(ApiError::bad_request(
            "目录已有无法读取的 .git；请修复仓库后重试，不会覆盖",
        ));
    }
    let git_metadata = git_metadata(&root, is_git)?;
    let mut index_path = None;
    let mut source_hashes = Vec::new();
    for relative in INDEX_PATHS {
        if let Some(info) = metadata(&root, relative)? {
            if !info.is_file() || info.len() > 2 * 1024 * 1024 {
                return Err(ApiError::bad_request(format!(
                    "任务索引不是可读取的小型文件：{relative}"
                )));
            }
            let bytes = root
                .read_regular(relative, 2 * 1024 * 1024)?
                .ok_or_else(|| ApiError::conflict(format!("来源在扫描期间消失：{relative}")))?;
            let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
                ApiError::bad_request(format!("任务索引 JSON 无效：{relative}；请修复后接入"))
            })?;
            if !value.get("issues").is_some_and(serde_json::Value::is_array) {
                return Err(ApiError::bad_request(format!(
                    "任务索引缺少 issues 数组：{relative}；不会创建替代索引"
                )));
            }
            if index_path.is_none() {
                index_path = Some(relative.to_owned());
            }
            source_hashes.push((relative, format!("{:x}", Sha256::digest(&bytes))));
        }
    }
    if index_path.is_none() {
        if let Some(material) = unrecognized_task_material(&root)? {
            return Err(ApiError::bad_request(format!(
                "检测到现有任务或治理资料：{material}，但没有受支持的任务索引。为避免把现有工作误报为空项目或创建替代索引，已停止接入；请先确认资料归属及唯一索引后重新预览。"
            )));
        }
    }
    let formal_ticket_capability = index_path
        .as_deref()
        .is_none_or(|relative| relative == INDEX);
    let mut ticket_writes = Vec::new();
    if formal_ticket_capability {
        for (relative, description) in [
            (
                TICKETS_README,
                "创建票务目录清单，供正式开票入口登记新任务。",
            ),
            (CHANGES_LEDGER, "创建追加式项目变更账本，不覆盖现有记录。"),
        ] {
            match root.read_regular(relative, 2 * 1024 * 1024)? {
                Some(bytes) => {
                    if relative == TICKETS_README {
                        let text = std::str::from_utf8(&bytes).map_err(|_| {
                            ApiError::bad_request("已有任务 README 不是有效 UTF-8；不会覆盖")
                        })?;
                        if !text.lines().any(|line| line.starts_with("| `")) {
                            return Err(ApiError::bad_request(
                                "已有任务 README 缺少目录清单表行，ticket-ops 无法安全登记；不会覆盖",
                            ));
                        }
                    }
                    source_hashes.push((relative, format!("{:x}", Sha256::digest(&bytes))));
                }
                None => ticket_writes.push(WorkspaceWrite {
                    path: relative.into(),
                    action: "create_file".into(),
                    description: description.into(),
                }),
            }
        }
    }
    if let Some(info) = metadata(&root, "facts/requirements/tickets/progress-current.md")? {
        if !info.is_file() || info.len() > 2 * 1024 * 1024 {
            return Err(ApiError::bad_request(
                "已有进度投影不是可读取的小型普通文件；不会覆盖",
            ));
        }
        let bytes = root
            .read_regular(
                "facts/requirements/tickets/progress-current.md",
                2 * 1024 * 1024,
            )?
            .ok_or_else(|| ApiError::conflict("进度投影在扫描期间消失"))?;
        source_hashes.push((
            "facts/requirements/tickets/progress-current.md",
            format!("{:x}", Sha256::digest(&bytes)),
        ));
    }
    let existing_profile = metadata(&root, ".agentup-app/project.json")?;
    if !is_git && existing_profile.is_some() {
        return Err(ApiError::bad_request(
            "此目录已有项目档案但 Git 不可用，请先恢复仓库",
        ));
    }
    let entries = root.root_entries(4096)?;
    let mut writes = Vec::new();
    if !is_git {
        writes.push(WorkspaceWrite {
            path: ".git/".into(),
            action: "initialize_git".into(),
            description: "初始化本地 Git 仓库，主分支 main；不创建提交".into(),
        });
    }
    if index_path.is_none() {
        writes.push(WorkspaceWrite {
            path: INDEX.into(),
            action: "create_file".into(),
            description: "建立空任务索引（issues: []），真实任务由你与 Agent 定义".into(),
        });
    }
    writes.extend(ticket_writes);
    if existing_profile.is_none() {
        writes.push(WorkspaceWrite {
            path: ".agentup-app/project.json".into(),
            action: "create_file".into(),
            description: "保存项目标识与已有来源引用".into(),
        });
    }
    writes.push(WorkspaceWrite {
        path: "本机项目列表".into(),
        action: "register_local".into(),
        description: "登记目录，打开真实任务工作台".into(),
    });
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            &canonical,
            is_git,
            &git_metadata,
            &source_hashes,
            &entries,
            &writes
        ))?)
    );
    let warnings = Vec::new();
    Ok(WorkspacePreview {
        name: root
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("项目")
            .to_owned(),
        path: canonical,
        mode: if !is_git {
            "new"
        } else if index_path.is_some() {
            "governed"
        } else {
            "git"
        }
        .into(),
        git: git_metadata,
        index_path,
        discovery: None,
        writes,
        fingerprint,
        warnings,
    })
}

fn create_index(root: &ProjectRoot) -> ApiResult<()> {
    root.create_empty_file(INDEX, b"{\n  \"issues\": []\n}\n")
}

fn tickets_readme() -> &'static [u8] {
    const CONTENT: &str = "<!-- Input: 用户确认的任务草案、依赖边与项目范围。 -->\n<!-- Output: 本项目的真实任务票、状态索引与目录清单。 -->\n<!-- Pos: 项目任务票目录与管理入口。 -->\n\n# 任务票\n\n任务正文以 JSON 文件保存，`index.json` 是状态真相源。只有经由用户确认的任务才落盘。\n\n## 目录清单\n\n| 名字 | 地位 | 功能 |\n| --- | --- | --- |\n| `index.json` | 状态真相源 | 任务状态、依赖与复查指针。 |\n| `README.md` | 目录 | 任务票目录。 |\n| `progress-current.md` | 投影 | 由索引生成的现役状态视图。 |\n";
    CONTENT.as_bytes()
}

fn ensure_profile(root: &ProjectRoot, preview: &WorkspacePreview) -> ApiResult<ProjectImport> {
    if let Some(bytes) = root.read_regular(".agentup-app/project.json", 2 * 1024 * 1024)? {
        let profile: ProjectImport = serde_json::from_slice(&bytes)
            .map_err(|_| ApiError::bad_request("已有项目档案无法识别，不会覆盖"))?;
        if profile.schema_version != 1
            || uuid::Uuid::parse_str(&profile.id).is_err()
            || profile.name.trim().is_empty()
            || chrono::DateTime::parse_from_rfc3339(&profile.created_at).is_err()
        {
            return Err(ApiError::bad_request("已有项目档案内容无效，不会覆盖"));
        }
        return Ok(profile);
    }
    let name = root
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("项目目录名无法识别"))?
        .to_owned();
    let sources = match preview.index_path.as_deref() {
        Some(relative) => {
            let bytes = root
                .read_regular(relative, 2 * 1024 * 1024)?
                .ok_or_else(|| ApiError::conflict(format!("任务索引在接入期间消失：{relative}")))?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|_| ApiError::bad_request(format!("任务索引 JSON 无效：{relative}")))?;
            let rows = value
                .get("issues")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| {
                    ApiError::bad_request(format!("任务索引缺少 issues 数组：{relative}"))
                })?;
            let mut source = ManagementSource {
                path: relative.into(),
                kind: "ticket_index".into(),
                category: "unknown".into(),
                item_count: rows.len(),
                current_count: 0,
                history_count: 0,
                unknown_count: 0,
                fingerprint: format!("{:x}", Sha256::digest(&bytes)),
            };
            for row in rows {
                let id_ok = row
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| !id.trim().is_empty());
                match (id_ok, row.get("status").and_then(serde_json::Value::as_str)) {
                    (
                        true,
                        Some(
                            "ready" | "ready-for-agent" | "in_progress" | "blocked"
                            | "review_ready" | "review_pass" | "review_fail" | "pending",
                        ),
                    ) => source.current_count += 1,
                    (true, Some("done" | "superseded")) => source.history_count += 1,
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
            vec![source]
        }
        None => vec![],
    };
    let profile = ProjectImport {
        schema_version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        name,
        created_at: chrono::Utc::now().to_rfc3339(),
        sources,
    };
    let mut bytes = serde_json::to_vec_pretty(&profile)?;
    bytes.push(b'\n');
    root.create_empty_file(".agentup-app/project.json", &bytes)?;
    Ok(profile)
}

// Fingerprint revalidation precedes all writes. Exclusive creation never replaces
// user files. If a later step fails, preserved partial setup is visible on rescan.
pub fn confirm(input: &WorkspaceConfirmInput) -> ApiResult<(String, ProjectImport)> {
    let _guard = ONBOARDING
        .lock()
        .map_err(|_| ApiError::internal("项目接入暂时不可用"))?;
    let fresh = preview(&input.path)?;
    if fresh.fingerprint != input.fingerprint {
        return Err(ApiError::conflict("目录在预览后发生变化，请重新预览并确认"));
    }
    let root = ProjectRoot::open(PathBuf::from(&fresh.path))?;
    if fresh.git.is_none() {
        let result = git(
            &root,
            &["init", "--quiet", "--template=", "--initial-branch=main"],
        )?;
        if !result.status.success() {
            return Err(ApiError::internal(format!(
                "Git 初始化失败：{}",
                String::from_utf8_lossy(&result.stderr)
            )));
        }
    }
    if fresh.index_path.is_none() {
        create_index(&root)?;
    }
    for (relative, contents) in [
        (TICKETS_README, tickets_readme().to_vec()),
        (CHANGES_LEDGER, Vec::new()),
    ] {
        if fresh.writes.iter().any(|write| write.path == relative) {
            root.create_empty_file(relative, &contents)?;
        }
    }
    if !root.still_at_path()? {
        return Err(ApiError::conflict(
            "项目目录在接入期间被移动或替换，请重新预览",
        ));
    }
    let profile = ensure_profile(&root, &fresh)?;
    if !root.still_at_path()? {
        return Err(ApiError::conflict(
            "项目目录在接入期间被移动或替换，请重新预览",
        ));
    }
    Ok((fresh.path, profile))
}

#[tauri::command]
pub async fn workspace_onboarding_preview(path: String) -> ApiResult<WorkspacePreview> {
    tauri::async_runtime::spawn_blocking(move || preview(&path))
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn workspace_onboarding_confirm(
    state: tauri::State<'_, std::sync::Arc<crate::db::AppState>>,
    input: WorkspaceConfirmInput,
) -> ApiResult<ImportProjectResult> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (path, profile) = confirm(&input)?;
        let conn = state
            .conn
            .lock()
            .map_err(|_| ApiError::internal("项目列表暂时不可用"))?;
        let project =
            crate::db::register_imported_project(&conn, &profile.id, &profile.name, &path)?;
        Ok(ImportProjectResult { project, profile })
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    #[test]
    fn index_creation_stays_inside_open_root_after_path_replacement() {
        let parent = TempDir::new().unwrap();
        let root_path = parent.path().join("project");
        fs::create_dir(&root_path).unwrap();
        let root = ProjectRoot::open(root_path.clone()).unwrap();

        let moved = parent.path().join("moved-project");
        fs::rename(&root_path, &moved).unwrap();
        let outside = parent.path().join("attacker");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, &root_path).unwrap();

        create_index(&root).unwrap();
        assert!(moved.join(INDEX).is_file());
        assert!(!outside.join("facts").exists());
        assert!(!root.still_at_path().unwrap());
    }

    #[test]
    fn index_creation_never_follows_or_replaces_existing_index_symlink() {
        let parent = TempDir::new().unwrap();
        let root_path = parent.path().join("project");
        fs::create_dir_all(root_path.join("facts/requirements/tickets")).unwrap();
        let outside = parent.path().join("outside.json");
        fs::write(&outside, b"preserve").unwrap();
        symlink(
            &outside,
            root_path.join("facts/requirements/tickets/index.json"),
        )
        .unwrap();
        let root = ProjectRoot::open(root_path).unwrap();
        assert!(create_index(&root).is_err());
        assert_eq!(fs::read(outside).unwrap(), b"preserve");
    }
}
