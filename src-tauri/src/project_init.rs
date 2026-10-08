use crate::db::{self, AppState};
use crate::error::{ApiError, ApiResult};
use crate::types::{now_iso, GovernanceItem, InitReport, InitStep, ProjectDoc};
use rusqlite::params;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// 项目初始化引擎 —— 目录绑定、存量文档扫描入库、治理种子（app 替代 agent-up 技能）。
/// 铁律：目录零污染（只建目录 + git init，绝不写治理文件）；扫描只读；数据在库。

pub const SCAN_MAX_FILES: usize = 200;
pub const EXCERPT_CHARS: usize = 500;
pub const CONTENT_MAX_BYTES: usize = 256 * 1024;
/// 递归深度保护（正常仓库远达不到；防符号链接/病态嵌套）。
const SCAN_MAX_DEPTH: usize = 6;

const IGNORED_DIRS: &[&str] = &[
    ".git", "node_modules", "dist", "build", "target", ".next", "vendor", "coverage", "out",
    ".venv", "__pycache__", ".idea", ".vscode",
];
/// 需求类文档分类关键词（路径或文件名 contains 命中）——仅作展示分组提示，不影响是否转需求。
const REQUIREMENT_KEYWORDS: &[&str] = &[
    "prd", "requirement", "spec", "rfc", "需求", "方案", "设计", "proposal", "todo",
];
const TEXT_EXTS: &[&str] = &["md", "markdown", "txt"];
const BINARY_DOC_EXTS: &[&str] = &["pdf", "docx"];
/// 高风险面（agent-up complexity-profile §2.2 定稿，受控枚举）。
pub const RISK_SURFACES: &[&str] = &["数据迁移兼容", "认证授权", "安全隐私", "外部服务", "发布运行可靠性"];
/// Profile 七维（D/B/I/U/S/M/O）。
pub const PROFILE_DIMENSIONS: &[&str] = &["D", "B", "I", "U", "S", "M", "O"];

// ---------------------------------------------------------------- 报告结构

pub(crate) fn step(item: &str, action: &str, detail: impl Into<String>) -> InitStep {
    InitStep { item: item.into(), action: action.into(), detail: detail.into() }
}

// ---------------------------------------------------------------- 初始化 / 重新初始化（命令层薄封装调用）

/// 初始化目录为项目：查重 → 目录准备 → 扫描入库 → 治理种子 → 登记。
pub fn init_project(state: &AppState, path_input: &str) -> ApiResult<crate::types::InitProjectOutcome> {
    let root = normalize_path(path_input)?;
    let path_str = root.display().to_string();

    // 已登记：直接返回已有项目（幂等，与「选中即进入」心智一致）
    {
        let conn = state.conn.lock().unwrap();
        if let Some(existing) = db::find_project_by_path(&conn, &path_str)? {
            let name = existing.name.clone();
            let mut report = InitReport { path: path_str, name, ..Default::default() };
            report.steps.push(step("项目", "kept", "该目录已登记为项目，直接打开"));
            return Ok(crate::types::InitProjectOutcome { project: existing, report, already_registered: true });
        }
    }

    let mut report = InitReport::default();
    prepare_directory(&root, &mut report)?;

    // 登记与入库（锁内完成；双检防并发重复登记）
    let (project, report) = {
        let conn = state.conn.lock().unwrap();
        let project = match db::find_project_by_path(&conn, &path_str)? {
            Some(existing) => existing,
            None => db::create_project_full(&conn, &basename(&root), None, Some(&path_str))?,
        };
        let (added, updated, missing) = sync_project_docs(&conn, &project.id, &root)?;
        let requirement_docs = db::list_project_docs(&conn, &project.id)?
            .iter()
            .filter(|d| d.kind == "requirement" && !d.file_missing)
            .count();
        let (seeded, _) = seed_governance(&conn, &project.id, &root, requirement_docs)?;
        let pending_open = db::list_governance_items(&conn, &project.id, Some("pending"))?
            .iter()
            .filter(|i| i.status == "active")
            .count() as i64;
        report.name = project.name.clone();
        report.docs_added = added;
        report.docs_updated = updated;
        report.docs_missing = missing;
        report.governance_seeded = seeded;
        report.pending_open = pending_open;
        report.steps.push(step("存量文档", "created", format!("入库 {added} 篇（更新 {updated}，缺失标记 {missing}）")));
        report.steps.push(step("治理数据", "created", format!("种子 {seeded} 条，待定 {pending_open} 条")));
        // 存量文档 → 需求（核心动作）：不满足于入库，直接进入需求流水线；命中完成信号的先本地归档
        let (converted, archived) = convert_unlinked_docs(&conn, &project.id)?;
        report.requirements_created = converted.len() as i64;
        report.locally_archived = archived;
        report.converted_requirement_ids = converted.clone();
        if converted.is_empty() {
            report.steps.push(step("需求转换", "skipped", "无可转换的存量文档"));
        } else if archived > 0 {
            report.steps.push(step("需求转换", "created", format!("{} 篇存量文档已转为需求（本地规则预归档 {archived} 条为已完成，其余依次理解）", converted.len())));
        } else {
            report.steps.push(step("需求转换", "created", format!("{} 篇存量文档已转为需求，正在依次理解", converted.len())));
        }
        (project, report)
    };
    Ok(crate::types::InitProjectOutcome { project, report, already_registered: false })
}

