use crate::error::{ApiError, ApiResult};
use serde::Serialize;
use std::path::PathBuf;
use std::process::Stdio;

/// 本机 Agent Runtime —— 对应 PRD《02》§7 agents 表与票 44 的注册表模式。
/// 不再直连 LLM API：所有 AI 阶段（理解/方案/实施/验证/审查）通过本机已安装的
/// Agent CLI 以固定启动向量调用，从事件流回收最后一条文本。

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    /// 从 CLI 配置文件读到的模型名（读不到为 None）。
    pub model: Option<String>,
    /// 分析阶段能否强制只读（codex 可 --sandbox read-only；opencode 无沙箱）
    pub read_only_analysis: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Codex,
    Opencode,
    /// ZCode 桌面 App：优先官方发行 CLI（PATH 上的 zcode），兜底 App 内嵌 headless 入口
    /// （/Applications/ZCode.app/Contents/Resources/glm/zcode.cjs，需系统 node 执行）。
    /// 会话经 --surface desktop 呈现在桌面 App 里。
    Zcode,
}

pub struct RuntimeSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub kind: RuntimeKind,
    pub read_only_analysis: bool,
}

/// 运行时注册表：只收录本机验证过启动向量的 CLI；新增 runtime = 加一条 + 补探测测试。
pub const REGISTRY: &[RuntimeSpec] = &[
    RuntimeSpec {
        id: "codex-cli",
        name: "Codex CLI",
        description: "OpenAI Codex 命令行，支持 --sandbox 只读/写隔离，分析阶段可强制只读。",
        kind: RuntimeKind::Codex,
        read_only_analysis: true,
    },
    RuntimeSpec {
        id: "opencode",
        name: "OpenCode",
        description: "opencode run 非交互执行；无沙箱，不可强制只读。",
        kind: RuntimeKind::Opencode,
        read_only_analysis: false,
    },
    RuntimeSpec {
        id: "zcode-app",
        name: "ZCode 桌面",
        description: "ZCode 桌面 App（内嵌 CLI headless 调用，会话呈现在桌面 App 里）；无沙箱，随 App 升级需重验。",
        kind: RuntimeKind::Zcode,
        read_only_analysis: false,
    },
];

pub const DEFAULT_RUNTIME_ID: &str = "codex-cli";

fn which(binary: &str) -> Option<PathBuf> {
    // 依次探测 PATH 与常见 Homebrew 位置
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(binary);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    for base in ["/opt/homebrew/bin", "/usr/local/bin"] {
        let candidate = PathBuf::from(base).join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn binary_for(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Codex => "codex",
        RuntimeKind::Opencode => "opencode",
        RuntimeKind::Zcode => "zcode",
    }
}

fn env_override_for(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Codex => "AGENTUP_CODEX",
        RuntimeKind::Opencode => "AGENTUP_OPENCODE",
        RuntimeKind::Zcode => "AGENTUP_ZCODE",
    }
}

/// 解析 node 可执行文件（内嵌 zcode.cjs 需要）：AGENTUP_NODE → PATH → 常见安装位置。
/// Finder 启动的 App PATH 极小，必须显式兜底。
fn resolve_node() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("AGENTUP_NODE") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(p) = which("node") {
        return Some(p);
    }
    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(home).join(".local/bin/node");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    for base in ["/opt/homebrew/bin/node", "/usr/local/bin/node", "/usr/bin/node"] {
        let p = PathBuf::from(base);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// ZCode 桌面 App 内嵌 headless CLI（非发行物，随 App 升级可能漂移——所以只作兜底，
/// PATH 上出现官方 zcode CLI 时自动优先）。环境变量 AGENTUP_ZCODE 可整体覆盖。
fn embedded_zcode_cli() -> PathBuf {
    PathBuf::from("/Applications/ZCode.app/Contents/Resources/glm/zcode.cjs")
}

fn is_embedded_zcode(path: &PathBuf) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("cjs")
}

/// 解析 runtime 可执行文件：环境变量覆盖（AGENTUP_CODEX/AGENTUP_OPENCODE/AGENTUP_ZCODE）优先，
/// 其次 PATH 探测；Zcode 额外带 App 内嵌 CLI 兜底。
/// 测试环境（AGENTUP_TEST_FAKE_RUNTIME=1）返回 None 以走 mock 路径。
pub fn resolve_binary(kind: RuntimeKind) -> Option<PathBuf> {
    if std::env::var("AGENTUP_TEST_FAKE_RUNTIME").as_deref() == Ok("1") {
        return None;
    }
    if let Ok(path) = std::env::var(env_override_for(kind)) {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(p) = which(binary_for(kind)) {
        return Some(p);
    }
    match kind {
        RuntimeKind::Zcode => {
            let embedded = embedded_zcode_cli();
            if embedded.is_file() {
                Some(embedded)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 从 CLI 配置文件探测当前模型（尽力而为，读不到不报错）。
/// codex: ~/.codex/config.toml 顶层 `model = "..."`；opencode: ~/.config/opencode/opencode.json(c) 的 "model"。
fn detect_model(kind: RuntimeKind) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    match kind {
        RuntimeKind::Codex => {
            let text = std::fs::read_to_string(PathBuf::from(home).join(".codex/config.toml")).ok()?;
            for line in text.lines() {
                let line = line.trim();
                // 顶层 model 键（遇到 [section] 即停止；model_provider 等同前缀键必须精确匹配）
                if line.starts_with('[') {
                    break;
                }
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim() == "model" {
                        let value = value.trim().trim_matches('"').trim_matches('\'').trim();
                        if !value.is_empty() {
                            return Some(value.to_string());
                        }
                    }
                }
            }
            None
        }
        RuntimeKind::Opencode => {
            for candidate in ["opencode.json", "opencode.jsonc", "config.json"] {
                let path = PathBuf::from(&home).join(".config/opencode").join(candidate);
                let Ok(raw) = std::fs::read_to_string(path) else { continue };
                let cleaned = strip_jsonc(&raw);
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&cleaned) {
                    if let Some(model) = value.get("model").and_then(|m| m.as_str()) {
                        if !model.is_empty() {
                            return Some(model.to_string());
                        }
                    }
                }
            }
            None
        }
        // zcode：~/.zcode/cli/config.json 的 "model" 键（缺失时前端显示「跟随桌面 App 登录凭据」）
        RuntimeKind::Zcode => {
            let raw = std::fs::read_to_string(PathBuf::from(home).join(".zcode/cli/config.json")).ok()?;
            let cleaned = strip_jsonc(&raw);
            let value: serde_json::Value = serde_json::from_str(&cleaned).ok()?;
            value.get("model").and_then(|m| m.as_str()).map(String::from).filter(|m| !m.is_empty())
        }
    }
}

/// jsonc 宽容清洗：去掉 // 行注释与 } ] 前的尾随逗号（尽力而为，不做完整 jsonc 语法）。
fn strip_jsonc(raw: &str) -> String {
    let no_comments: String = raw
        .lines()
        .map(|l| l.split("//").next().unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n");
    let chars: Vec<char> = no_comments.chars().collect();
    let mut out = String::with_capacity(chars.len());
    for (i, c) in chars.iter().enumerate() {
        if *c == ',' {
            let next = chars[i + 1..].iter().find(|nc| !nc.is_whitespace()).copied();
            if matches!(next, Some('}') | Some(']')) {
                continue;
            }
        }
        out.push(*c);
    }
    out
}

/// 测试钩子：detect_model 读取 HOME 环境变量，集成测试用它注入临时配置目录。
pub fn detect_model_for_test(kind: RuntimeKind) -> Option<String> {
    detect_model(kind)
}

pub fn probe_runtime(spec: &RuntimeSpec) -> RuntimeInfo {
    let resolved = resolve_binary(spec.kind);
    let version = resolved.as_ref().and_then(|path| {
        std::process::Command::new(path)
            .arg("--version")
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.lines().next().unwrap_or("").trim().to_string())
            .filter(|s| !s.is_empty())
    });
    RuntimeInfo {
        id: spec.id,
        name: spec.name,
        description: spec.description,
        available: resolved.is_some(),
        path: resolved.as_ref().map(|p| p.display().to_string()),
        version,
        model: detect_model(spec.kind),
        read_only_analysis: spec.read_only_analysis,
    }
}

pub fn probe_all() -> Vec<RuntimeInfo> {
    REGISTRY.iter().map(probe_runtime).collect()
}

pub fn spec_by_id(id: &str) -> Option<&'static RuntimeSpec> {
    REGISTRY.iter().find(|s| s.id == id)
}

pub fn resolve_spec(id: Option<&str>) -> &'static RuntimeSpec {
    id.and_then(spec_by_id)
        .unwrap_or_else(|| spec_by_id(DEFAULT_RUNTIME_ID).expect("default runtime"))
}