/// 重新初始化：幂等重跑目录准备 + 重扫文档 + 种子补缺 + 重算待定。
pub fn reinit_project(state: &AppState, project_id: &str) -> ApiResult<InitReport> {
    let path_str = {
        let conn = state.conn.lock().unwrap();
        db::get_project_path(&conn, project_id)?
            .ok_or_else(|| ApiError::bad_request("该项目未绑定目录，无法重新初始化"))?
    };
    let root = PathBuf::from(&path_str);
    let mut report = InitReport::default();
    prepare_directory(&root, &mut report)?;
    let conn = state.conn.lock().unwrap();
    let (added, updated, missing) = sync_project_docs(&conn, project_id, &root)?;
    let requirement_docs = db::list_project_docs(&conn, project_id)?
        .iter()
        .filter(|d| d.kind == "requirement" && !d.file_missing)
        .count();
    let (seeded, _) = seed_governance(&conn, project_id, &root, requirement_docs)?;
    let pending_open = db::list_governance_items(&conn, project_id, Some("pending"))?
        .iter()
        .filter(|i| i.status == "active")
        .count() as i64;
    report.docs_added = added;
    report.docs_updated = updated;
    report.docs_missing = missing;
    report.governance_seeded = seeded;
    report.pending_open = pending_open;
    report.steps.push(step("存量文档", "updated", format!("新增 {added} / 更新 {updated} / 缺失标记 {missing}")));
    report.steps.push(step("治理数据", "kept", format!("补缺 {seeded} 条，待定 {pending_open} 条")));
    let (converted, archived) = convert_unlinked_docs(&conn, project_id)?;
    report.requirements_created = converted.len() as i64;
    report.locally_archived = archived;
    report.converted_requirement_ids = converted.clone();
    if converted.is_empty() {
        report.steps.push(step("需求转换", "kept", "无新增可转换的存量文档"));
    } else if archived > 0 {
        report.steps.push(step("需求转换", "created", format!("{} 篇存量文档已转为需求（本地规则预归档 {archived} 条为已完成，其余依次理解）", converted.len())));
    } else {
        report.steps.push(step("需求转换", "created", format!("{} 篇存量文档已转为需求，正在依次理解", converted.len())));
    }
    Ok(report)
}

// ---------------------------------------------------------------- 路径处理

/// 展开 ~、转绝对路径、去尾斜杠。
pub fn normalize_path(input: &str) -> ApiResult<PathBuf> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("路径不能为空"));
    }
    let expanded = if trimmed == "~" {
        std::env::var("HOME").map_err(|_| ApiError::bad_request("无法定位用户主目录"))?
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        let home = std::env::var("HOME").map_err(|_| ApiError::bad_request("无法定位用户主目录"))?;
        format!("{home}/{rest}")
    } else {
        trimmed.to_string()
    };
    let path = PathBuf::from(expanded);
    if !path.is_absolute() {
        return Err(ApiError::bad_request("请使用绝对路径（如 /Users/xxx/Projects/demo）"));
    }
    Ok(path)
}

pub fn basename(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

// ---------------------------------------------------------------- 目录准备

fn git_init(dir: &Path) -> bool {
    match std::process::Command::new("git").arg("init").current_dir(dir).output() {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// 目录准备：缺则建 + git init。永不触碰目录内既有内容。
pub fn prepare_directory(path: &Path, report: &mut InitReport) -> ApiResult<()> {
    report.path = path.display().to_string();
    report.name = basename(path);
    if path.exists() && !path.is_dir() {
        return Err(ApiError::bad_request(format!("路径指向文件而非目录：{}", path.display())));
    }
    if path.exists() {
        report.steps.push(step("目录", "kept", "已存在，原样保留"));
    } else {
        std::fs::create_dir_all(path)
            .map_err(|e| ApiError::internal(format!("创建目录失败（{}）：{e}", path.display())))?;
        report.steps.push(step("目录", "created", "已创建（原不存在）"));
    }
    if path.join(".git").exists() {
        report.steps.push(step("git", "skipped", "已是 git 仓库"));
    } else if git_init(path) {
        report.steps.push(step("git", "created", "已执行 git init"));
    } else {
        report.steps.push(step("git", "warned", "git init 失败（未安装 git 或无写权限），不影响项目创建"));
    }
    Ok(())
}

// ---------------------------------------------------------------- 存量文档扫描

#[derive(Debug, Clone)]
pub struct ScannedDoc {
    pub rel_path: String,
    pub kind: &'static str,
    pub title: String,
    pub excerpt: Option<String>,
    pub content: Option<String>,
    pub content_chars: Option<i64>,
    pub file_mtime: Option<String>,
}

fn is_ignored_dir(name: &str) -> bool {
    IGNORED_DIRS.contains(&name)
}

fn mtime_iso(path: &Path) -> Option<String> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| {
            chrono::DateTime::<chrono::Utc>::from(std::time::UNIX_EPOCH + d)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })
}

fn first_n_chars(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

fn classify(rel_path: &str, file_name: &str) -> &'static str {
    let lower_file = file_name.to_lowercase();
    let lower_rel = rel_path.to_lowercase();
    if lower_file.starts_with("readme") || lower_file == "agents.md" || lower_file == "changelog.md" {
        return "readme";
    }
    if REQUIREMENT_KEYWORDS.iter().any(|k| lower_rel.contains(k)) {
        return "requirement";
    }
    "doc"
}

fn read_text_file(path: &Path) -> (Option<String>, Option<i64>) {
    let Ok(bytes) = std::fs::read(path) else { return (None, None) };
    let truncated: Vec<u8> = bytes.into_iter().take(CONTENT_MAX_BYTES).collect();
    let text = String::from_utf8_lossy(&truncated);
    let chars = text.chars().count();
    (Some(text.to_string()), Some(chars as i64))
}

/// 遍历项目全部目录收集文档：所有层级的文本类文档 + 二进制文档（pdf/docx）。
/// 跳过 IGNORED_DIRS 与隐藏目录；深度 ≤ SCAN_MAX_DEPTH；总量 ≤ SCAN_MAX_FILES；只读。
/// 找「有可能是需求的文档」靠广撒网：是否成为需求由转换阶段决定，不靠目录名/文件名猜。
pub fn scan_inventory(root: &Path) -> Vec<ScannedDoc> {
    let mut docs: Vec<ScannedDoc> = Vec::new();
    walk_dir(root, root, &mut docs);
    docs
}

fn walk_dir(dir: &Path, root: &Path, docs: &mut Vec<ScannedDoc>) {
    walk_depth(dir, root, docs, 0);
}

fn walk_depth(dir: &Path, root: &Path, docs: &mut Vec<ScannedDoc>, depth: usize) {
    if docs.len() >= SCAN_MAX_FILES || depth > SCAN_MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut sub_dirs: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        if docs.len() >= SCAN_MAX_FILES {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !is_ignored_dir(&name) && !name.starts_with('.') {
                sub_dirs.push(path);
            }
            continue;
        }
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase().to_string());
        let ext = ext.as_deref();
        let is_text = ext.map(|e| TEXT_EXTS.contains(&e)).unwrap_or(false);
        let is_binary_doc = ext.map(|e| BINARY_DOC_EXTS.contains(&e)).unwrap_or(false);
        if !is_text && !is_binary_doc {
            continue;
        }
        push_doc(docs, root, &path, &name, !is_text);
    }
    for sub in sub_dirs {
        walk_depth(&sub, root, docs, depth + 1);
    }
}

fn push_doc(docs: &mut Vec<ScannedDoc>, root: &Path, path: &Path, file_name: &str, binary: bool) {
    let Ok(rel) = path.strip_prefix(root) else { return };
    let rel_path = rel.to_string_lossy().replace('\\', "/");
    let kind = classify(&rel_path, file_name);
    let title = file_name.to_string();
    if binary {
        docs.push(ScannedDoc {
            rel_path,
            kind,
            title,
            excerpt: None,
            content: None,
            content_chars: None,
            file_mtime: mtime_iso(path),
        });
        return;
    }
    let (content, chars) = read_text_file(path);
    let excerpt = content.as_ref().map(|c| first_n_chars(c, EXCERPT_CHARS));
    docs.push(ScannedDoc {
        rel_path,
        kind,
        title,
        excerpt,
        content,
        content_chars: chars,
        file_mtime: mtime_iso(path),
    });
}

/// 库 ←→ 目录 同步：新增 INSERT / mtime 变化 UPDATE / 消失标 file_missing。返回 (新增, 更新, 标缺失)。
pub fn sync_project_docs(conn: &rusqlite::Connection, project_id: &str, root: &Path) -> ApiResult<(i64, i64, i64)> {
    let scanned = scan_inventory(root);
    let mut added = 0i64;
    let mut updated = 0i64;
    for doc in &scanned {
        if db::upsert_project_doc(conn, project_id, doc)? {
            added += 1;
        } else {
            updated += 1;
        }
    }
    let present: Vec<String> = scanned.iter().map(|d| d.rel_path.clone()).collect();
    let missing = db::mark_missing_project_docs(conn, project_id, &present)?;
    Ok((added, updated, missing))
}

// ---------------------------------------------------------------- 治理种子