// ---------------------------------------------------------------- 调用

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeOutcome {
    /// 事件流里最后一条文本（即模型输出）
    pub text: String,
    pub runtime_id: String,
    /// 运行时会话 id（codex thread_id / zcode sessionId / opencode sessionID），可用于续接。
    pub thread_hint: Option<String>,
    /// 事件流聚合的 token 用量；事件流未提供时为 None（不编数）。
    pub tokens: Option<i64>,
}

/// 单例的动态超时（理解/方案等长任务允许 10 分钟）。
fn spawn_timeout_secs() -> u64 {
    600
}

/// 用户取消的统一错误码（ApiError.code 复用 HTTP 语义，499 = client closed request）。
pub const CANCELLED_CODE: u16 = 499;

pub fn cancelled_error() -> ApiError {
    ApiError { code: CANCELLED_CODE, message: "已被用户取消".to_string() }
}

pub fn is_cancelled(e: &ApiError) -> bool {
    e.code == CANCELLED_CODE
}

/// 流式事件的统一形状：经 Tauri event `requirement://{id}/stream` 推给前端。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamEvent {
    /// 模型 token 级增量（仅 zcode 提供）
    Delta { text: String },
    /// 消息级完整文本（三家都有：codex item.completed / opencode text / zcode turn.completed）
    Message { text: String },
    /// 执行器/生命周期等提示行
    Notice { text: String },
}

fn extract_last_text_codex(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "item.completed" {
        return None;
    }
    let item = v.get("item")?;
    if item.get("type")?.as_str()? != "agent_message" {
        return None;
    }
    item.get("text").and_then(|t| t.as_str()).map(String::from)
}

/// codex 事件流 token 用量：`turn.completed` 事件携带 usage{input_tokens,output_tokens}；累计所有 turn。
fn extract_tokens_codex(line: &str) -> Option<i64> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type").and_then(|t| t.as_str()) != Some("turn.completed") {
        return None;
    }
    let usage = v.get("usage")?;
    let input = usage.get("input_tokens").and_then(|t| t.as_i64()).unwrap_or(0);
    let output = usage.get("output_tokens").and_then(|t| t.as_i64()).unwrap_or(0);
    Some(input + output)
}

fn extract_last_text_opencode(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "text" {
        return None;
    }
    v.pointer("/part/text").and_then(|t| t.as_str()).map(String::from)
}

/// opencode 事件流 token：结构未稳定，遇到 tokens{input,output} 形状则取，否则 None。
fn extract_tokens_opencode(line: &str) -> Option<i64> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let tokens = v.get("tokens")?;
    let input = tokens.get("input").and_then(|t| t.as_i64()).unwrap_or(0);
    let output = tokens.get("output").and_then(|t| t.as_i64()).unwrap_or(0);
    Some(input + output)
}

/// zcode stream-json：`turn.completed` 事件携带 payload.response（完整回复）与 usage 计数；
/// 事件流末行还有 `type:"result"` 终结行（同形状），两者都认——后者覆盖前者作双保险。
pub fn extract_last_text_zcode(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let event_type = v.get("type")?.as_str()?;
    match event_type {
        "turn.completed" => v.pointer("/payload/response").and_then(|t| t.as_str()).map(String::from),
        "result" => v.get("response").and_then(|t| t.as_str()).map(String::from),
        _ => None,
    }
}