/// 规则块九字段种子（内容取自 agent-up 协议，authority_level=4 流程规则）。
const SEED_RULES: &[(&str, &str, &str, &str, &str, &str, &str, &str, &str)] = &[
    (
        "R-AG-001", "记录保护", "MUST NOT",
        "对目标项目内任何既有文件与事实（文档、代码布局、命名、既有规则）",
        "一律视为项目事实：只登记、只补缺；差异报告用户",
        "静默改写、覆盖既有事实",
        "无法判定某内容是否为既有事实",
        "登记清单与差异报告",
        "执行体",
    ),
    (
        "R-AG-002", "条件产物默认不创建", "MUST NOT",
        "未命中创建触发条件的治理产物（issues/specs/CONTEXT.md 等）",
        "保持不创建，仅在 manifest 触发矩阵命中时生成",
        "默认批量创建条件产物",
        "触发条件无法判定",
        "产物清单与触发矩阵核对记录",
        "执行体",
    ),
    (
        "R-AG-003", "三道门禁", "MUST",
        "一切语义产出与交付",
        "语义变化未经用户确认不写文件；无验证证据不算完成；无独立 Review 通过不提交",
        "跳过任一道门禁宣告完成",
        "任一门禁证据缺失",
        "确认记录、验证证据、Review 结论",
        "执行体与用户",
    ),
    (
        "R-RP-001", "读取阶梯不跳过", "MUST",
        "任何工作会话开始时",
        "按 L0 治理规则 → L1 状态 → L2 任务合同/规格 → L3 源码事实 的顺序加载必读集合",
        "跳过必读集合直接动手；用聊天记忆替代阶梯",
        "必读来源缺失或不可读",
        "上下文注入记录",
        "执行体",
    ),
    (
        "R-RP-002", "权威冲突即停", "MUST",
        "任何两个权威来源对同一事实给出不一致结论时",
        "停止修改 → 报告冲突双方与层级 → 给出选项 → 等用户裁决",
        "自行选择一种解释继续写；静默合并；低层级覆盖高层级",
        "用户不在场",
        "冲突报告与用户裁决记录",
        "执行体",
    ),
    (
        "R-RP-003", "最小读取档位", "SHOULD",
        "加载治理资源时",
        "按 minimum 档位加载：L0+L1+任务相关 L2；L3 按动作所需逐文件；L4 按需",
        "一次全量加载所有治理资源",
        "档位边界无法映射到读取阶梯",
        "读取范围说明",
        "执行体",
    ),
    (
        "R-CP-001", "复杂度判级", "MUST",
        "每项工作动工前",
        "独立记录 Complexity C0|C1|C2|C3、命中理由与评估日期；任一命中条件即至少该级，按最高命中定级",
        "自造等级（C4/半级）；用工作量或耗时替代触发条件判级",
        "判级依据不足",
        "判级记录（写入需求）",
        "理解阶段（Triage）",
    ),
    (
        "R-CP-002", "C3 拆票门禁", "MUST NOT",
        "任务判为 C3（无法在一个上下文完成并验证）",
        "禁止直接开工：拆为可独立演示/验证的 C0-C2 垂直票，或升级协调",
        "C3 直接开工；拆出仍命中 C3 的票",
        "无法拆出可独立演示/验证的垂直票",
        "拆票记录",
        "规划阶段",
    ),
    (
        "R-CP-006", "车道声明", "MUST",
        "为需求选定执行车道时",
        "按准入判据声明：C0+全机械可验→微任务道；C0+含语义判断→用户即 Review；其余→全三阶段。执行体无权自选车道",
        "放宽机械可验判据；为进快道压缩验收条件；C1+ 走快道",
        "判据命中与否无法判定（按全三阶段处理）",
        "判级与判据命中理由",
        "理解阶段（Triage）",
    ),
    (
        "R-GF-010", "措辞禁令", "MUST NOT",
        "撰写关键规则（MUST/MUST NOT/门禁/验收语句）",
        "使用可判定措辞；柔性指引降级为 SHOULD/MAY 并写清判断依据",
        "使用“尽量、适当、必要时、通常、原则上、酌情”等不可判定措辞",
        "关键语句含模糊词",
        "规则文本",
        "所有产出规则的执行体",
    ),
];

/// 三阶段角色合同种子（能力基元为 agent-up 受控枚举）。json! 含堆分配，须函数返回而非 const。
fn seed_roles() -> Vec<(&'static str, &'static str, &'static str, serde_json::Value)> {
    vec![
        (
            "role-implementation",
            "实施角色合同",
            "implementation",
            serde_json::json!({
                "stage": "implementation",
                "capabilities": ["inspect", "search", "read", "edit", "write", "execute"],
                "permissions": [],
                "scope": "按任务合同在项目目录内实施变更；不越 Scope、不擅自提交版本库",
                "forbidden": ["扩大实施范围", "未经确认写入语义变化", "git commit/push"],
                "acceptance": ["变更清单逐条可核对", "相关验证命令已运行并通过", "交付说明覆盖全部成功标准"],
                "output_format": "JSON：{summary, changes[], files[{path,description}], test_plan}",
                "exit_criteria": "完整交付并自检通过，停在待验证状态"
            }),
        ),
        (
            "role-review",
            "审查角色合同",
            "review",
            serde_json::json!({
                "stage": "review",
                "capabilities": ["inspect", "search", "read", "readonly-execute"],
                "permissions": [],
                "scope": "只读审查实施结果：核验证据、对照成功标准、评估变更影响面",
                "forbidden": ["修改任何文件", "信任实施方陈述代替证据", "git add/commit"],
                "acceptance": ["每条结论指向具体证据", "不通过项给出缺陷与复现路径"],
                "output_format": "JSON：{passed, results[{criteria,passed,note}], conclusion} 或 {findings[], conclusion}",
                "exit_criteria": "给出明确放行/不放行结论"
            }),
        ),
        (
            "role-commit",
            "提交角色合同",
            "commit",
            serde_json::json!({
                "stage": "commit",
                "capabilities": ["inspect", "vcs-read", "vcs-write"],
                "permissions": [],
                "scope": "仅在用户明确授权且白名单满足后创建提交",
                "forbidden": ["未经用户要求执行 git add/commit/push", "提交白名单外文件"],
                "acceptance": ["Review 证据齐备", "提交范围与白名单一致"],
                "output_format": "提交记录（hash + 说明）",
                "exit_criteria": "提交完成且不 push、不部署"
            }),
        ),
    ]
}

/// 待定项种子：验证命令 + 领域词条（盘点无法自证的事实，留待治理补全，不编造）。
fn seed_pendings(conn: &rusqlite::Connection, project_id: &str, root: &Path, requirement_docs: usize) -> ApiResult<(i64, i64)> {
    let mut seeded = 0i64;
    let manifest_hint = ["package.json", "Cargo.toml", "Makefile", "pyproject.toml", "go.mod", "pom.xml"]
        .iter()
        .find(|f| root.join(f).is_file())
        .map(|f| format!("检测到 {f}，可从其配置推断验证命令"));
    let body = serde_json::json!({
        "question": "本项目的验证命令是什么？",
        "reason": "验证命令属于项目事实：能从仓库查证则查证，查不到必须问人，不编造",
        "hint": manifest_hint.unwrap_or_else(|| "如 npm test / cargo test / make check".into()),
    });
    if db::insert_governance_item_if_absent(conn, project_id, "pending", "pending-verify-command", "验证命令待定", &body)? {
        seeded += 1;
    }
    if requirement_docs > 0 {
        let body = serde_json::json!({
            "question": "本项目有哪些必须统一的领域术语？",
            "reason": "领域词汇是项目事实：没有依据的词条一律不建，需从权威文档提炼或由人确认",
            "hint": format!("已入库 {requirement_docs} 篇需求类文档，可从中提炼"),
        });
        if db::insert_governance_item_if_absent(conn, project_id, "pending", "pending-domain-terms", "领域术语待定", &body)? {
            seeded += 1;
        }
    }
    let open: i64 = conn.query_row(
        "SELECT COUNT(*) FROM governance_items WHERE project_id = ?1 AND kind = 'pending' AND status = 'active'",
        params![project_id],
        |row| row.get(0),
    )?;
    Ok((seeded, open))
}

/// 治理种子：规则 10 + 角色 3 + 待定推断；幂等（已存在跳过）。返回（实际写入条数, 报告步骤）。
pub fn seed_governance(conn: &rusqlite::Connection, project_id: &str, root: &Path, requirement_docs: usize) -> ApiResult<(i64, Vec<InitStep>)> {
    let mut seeded = 0i64;
    for (key, title, level, when, action, forbidden, stop_if, evidence, owner) in SEED_RULES {
        let body = serde_json::json!({
            "level": level, "when": when, "action": action,
            "forbidden": forbidden, "stop_if": stop_if,
            "evidence": evidence, "owner": owner,
            "authority_level": 4, "authority_note": "AgentUp Harness 内置流程规则（对齐 agent-up 协议）",
        });
        if db::insert_governance_item_if_absent(conn, project_id, "rule", key, title, &body)? {
            seeded += 1;
        }
    }
    for (key, title, _stage, body) in seed_roles() {
        if db::insert_governance_item_if_absent(conn, project_id, "role", key, title, &body)? {
            seeded += 1;
        }
    }
    let (pending_seeded, _open) = seed_pendings(conn, project_id, root, requirement_docs)?;
    seeded += pending_seeded;
    Ok((seeded, Vec::new()))
}

// ---------------------------------------------------------------- 存量文档自动转需求

/// 完成信号词：命中即按「已完成/纯资料」预归档（本地规则，不依赖 LLM；理解阶段可复核推翻）。
pub const DONE_SIGNALS: &[&str] = &[
    "已完成", "已实现", "已上线", "已交付", "已关闭", "implemented", "already done", "released",
];

/// 扫描文档字段（相对路径 + 标题 + 正文前 2000 字）判定是否"已完成/纯资料"。
pub fn looks_done(rel_path: &str, title: &str, content: &str) -> bool {
    let head: String = content.chars().take(2000).collect::<String>().to_lowercase();
    let hay = format!("{rel_path} {title} {head}");
    DONE_SIGNALS.iter().any(|k| hay.contains(k))
}