/// zcode token：turn.completed / result 行的 usage.totalTokens（多 turn 累加）。
pub fn extract_tokens_zcode(line: &str) -> Option<i64> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let event_type = v.get("type")?.as_str()?;
    if event_type != "turn.completed" && event_type != "result" {
        return None;
    }
    let usage = if event_type == "result" {
        v.get("usage")?
    } else {
        v.pointer("/payload/usage")?
    };
    let input = usage.get("inputTokens").and_then(|t| t.as_i64()).unwrap_or(0);
    let output = usage.get("outputTokens").and_then(|t| t.as_i64()).unwrap_or(0);
    Some(input + output)
}

/// 以固定启动向量调用本机 runtime，prompt 经 stdin 传入，逐行消费事件流：
/// 每行解析出文本/token/会话 id，并通过 on_event 回调实时上报（供 Tauri event 推流）。
/// workdir 为项目工作目录（初始化绑定的 projects.path），None 时继承应用进程 cwd。
/// 失败（退出码非 0 / 无文本输出）返回 Err；用户取消返回 CANCELLED_CODE；调用方负责降级或报错。
/// 阻塞实现经 spawn_blocking 跑在独立线程池，不占用 tokio worker。
pub async fn invoke_streaming(
    spec: &'static RuntimeSpec,
    prompt: &str,
    read_only: bool,
    workdir: Option<&std::path::Path>,
    cancel_key: &str,
    on_event: impl Fn(StreamEvent) + Send + 'static,
) -> ApiResult<RuntimeOutcome> {
    invoke_streaming_with_timeout(spec, prompt, read_only, false, workdir, spawn_timeout_secs(), cancel_key, on_event).await
}

/// Native delivery only: enforce an explicit worktree write sandbox, never inherited yolo mode.
pub async fn invoke_isolated(
    prompt: &str,
    workdir: &std::path::Path,
    cancel_key: &str,
) -> ApiResult<RuntimeOutcome> {
    let spec = spec_by_id("codex-cli").ok_or_else(|| ApiError::internal("隔离执行器不可用"))?;
    invoke_streaming_with_timeout(spec, prompt, false, true, Some(workdir),
        spawn_timeout_secs(), cancel_key, |_| {}).await
}

/// 连通性检测：发一个最小 prompt 验证 CLI/API/模型全链路，返回（模型回复, 耗时毫秒）。60s 超时。
pub async fn check_connectivity(spec: &'static RuntimeSpec) -> ApiResult<(String, u64)> {
    let started = std::time::Instant::now();
    let outcome = invoke_streaming_with_timeout(spec, "连通性测试：请只回复两个字：正常", true, false, None, 60, "", |_| {}).await?;
    Ok((outcome.text, started.elapsed().as_millis() as u64))
}

/// 运行中调用的取消登记表：cancel key（requirement:task）→ 标志位。
/// Child 由等待侧独占持有；cancel() 只置位标志，等待侧轮询到置位即 kill 进程并返回取消错误。
/// （tokio Child 不可 Clone、无独立 kill 句柄，标志位 + 轮询是唯一无 libc 依赖的干净方案。）
static CANCEL_FLAGS: std::sync::Mutex<Option<std::collections::HashMap<String, std::sync::Arc<std::sync::atomic::AtomicBool>>>> =
    std::sync::Mutex::new(None);

const CANCEL_POLL_MS: u64 = 120;

/// 取消某需求某任务的 runtime 调用。返回是否找到登记（找到则该调用将以 CANCELLED_CODE 失败）。
pub fn cancel(requirement_id: &str, task_id: &str) -> bool {
    let key = child_key(requirement_id, task_id);
    let flags = CANCEL_FLAGS.lock().unwrap();
    match flags.as_ref().and_then(|m| m.get(&key)) {
        Some(flag) => {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            true
        }
        None => false,
    }
}

/// 推流出口：lib.rs 启动时注册 AppHandle；CLI 进程无 AppHandle，事件静默丢弃（不推流不影响链路）。
static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