/// 把「未转换过」的存量文档自动落成需求（初始化后的核心动作）。
/// 广撒网原则：遍历找到的所有有内容的文本文档都转为需求——是否真是需求由理解阶段消化内容后判定，
/// 不靠文件名关键词预判。kind（requirement/readme/doc）仅作展示分组。
/// 本地规则预归类：字段命中完成信号（DONE_SIGNALS）的需求直接归档为已完成，不占理解队列；
/// 理解阶段（LLM）随后可复核推翻。
/// 幂等：doc.requirement_id 已存在且需求仍在 → 跳过；需求被删除 → 重新转换。
/// 返回（新建需求 id 列表，本地预归档条数）。
pub fn convert_unlinked_docs(conn: &rusqlite::Connection, project_id: &str) -> ApiResult<(Vec<String>, i64)> {
    let docs = db::list_project_docs(conn, project_id)?;
    let mut ids: Vec<String> = Vec::new();
    let mut archived = 0i64;
    for doc in docs.iter().filter(|d| !d.file_missing) {
        let already_linked = match &doc.requirement_id {
            Some(rid) => db::get_requirement(conn, rid)?.is_some(),
            None => false,
        };
        if already_linked {
            continue;
        }
        let full = db::get_project_doc_content(conn, &doc.id)?.unwrap_or_default();
        if full.trim().is_empty() {
            continue;
        }
        let mut text: String = full.chars().take(8000).collect();
        if full.chars().count() > 8000 {
            text.push_str("\n\n【已截断：全文见项目文档】");
        }
        let content = format!("来自存量文档「{}」：\n\n{}", doc.rel_path, text);
        let requirement = db::create_requirement_with_source(conn, project_id, &content, Some(&doc.rel_path), None, crate::types::RequirementMode::Standard, None)?;
        db::link_doc_requirement(conn, &doc.id, &requirement.id)?;
        db::create_log(
            conn,
            &requirement.id,
            None,
            "init",
            "info",
            &format!("由存量文档自动转换：{}", doc.rel_path),
            None,
        )?;
        // 本地规则归类：命中完成信号 → 直接归档；否则进入「初始化中」（隐藏于需求列表，理解完成后再展示）
        if looks_done(&doc.rel_path, &doc.title, &full) {
            db::update_requirement(
                conn,
                &requirement.id,
                &crate::db::RequirementUpdates {
                    status: Some(crate::types::RequirementStatus::Completed),
                    current_step: None,
                    ..Default::default()
                },
            )?;
            db::create_log(
                conn,
                &requirement.id,
                None,
                "classify",
                "info",
                "本地规则归类：文档字段命中完成信号（已完成/已实现/已上线等），归档为已完成；可重新理解以复核",
                None,
            )?;
            archived += 1;
        } else {
            db::update_requirement(
                conn,
                &requirement.id,
                &crate::db::RequirementUpdates {
                    status: Some(crate::types::RequirementStatus::Initializing),
                    ..Default::default()
                },
            )?;
        }
        ids.push(requirement.id);
    }
    Ok((ids, archived))
}

// ---------------------------------------------------------------- 读取阶梯上下文注入

fn truncate_chars(text: &str, n: usize) -> String {
    let mut out: String = text.chars().take(n).collect();
    if text.chars().count() > n {
        out.push('…');
    }
    out
}

/// 按读取阶梯组装项目上下文块：治理规则（MUST 优先）→ 阶段角色合同 → 需求文档摘要 → 目录条目。
/// 任何一段缺数据都静默降级（未初始化目录的项目注入空块）。
pub fn build_context_block(conn: &rusqlite::Connection, project_id: &str, stage: &str) -> String {
    let mut sections: Vec<String> = Vec::new();

    // L0：治理规则（MUST/MUST NOT 优先，限 8 条）
    let mut rules = db::list_governance_items(conn, project_id, Some("rule")).unwrap_or_default();
    rules.sort_by_key(|r| match r.body.get("level").and_then(|l| l.as_str()) {
        Some("MUST NOT") => 0,
        Some("MUST") => 1,
        Some("SHOULD") => 2,
        _ => 3,
    });
    if !rules.is_empty() {
        let lines: Vec<String> = rules
            .iter()
            .take(8)
            .map(|r| {
                let level = r.body.get("level").and_then(|l| l.as_str()).unwrap_or("MUST");
                let action = r.body.get("action").and_then(|a| a.as_str()).unwrap_or("");
                let forbidden = r.body.get("forbidden").and_then(|a| a.as_str()).unwrap_or("");
                format!("- [{level}] {}（{}）：{action}；禁止：{forbidden}", r.title, r.key)
            })
            .collect();
        sections.push(format!("## 必守治理规则（权威层级 4，违反即失败）\n{}", lines.join("\n")));
    }

    // 当前阶段角色合同
    let role_key = match stage {
        "understand" | "plan" => None, // 分析阶段无对应 agent-up 角色，规则已覆盖
        "implement" => Some("role-implementation"),
        "verify" | "review" => Some("role-review"),
        _ => None,
    };
    if let Some(key) = role_key {
        if let Some(role) = db::list_governance_items(conn, project_id, Some("role"))
            .unwrap_or_default()
            .into_iter()
            .find(|r| r.key == key)
        {
            let caps = role.body.get("capabilities").and_then(|c| c.as_array()).map(|a| {
                a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")
            }).unwrap_or_default();
            let forbidden = role.body.get("forbidden").and_then(|c| c.as_array()).map(|a| {
                a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("；")
            }).unwrap_or_default();
            sections.push(format!(
                "## 本阶段角色合同（{}）\n允许能力：{caps}\n禁止行为：{forbidden}",
                role.title
            ));
        }
    }

    // L2：需求类文档摘要（≤5 篇 × 500 字）
    let docs = db::list_project_docs(conn, project_id).unwrap_or_default();
    let req_docs: Vec<&ProjectDoc> = docs.iter().filter(|d| d.kind == "requirement" && !d.file_missing).take(5).collect();
    if !req_docs.is_empty() {
        let lines: Vec<String> = req_docs
            .iter()
            .map(|d| {
                let excerpt = d.excerpt.as_deref().map(|e| truncate_chars(e, EXCERPT_CHARS)).unwrap_or_default();
                format!("### {}（{}）\n{}", d.rel_path, d.kind, excerpt)
            })
            .collect();
        sections.push(format!("## 项目存量需求文档摘要（权威层级 2：项目事实）\n{}", lines.join("\n\n")));
    }

    // L3：目录条目（顶层 ≤50 条，只读事实）
    if let Some(Some(path)) = db::get_project_path(conn, project_id).ok().map(|p| p) {
        let root = PathBuf::from(&path);
        if let Ok(entries) = std::fs::read_dir(&root) {
            let mut names: Vec<String> = entries
                .flatten()
                .take(50)
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.path().is_dir() { format!("{name}/") } else { name }
                })
                .collect();
            names.sort();
            if !names.is_empty() {
                sections.push(format!("## 项目目录条目（顶层，只读事实）\n{}", names.join("  ")));
            }
        }
    }

    if sections.is_empty() {
        return String::new();
    }
    format!("【项目治理上下文】\n{}", sections.join("\n\n"))
}

// ---------------------------------------------------------------- 导出（库 → 文件，单向投影）

#[derive(Debug, Clone, Serialize, Default)]
pub struct ExportOutcome {
    pub written: Vec<String>,
    pub skipped: Vec<String>,
    pub target_dir: String,
}

fn generated_header() -> String {
    // 不带时间戳：保证同一数据重复导出内容逐字节一致（cmp 幂等跳过的前提）。
    "/* 生成于 AgentUp Harness —— 单向投影，事实源在应用数据库，勿手改 */\n".to_string()
}

fn render_rules_markdown(rules: &[GovernanceItem]) -> String {
    let mut out = String::from("# 开发流程规则\n\n");
    out.push_str(&generated_header());
    out.push_str("> 本文件由 AgentUp Harness 从数据库治理条目投影生成；修改请在应用内进行。\n\n");
    for r in rules {
        let g = |k: &str| r.body.get(k).and_then(|v| v.as_str()).unwrap_or("无");
        let level = r.body.get("level").and_then(|v| v.as_str()).unwrap_or("MUST");
        let authority = r.body.get("authority_level").and_then(|v| v.as_i64()).unwrap_or(4);
        out.push_str(&format!(
            "## {} {}（{}）\n\n- **When**: {}\n- **Action**: {}\n- **Forbidden**: {}\n- **Stop if**: {}\n- **Evidence**: {}\n- **Owner**: {}\n- **Authority**: 权威层级第 {authority} 级（流程规则）\n\n",
            r.key, r.title, level, g("when"), g("action"), g("forbidden"), g("stop_if"), g("evidence"), g("owner")
        ));
    }
    out
}

fn render_role_markdown(item: &GovernanceItem) -> String {
    let g = |k: &str| -> String {
        match item.body.get(k) {
            Some(serde_json::Value::Array(a)) => a
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("- {s}"))
                .collect::<Vec<_>>()
                .join("\n"),
            Some(serde_json::Value::String(s)) => s.clone(),
            _ => "无".into(),
        }
    };
    let mut out = String::from("---\n");
    out.push_str(&format!(
        "id: {}\nkind: process\nauthority: 权威层级第 4 级（流程规则）\nlifecycle: Live\nread_when: 执行 {} 阶段前\ntrigger: 阶段能力边界变化时\nowner: 执行体\nupdate_policy: 应用内修改后重新导出\ndepends_on: development-process\n",
        item.key, g("stage")
    ));
    out.push_str("---\n\n");
    out.push_str(&generated_header());
    out.push_str(&format!("# {}\n\n## 输入\n{}\n\n## 所需能力\n{}\n\n## Scope 边界\n{}\n\n## 禁止行为\n{}\n\n## 验收条件\n{}\n\n## 输出格式\n{}\n\n## 阶段出口\n{}\n",
        item.title, "任务合同与项目上下文", g("capabilities"), g("scope"), g("forbidden"), g("acceptance"), g("output_format"), g("exit_criteria")));
    out
}

fn render_terms_markdown(terms: &[GovernanceItem]) -> String {
    let mut out = String::from("# CONTEXT 领域上下文\n\n");
    out.push_str(&generated_header());
    if terms.is_empty() {
        out.push_str("【待定：尚无确认的领域词条——词条是项目事实，无依据不编造】\n");
        return out;
    }
    for t in terms {
        let definition = t.body.get("definition").and_then(|v| v.as_str()).unwrap_or("【待定】");
        let avoid = t.body.get("avoid").and_then(|v| v.as_str()).unwrap_or_default();
        let avoid_note = if avoid.is_empty() { String::new() } else { format!(" _Avoid:_ {avoid}") };
        out.push_str(&format!("- **{}**：{}{avoid_note}\n", t.key, definition));
    }
    out
}