pub fn set_app_handle(handle: tauri::AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

/// 单条流事件推给前端：事件通道 requirement://{requirement_id}/stream。
pub fn emit_stream(requirement_id: &str, task_id: &str, stage: &str, event: &StreamEvent) {
    use tauri::Emitter;
    if let Some(app) = APP_HANDLE.get() {
        let payload = serde_json::json!({
            "task_id": task_id,
            "stage": stage,
            "event": event,
        });
        let _ = app.emit(&format!("requirement://{requirement_id}/stream"), payload);
    }
}

fn invoke_streaming_with_timeout(
    spec: &'static RuntimeSpec,
    prompt: &str,
    read_only: bool,
    isolated_write: bool,
    workdir: Option<&std::path::Path>,
    timeout_secs: u64,
    cancel_key: &str,
    on_event: impl Fn(StreamEvent) + Send + 'static,
) -> impl std::future::Future<Output = ApiResult<RuntimeOutcome>> + Send {
    // 阻塞实现跑在独立线程（等价 spawn_blocking），经一次性通道回传结果；
    // 返回 impl Future 保持调用方 async 语义，不占用 tokio worker。
    let (tx, rx) = tokio::sync::oneshot::channel::<ApiResult<RuntimeOutcome>>();
    let prompt = prompt.to_string();
    let workdir = workdir.map(|p| p.to_path_buf());
    let cancel_key = cancel_key.to_string();
    std::thread::spawn(move || {
        let result = invoke_blocking_impl(spec, &prompt, read_only, isolated_write, workdir.as_deref(), timeout_secs, &cancel_key, on_event);
        let _ = tx.send(result);
    });
    async move {
        // oneshot 语义：线程只会 send 一次（含 panic 时 drop 触发 Disconnected）
        match rx.await {
            Ok(result) => result,
            Err(_) => Err(ApiError::internal("runtime 调用线程异常退出".to_string())),
        }
    }
}

/// 取消键的唯一出处：`"{requirement_id}:{task_id}"`。task_id 也可以是流水线阶段名（plan/implement/verify/review/understand），
/// 见 CANCEL_STAGE_KEYS。写入与取消两侧都必须经此函数拼 key，禁止各自 format!。
pub fn child_key(requirement_id: &str, task_id: &str) -> String {
    format!("{requirement_id}:{task_id}")
}

/// 无 task 粒度的流水线阶段后缀：commands 取消时逐个尝试（理解兜底也在内）。
pub const CANCEL_STAGE_KEYS: &[&str] = &["plan", "implement", "verify", "review", "understand"];

fn invoke_blocking_impl(
    spec: &RuntimeSpec,
    prompt: &str,
    read_only: bool,
    isolated_write: bool,
    workdir: Option<&std::path::Path>,
    timeout_secs: u64,
    cancel_key: &str,
    on_event: impl Fn(StreamEvent) + Send + 'static,
) -> ApiResult<RuntimeOutcome> {
    let binary = resolve_binary(spec.kind).ok_or_else(|| {
        ApiError::internal(format!(
            "本机未找到 {}（可设环境变量 {} 指定路径）",
            spec.name,
            env_override_for(spec.kind)
        ))
    })?;

    use std::io::{BufRead, BufReader, Write};
    let mut child = match spec.kind {
        RuntimeKind::Codex => {
            let mut cmd = std::process::Command::new(&binary);
            cmd.args(["exec", "--json", "--skip-git-repo-check"]);
            if read_only {
                cmd.arg("--sandbox").arg("read-only");
            } else if isolated_write {
                cmd.args(["--sandbox", "workspace-write", "-c", "approval_policy=\"never\"",
                    "-c", "sandbox_workspace_write.network_access=false",
                    "-c", "sandbox_workspace_write.writable_roots=[]"]);
            }
            #[cfg(unix)]
            if isolated_write {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
            }
            cmd.arg("-")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Some(dir) = workdir {
                cmd.current_dir(dir);
            }
            cmd.spawn().map_err(|e| ApiError::internal(format!("启动 {} 失败: {e}", spec.name)))?
        }
        RuntimeKind::Opencode => {
            let mut cmd = std::process::Command::new(&binary);
            cmd.args(["run", "--format", "json"]);
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Some(dir) = workdir {
                cmd.current_dir(dir);
            }
            cmd.spawn().map_err(|e| ApiError::internal(format!("启动 {} 失败: {e}", spec.name)))?
        }
        RuntimeKind::Zcode => {
            // prompt 经 --prompt 参数传（实测 zcode headless 不读 stdin）。
            // macOS argv 总量上限约 1MB：超长载荷直接拒绝，提示改用其他 runtime。
            if prompt.len() > 180_000 {
                return Err(ApiError::internal(format!(
                    "{} 的载荷 {}KB 超过 --prompt 参数安全上限（180KB），请精简需求或改用其他运行时",
                    spec.name,
                    prompt.len() / 1024
                )));
            }
            // 内嵌 zcode.cjs 需要 node 执行（Finder 启动的 App PATH 极小，须显式解析 node）
            let mut cmd = if is_embedded_zcode(&binary) {
                let node = resolve_node().ok_or_else(|| {
                    ApiError::internal("未找到 node（内嵌 ZCode CLI 需要），可设环境变量 AGENTUP_NODE 指定路径")
                })?;
                let mut cmd = std::process::Command::new(&node);
                cmd.arg(&binary);
                cmd
            } else {
                std::process::Command::new(&binary)
            };
            cmd.args(["--prompt", prompt, "--output-format", "stream-json", "--no-browser", "--surface", "desktop"]);
            // 分析阶段用 plan 档（规划语义，倾向只读）；实施需要真实写文件，非交互只能全自动 yolo 档
            cmd.arg("--mode").arg(if read_only { "plan" } else { "yolo" });
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Some(dir) = workdir {
                cmd.current_dir(dir);
            }
            cmd.spawn().map_err(|e| ApiError::internal(format!("启动 {} 失败: {e}", spec.name)))?
        }
    };

    // Native implementation owns a process group; no writer survives timeout/cancellation or successful exit.
    struct IsolatedProcessGroup(Option<u32>);
    impl Drop for IsolatedProcessGroup {
        fn drop(&mut self) {
            #[cfg(unix)]
            if let Some(pid) = self.0 {
                let _ = std::process::Command::new("/bin/kill").args(["-KILL", &format!("-{pid}")]).output();
            }
        }
    }
    let isolated_group = IsolatedProcessGroup(isolated_write.then_some(child.id()));

    // stderr 收尾行收集（错误归因用），独立线程防管道写满死锁
    let stderr_handle = child.stderr.take();
    let stderr_thread = std::thread::spawn(move || {
        let mut last = String::new();
        if let Some(err) = stderr_handle {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    last = line;
                }
            }
        }
        last
    });

    // stdout 逐行消费：实时事件回调 + 聚合（last_text / tokens / thread_hint）
    let stdout_handle = child.stdout.take();
    let kind = spec.kind;
    let collector_thread = std::thread::spawn(move || {
        let mut last_text = String::new();
        let mut tokens: Option<i64> = None;
        let mut thread_hint: Option<String> = None;
        if let Some(out) = stdout_handle {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                match kind {
                    RuntimeKind::Codex => {
                        if thread_hint.is_none() {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                                if v.get("type").and_then(|t| t.as_str()) == Some("thread.started") {
                                    thread_hint = v.get("thread_id").and_then(|t| t.as_str()).map(String::from);
                                }
                            }
                        }
                        if let Some(t) = extract_tokens_codex(line) {
                            tokens = Some(tokens.unwrap_or(0) + t);
                        }
                        if let Some(text) = extract_last_text_codex(line) {
                            on_event(StreamEvent::Message { text: text.clone() });
                            last_text = text;
                        }
                    }
                    RuntimeKind::Opencode => {
                        if thread_hint.is_none() {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                                thread_hint = v.get("sessionID").and_then(|t| t.as_str()).map(String::from);
                            }
                        }
                        if let Some(t) = extract_tokens_opencode(line) {
                            tokens = Some(tokens.unwrap_or(0) + t);
                        }
                        if let Some(text) = extract_last_text_opencode(line) {
                            on_event(StreamEvent::Message { text: text.clone() });
                            last_text = text;
                        }
                    }
                    RuntimeKind::Zcode => {
                        // 会话 id 在每行顶层 sessionId；text_delta 提供 token 级增量
                        if thread_hint.is_none() {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                                thread_hint = v.get("sessionId").and_then(|t| t.as_str()).map(String::from);
                            }
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                            if v.get("type").and_then(|t| t.as_str()) == Some("model.streaming") {
                                if let Some(delta) = v.pointer("/payload/delta").and_then(|t| t.as_str()) {
                                    if !delta.is_empty() {
                                        on_event(StreamEvent::Delta { text: delta.to_string() });
                                    }
                                }
                            }
                        }
                        if let Some(t) = extract_tokens_zcode(line) {
                            tokens = Some(tokens.unwrap_or(0) + t);
                        }
                        if let Some(text) = extract_last_text_zcode(line) {
                            on_event(StreamEvent::Message { text: text.clone() });
                            last_text = text;
                        }
                    }
                }
            }
        }
        (last_text, tokens, thread_hint)
    });

    // prompt 传递：codex 用 "-" 显式读 stdin；opencode 无参数时读 stdin；zcode 经 --prompt argv 传（不写 stdin，只关管道）
    match spec.kind {
        RuntimeKind::Zcode => drop(child.stdin.take()),
        _ => {
            if let Some(stdin) = child.stdin.as_mut() {
                if let Err(e) = stdin.write_all(prompt.as_bytes()) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ApiError::internal(format!("写入 {} stdin 失败: {e}", spec.name)));
                }
            }
            drop(child.stdin.take());
        }
    }

    // 等待循环：退出状态 / 超时 / 取消标志（120ms 轮询，与旧实现同节奏）
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let cancel_flag = if cancel_key.is_empty() {
        None
    } else {
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        CANCEL_FLAGS
            .lock()
            .unwrap()
            .get_or_insert_with(std::collections::HashMap::new)
            .insert(cancel_key.to_string(), flag.clone());
        Some(flag)
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if let Some(flag) = &cancel_flag {
                    if flag.load(std::sync::atomic::Ordering::SeqCst) {
                        let _ = child.kill();
                        let _ = child.wait();
                        // 清登记，避免泄漏
                        CANCEL_FLAGS.lock().unwrap().as_mut().map(|m| m.remove(cancel_key));
                        return Err(cancelled_error());
                    }
                }
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    CANCEL_FLAGS.lock().unwrap().as_mut().map(|m| m.remove(cancel_key));
                    return Err(ApiError::internal(format!("runtime 执行超时（{timeout_secs}s）")));
                }
                std::thread::sleep(std::time::Duration::from_millis(CANCEL_POLL_MS));
            }
            Err(e) => {
                CANCEL_FLAGS.lock().unwrap().as_mut().map(|m| m.remove(cancel_key));
                return Err(ApiError::internal(format!("等待 {} 退出失败: {e}", spec.name)));
            }
        }
    };

    drop(isolated_group);
    let (last_text, tokens, thread_hint) = collector_thread
        .join()
        .map_err(|_| ApiError::internal("runtime 事件流收集线程崩溃".to_string()))?
        ;
    let stderr_last = stderr_thread.join().unwrap_or_default();
    CANCEL_FLAGS.lock().unwrap().as_mut().map(|m| m.remove(cancel_key));

    if !status.success() {
        let brief = readable_stderr(&stderr_last);
        return Err(ApiError::internal(format!(
            "{} 退出码 {:?}: {}",
            spec.name,
            status.code(),
            brief
        )));
    }
    if last_text.trim().is_empty() {
        return Err(ApiError::internal(format!("{} 未返回文本结果（事件流为空）", spec.name)));
    }
    Ok(RuntimeOutcome { text: last_text, runtime_id: spec.id.to_string(), thread_hint, tokens })
}

fn readable_stderr(last_line: &str) -> String {
    crate::error::readable_error(last_line).chars().take(300).collect()
}