fn write_projection(target: &Path, content: &str, outcome: &mut ExportOutcome) -> ApiResult<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if target.exists() {
        let existing = std::fs::read_to_string(target).unwrap_or_default();
        if existing == content {
            outcome.skipped.push(target.display().to_string());
            return Ok(());
        }
        let backup = target.with_extension(format!(
            "bak-{}",
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        ));
        std::fs::copy(target, &backup)?;
    }
    std::fs::write(target, content)?;
    outcome.written.push(target.display().to_string());
    Ok(())
}

/// 导出 agent-up 兼容文件集到项目目录（AGENTS.md 路由 + 规则 + 角色合同 + CONTEXT + artifacts 投影）。
pub fn export_agentup_files(state: &AppState, project_id: &str) -> ApiResult<ExportOutcome> {
    let path = db::get_project_path(&state.conn.lock().unwrap(), project_id)?
        .ok_or_else(|| ApiError::bad_request("该项目未绑定目录，无法导出文件集"))?;
    let root = PathBuf::from(path);
    let conn = state.conn.lock().unwrap();
    let rules = db::list_governance_items(&conn, project_id, Some("rule"))?;
    let roles = db::list_governance_items(&conn, project_id, Some("role"))?;
    let terms = db::list_governance_items(&conn, project_id, Some("term"))?;

    let mut outcome = ExportOutcome { target_dir: root.display().to_string(), ..Default::default() };

    let agents_md = format!(
        "{}# {} — Agent 路由入口\n\n本文件只做路由与边界声明，不复制流程细节。\n唯一流程权威：`docs/development-process.md`（由 AgentUp Harness 数据库投影）。\n\n## 先读什么\n\n1. 本文件（边界与路由）\n2. `docs/development-process.md`（流程规则与门禁）\n3. `docs/CONTEXT.md`（领域词条，如有）\n4. 与改动相关的源码与测试\n\n## 最小仓库边界\n\n- 密钥与凭据不进 Git\n- 不改动他人未提交的工作\n- 未获用户明确要求，不执行 git push、部署或发布\n",
        generated_header(),
        crate::project_init::basename(&root)
    );
    write_projection(&root.join("AGENTS.md"), &agents_md, &mut outcome)?;
    write_projection(&root.join("docs/development-process.md"), &render_rules_markdown(&rules), &mut outcome)?;
    write_projection(&root.join("docs/CONTEXT.md"), &render_terms_markdown(&terms), &mut outcome)?;
    for role in &roles {
        let file = match role.key.as_str() {
            "role-implementation" => "docs/agent/roles/implementation.md",
            "role-review" => "docs/agent/roles/review.md",
            "role-commit" => "docs/agent/roles/commit.md",
            other => other,
        };
        write_projection(&root.join(file), &render_role_markdown(role), &mut outcome)?;
    }
    // 产物台账投影：artifacts 表 → docs/agent/artifacts.yaml
    let artifacts: Vec<(String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT a.title, a.type, a.created_at FROM artifacts a
             JOIN requirements r ON r.id = a.requirement_id WHERE r.project_id = ?1 ORDER BY a.created_at ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok((row.get::<_, Option<String>>(0)?.unwrap_or_default(), row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let mut yaml = String::from("# 产物台账（AgentUp Harness 数据库投影）\n\nartifacts:\n");
    yaml.push_str(&generated_header());
    for (title, kind, created) in &artifacts {
        yaml.push_str(&format!(
            "  - id: \"art-{}\"\n    title: \"{}\"\n    kind: {}\n    lifecycle: Record\n    authority: 权威层级第 6 级（派生）\n    created_at: \"{}\"\n",
            chrono::DateTime::parse_from_rfc3339(created).map(|t| t.format("%Y%m%d%H%M%S").to_string()).unwrap_or_else(|_| created.replace(['-', ':', '.'], "")),
            title.replace('"', "'"),
            kind,
            created
        ));
    }
    if artifacts.is_empty() {
        yaml.push_str("  []\n");
    }
    write_projection(&root.join("docs/agent/artifacts.yaml"), &yaml, &mut outcome)?;
    Ok(outcome)
}

/// 导出 JSON 快照（治理 + 文档元数据）到指定路径。
pub fn export_json_snapshot(state: &AppState, project_id: &str, target: &str) -> ApiResult<ExportOutcome> {
    let conn = state.conn.lock().unwrap();
    let project = db::get_project(&conn, project_id)?.ok_or_else(|| ApiError::not_found("项目不存在"))?;
    let snapshot = serde_json::json!({
        "exported_at": now_iso(),
        "generator": "AgentUp Harness",
        "project": { "id": project.id, "name": project.name, "path": project.path },
        "governance": db::list_governance_items(&conn, project_id, None)?,
        "docs": db::list_project_docs(&conn, project_id)?,
    });
    drop(conn);
    let mut outcome = ExportOutcome { target_dir: target.to_string(), ..Default::default() };
    let target_path = PathBuf::from(target);
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&target_path, serde_json::to_string_pretty(&snapshot)?)?;
    outcome.written.push(target_path.display().to_string());
    Ok(outcome)
}
