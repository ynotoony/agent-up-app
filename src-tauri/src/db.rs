use crate::error::{ApiError, ApiResult};
use tauri::Manager;
use crate::types::*;
use rusqlite::{params, Connection, Row};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct AppState {
    pub conn: Mutex<Connection>,
    pub attachments_dir: PathBuf,
    pub roles_dir: PathBuf,
}

pub fn init_state(handle: &tauri::AppHandle) -> ApiResult<AppState> {
    let base = handle
        .path()
        .app_data_dir()
        .map_err(|e| ApiError::internal(format!("无法定位数据目录: {e}")))?;
    Ok(open_state(&base)?)
}

pub fn open_state(base: &Path) -> ApiResult<AppState> {
    std::fs::create_dir_all(base)?;
    let conn = Connection::open(base.join("agentup.db"))?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(include_str!("../schema.sql"))?;
    run_migrations(&conn)?;
    Ok(AppState {
        conn: Mutex::new(conn),
        attachments_dir: base.join("attachments"),
        roles_dir: base.join("roles"),
    })
}

/// 幂等列迁移：无 migration 框架，启动时按 pragma_table_info 补列（老库升级路径）。
/// 新建库由 schema.sql 直接带全量列，此处全部跳过。
fn run_migrations(conn: &Connection) -> ApiResult<()> {
    let add_col = |table: &str, decl: &str| -> ApiResult<()> {
        let col = decl.split_whitespace().next().unwrap_or_default();
        if col.is_empty() {
            return Err(ApiError::internal(format!("迁移列声明非法: {decl}")));
        }
        let exists: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1"),
            params![col],
            |row| row.get(0),
        )?;
        if exists == 0 {
            conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {decl}"), [])?;
        }
        Ok(())
    };
    add_col("projects", "path text")?;
    for decl in [
        "source_doc_path text",
        "complexity varchar(4)",
        "complexity_reason text",
        "profile jsonb",
        "risk_surfaces jsonb",
        "lane varchar(20)",
        "estimated_minutes integer",
        "actual_minutes integer",
        "total_tokens integer",
    ] {
        add_col("requirements", decl)?;
    }
    add_col("tasks", "tokens integer")?;
    add_col("project_docs", "requirement_id text")?;
    add_col("artifacts", "stage varchar(30)")?;

    // 存量回填 artifacts.stage：标题与阶段的对应是既定契约（与 orchestrator 写入、前端回看共用），一次性按标题推导。
    conn.execute_batch(
        "UPDATE artifacts SET stage = 'plan' WHERE stage IS NULL AND title = '实施方案';
         UPDATE artifacts SET stage = 'implement' WHERE stage IS NULL AND title = '交付产物';
         UPDATE artifacts SET stage = 'verify' WHERE stage IS NULL AND title = '验证报告';
         UPDATE artifacts SET stage = 'review' WHERE stage IS NULL AND title = '审查报告';",
    )?;

    // 旧数据搬迁：description 里以绝对路径形态存储的目录 → path 正式归位。
    conn.execute_batch(
        "UPDATE projects SET path = description WHERE path IS NULL AND description LIKE '/%';
         UPDATE projects SET description = NULL WHERE path IS NOT NULL AND description = path;",
    )?;

    // 存量文档转换的需求统一迁移到 initializing（理解归类完成前隐藏于需求列表）。
    // 正常用户创建的 pending 需求不带「来自存量文档」前缀，不受影响；幂等可重跑。
    conn.execute_batch(
        "UPDATE requirements SET status = 'initializing' WHERE status = 'pending' AND content LIKE '来自存量文档%';",
    )?;

    // 存量回填 source_doc_path：魔串信封只此一次解析（之后前端/CLI 均读字段）。
    conn.execute(
        "UPDATE requirements SET source_doc_path = substr(content, 9, instr(substr(content, 9), '」') - 1)
         WHERE source_doc_path IS NULL AND content LIKE '来自存量文档「%' AND instr(substr(content, 9), '」') > 0",
        [],
    )?;

    // 存量回填 execution_logs.details：从「执行者：X，角色版本 T」文案提取 executor/runtime_kind（老日志 details 为 NULL）。
    // substr 偏移 5 =「执行者：」4 字符 + 1；rtrim 字符集只含分隔符/数字/时间符号，不含字母，名称安全。幂等可重跑。
    conn.execute(
        "UPDATE execution_logs SET details = json_object(
            'executor', rtrim(substr(message, 5), '，角色版本 0123456789-T:Z'),
            'runtime_kind', CASE
                WHEN substr(message, 5) LIKE 'ZCode%' THEN 'zcode'
                WHEN substr(message, 5) LIKE 'Codex%' THEN 'codex'
                WHEN substr(message, 5) LIKE 'OpenCode%' THEN 'opencode'
                ELSE NULL END)
         WHERE details IS NULL AND message LIKE '执行者：%'",
        [],
    )?;

    // path 唯一索引：仅当当前无重复时创建（存量重复行交由应用层 projects_init 去重，不自动删数据）。
    let dup: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (SELECT path FROM projects WHERE path IS NOT NULL GROUP BY path HAVING COUNT(*) > 1)",
        [],
        |row| row.get(0),
    )?;
    if dup == 0 {
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS projects_path_idx ON projects(path) WHERE path IS NOT NULL;",
        )?;
    }
    Ok(())
}

/// 以附件目录为根执行一次落盘操作。
pub fn with_attachments_dir<T>(state: &AppState, f: impl FnOnce(&Path) -> ApiResult<T>) -> ApiResult<T> {
    f(&state.attachments_dir)
}

fn json_col<T: serde::de::DeserializeOwned>(row: &Row, col: &str) -> rusqlite::Result<Option<T>> {
    let value: Option<String> = row.get(col)?;
    Ok(value.and_then(|s| serde_json::from_str(&s).ok()))
}

// ---------------------------------------------------------------- Project

fn map_project(row: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        path: row.get("path")?,
        status: row.get("status")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        req_count: None,
    })
}

pub fn list_projects(conn: &Connection) -> ApiResult<Vec<Project>> {
    // req_count 与统计口径一致：initializing（理解归类中）不计入
    let mut stmt = conn.prepare(
        "SELECT p.*, (SELECT COUNT(*) FROM requirements r WHERE r.project_id = p.id AND r.status != 'initializing') AS req_count
         FROM projects p ORDER BY p.created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            let mut p = map_project(row)?;
            p.req_count = Some(row.get::<_, i64>("req_count")?);
            Ok(p)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn create_project(conn: &Connection, name: &str, description: Option<&str>) -> ApiResult<Project> {
    create_project_full(conn, name, description, None)
}

pub fn create_project_full(conn: &Connection, name: &str, description: Option<&str>, path: Option<&str>) -> ApiResult<Project> {
    let id = new_id();
    let ts = now_iso();
    conn.execute(
        "INSERT INTO projects (id, name, description, path, status, created_at, updated_at) VALUES (?1,?2,?3,?4,'active',?5,?5)",
        params![id, name, description, path, ts],
    )?;
    Ok(get_project(conn, &id)?.expect("project just inserted"))
}

pub fn find_project_by_path(conn: &Connection, path: &str) -> ApiResult<Option<Project>> {
    let mut stmt = conn.prepare("SELECT * FROM projects WHERE path = ?1 ORDER BY created_at DESC LIMIT 1")?;
    let mut rows = stmt.query_map(params![path], map_project)?;
    match rows.next() {
        Some(Ok(p)) => Ok(Some(p)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

pub fn get_project(conn: &Connection, id: &str) -> ApiResult<Option<Project>> {
    let mut stmt = conn.prepare("SELECT * FROM projects WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![id], map_project)?;
    match rows.next() {
        Some(Ok(project)) => Ok(Some(project)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// 项目工作目录（projects.path）；未绑定目录的项目返回 None。
pub fn get_project_path(conn: &Connection, project_id: &str) -> ApiResult<Option<String>> {
    Ok(get_project(conn, project_id)?.and_then(|p| p.path))
}

pub fn update_project(conn: &Connection, id: &str, updates: &UpdateProjectInput) -> ApiResult<Option<Project>> {
    let current = match get_project(conn, id)? {
        Some(p) => p,
        None => return Ok(None),
    };
    let name = updates.name.clone().unwrap_or(current.name);
    let description = match &updates.description {
        Some(d) => d.clone(),
        None => current.description,
    };
    let status = updates.status.clone().unwrap_or(current.status);
    conn.execute(
        "UPDATE projects SET name=?1, description=?2, status=?3, updated_at=?4 WHERE id=?5",
        params![name, description, status, now_iso(), id],
    )?;
    get_project(conn, id)
}

pub fn delete_project(conn: &Connection, id: &str) -> ApiResult<()> {
    conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
    Ok(())
}

// ---------------------------------------------------------------- Requirement

/// 列表截断长度：卡片单/双行 + 存量文档前缀（「来自存量文档「…」」）稳定落在预览内。
pub const CONTENT_PREVIEW_CHARS: usize = 200;

fn truncate_chars(text: &str, n: usize) -> String {
    let mut out: String = text.chars().take(n).collect();
    if text.chars().count() > n {
        out.push('…');
    }
    out
}

/// 列表查询 SELECT：content 全列不取（轮询带宽），以 substr 预览替代；列序与 REQUIREMENT_SELECT 对齐。
const REQUIREMENT_SELECT_LIST: &str = "SELECT r.id, r.project_id, NULL AS content, substr(r.content, 1, 200) AS content_preview, \
    r.source_doc_path, r.goal_summary, r.success_criteria, r.risks, r.ambiguities, r.attachments, r.status, r.mode, r.current_step, \
    r.current_version, r.confirmed_at, r.complexity, r.complexity_reason, r.profile, r.risk_surfaces, r.lane, \
    r.estimated_minutes, r.actual_minutes, r.total_tokens, r.created_at, r.updated_at, p.name AS project_name \
    FROM requirements r JOIN projects p ON p.id = r.project_id";

fn map_requirement(row: &Row) -> rusqlite::Result<Requirement> {
    // content 列：列表查询为 NULL（只带 preview），详情查询为全量原文
    let content: Option<String> = row.get("content")?;
    let raw_preview: String = row.get("content_preview")?;
    let content_preview = truncate_chars(&raw_preview, CONTENT_PREVIEW_CHARS);
    Ok(Requirement {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        content,
        content_preview,
        source_doc_path: row.get("source_doc_path")?,
        goal_summary: row.get("goal_summary")?,
        success_criteria: json_col(row, "success_criteria")?,
        risks: json_col(row, "risks")?,
        ambiguities: json_col(row, "ambiguities")?,
        attachments: json_col(row, "attachments")?,
        status: RequirementStatus::parse(&row.get::<_, String>("status")?),
        mode: RequirementMode::parse(&row.get::<_, String>("mode")?),
        current_step: row.get("current_step")?,
        current_version: row.get::<_, i64>("current_version")?,
        confirmed_at: row.get("confirmed_at")?,
        complexity: row.get("complexity")?,
        complexity_reason: row.get("complexity_reason")?,
        profile: json_col(row, "profile")?,
        risk_surfaces: json_col(row, "risk_surfaces")?,
        lane: row.get("lane")?,
        estimated_minutes: row.get("estimated_minutes")?,
        actual_minutes: row.get("actual_minutes")?,
        total_tokens: row.get("total_tokens")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        project_name: row.get("project_name").ok(),
    })
}

const REQUIREMENT_SELECT: &str =
    "SELECT r.*, p.name AS project_name FROM requirements r JOIN projects p ON p.id = r.project_id";

/// 详情查询的 preview 表达式：map_requirement 统一读取 content_preview 列，详情场景直接由全量 content 计算。
fn with_preview(sql: &str) -> String {
    sql.replace("SELECT r.*", "SELECT r.*, substr(r.content, 1, 200) AS content_preview")
}

pub fn create_requirement(
    conn: &Connection,
    project_id: &str,
    content: &str,
    attachments: Option<&Vec<AttachmentItem>>,
    mode: RequirementMode,
    estimated_minutes: Option<i64>,
) -> ApiResult<Requirement> {
    create_requirement_with_source(conn, project_id, content, None, attachments, mode, estimated_minutes)
}

/// source_doc_path：存量文档转换的需求在此登记来源路径（结构化替代「来自存量文档「…」」信封解析）。
pub fn create_requirement_with_source(
    conn: &Connection,
    project_id: &str,
    content: &str,
    source_doc_path: Option<&str>,
    attachments: Option<&Vec<AttachmentItem>>,
    mode: RequirementMode,
    estimated_minutes: Option<i64>,
) -> ApiResult<Requirement> {
    let id = new_id();
    let ts = now_iso();
    conn.execute(
        "INSERT INTO requirements (id, project_id, content, source_doc_path, attachments, status, mode, current_version, estimated_minutes, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,'pending',?6,1,?7,?8,?8)",
        params![
            id,
            project_id,
            content,
            source_doc_path,
            attachments.map(|a| serde_json::to_string(a).unwrap()),
            mode.as_str(),
            estimated_minutes,
            ts
        ],
    )?;
    Ok(get_requirement(conn, &id)?.expect("requirement just inserted"))
}

pub fn get_requirement(conn: &Connection, id: &str) -> ApiResult<Option<Requirement>> {
    // 详情路径：取全量 content（Option），preview 由 SQL 侧同计算
    let sql = with_preview(&format!("{REQUIREMENT_SELECT} WHERE r.id = ?1"));
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map(params![id], map_requirement)?;
    match rows.next() {
        Some(Ok(mut r)) => {
            r.content_preview = truncate_chars(&r.content.clone().unwrap_or_default(), CONTENT_PREVIEW_CHARS);
            Ok(Some(r))
        }
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

#[derive(Debug, Default, Clone)]
pub struct RequirementUpdates {
    pub goal_summary: Option<String>,
    pub success_criteria: Option<Vec<UnderstandingCriteria>>,
    pub risks: Option<Vec<UnderstandingRisk>>,
    pub ambiguities: Option<Vec<UnderstandingAmbiguity>>,
    pub attachments: Option<Vec<AttachmentItem>>,
    pub status: Option<RequirementStatus>,
    pub mode: Option<RequirementMode>,
    pub current_step: Option<String>,
    pub current_version: Option<i64>,
    pub confirmed_at: Option<String>,
    pub complexity: Option<String>,
    pub complexity_reason: Option<String>,
    pub profile: Option<serde_json::Value>,
    pub risk_surfaces: Option<Vec<String>>,
    pub lane: Option<String>,
    pub estimated_minutes: Option<i64>,
    pub actual_minutes: Option<i64>,
    pub total_tokens: Option<i64>,
}

pub fn update_requirement(conn: &Connection, id: &str, u: &RequirementUpdates) -> ApiResult<Option<Requirement>> {
    let current = match get_requirement(conn, id)? {
        Some(r) => r,
        None => return Ok(None),
    };
    conn.execute(
        "UPDATE requirements SET goal_summary=?1, success_criteria=?2, risks=?3, ambiguities=?4, attachments=?5,
         status=?6, mode=?7, current_step=?8, current_version=?9, confirmed_at=?10,
         complexity=?11, complexity_reason=?12, profile=?13, risk_surfaces=?14, lane=?15,
         estimated_minutes=?16, actual_minutes=?17, total_tokens=?18,
         updated_at=?19 WHERE id=?20",
        params![
            u.goal_summary.clone().or(current.goal_summary),
            serde_json::to_string(&u.success_criteria.clone().or(current.success_criteria))?,
            serde_json::to_string(&u.risks.clone().or(current.risks))?,
            serde_json::to_string(&u.ambiguities.clone().or(current.ambiguities))?,
            u.attachments.clone().or(current.attachments).map(|a| serde_json::to_string(&a).unwrap()),
            u.status.map(|s| s.as_str().to_string()).unwrap_or(current.status.as_str().to_string()),
            u.mode.map(|m| m.as_str().to_string()).unwrap_or(current.mode.as_str().to_string()),
            u.current_step.clone().or(current.current_step),
            u.current_version.unwrap_or(current.current_version),
            u.confirmed_at.clone().or(current.confirmed_at),
            u.complexity.clone().or(current.complexity),
            u.complexity_reason.clone().or(current.complexity_reason),
            u.profile.clone().or(current.profile).map(|p| serde_json::to_string(&p).unwrap()),
            u.risk_surfaces.clone().or(current.risk_surfaces).map(|v| serde_json::to_string(&v).unwrap()),
            u.lane.clone().or(current.lane),
            u.estimated_minutes.or(current.estimated_minutes),
            u.actual_minutes.or(current.actual_minutes),
            u.total_tokens.or(current.total_tokens),
            now_iso(),
            id
        ],
    )?;
    get_requirement(conn, id)
}

pub fn list_requirements_by_project(conn: &Connection, project_id: &str) -> ApiResult<Vec<Requirement>> {
    // initializing（文档引入、理解归类中）对需求列表不可见，理解完成转 awaiting/completed 后自然出现
    let sql = format!(
        "{REQUIREMENT_SELECT_LIST} WHERE r.project_id = ?1 AND r.status != 'initializing' ORDER BY r.created_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![project_id], map_requirement)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn list_recent_requirements(conn: &Connection, limit: i64) -> ApiResult<Vec<Requirement>> {
    let sql = format!("{REQUIREMENT_SELECT_LIST} WHERE r.status != 'initializing' ORDER BY r.created_at DESC LIMIT ?1");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![limit], map_requirement)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// 初始化进度专用：仅取 initializing 队列（列表查询会过滤它们，此函数给进度入口用）。
pub fn list_initializing_requirements(conn: &Connection, project_id: &str) -> ApiResult<Vec<Requirement>> {
    let sql = format!("{REQUIREMENT_SELECT_LIST} WHERE r.project_id = ?1 AND r.status = 'initializing' ORDER BY r.created_at ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![project_id], map_requirement)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn list_all_requirements(conn: &Connection) -> ApiResult<Vec<Requirement>> {
    let sql = format!("{REQUIREMENT_SELECT_LIST} ORDER BY r.created_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_requirement)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

// ---------------------------------------------------------------- RequirementVersion

#[allow(clippy::too_many_arguments)]
pub fn create_requirement_version(
    conn: &Connection,
    requirement_id: &str,
    version: i64,
    u: &UnderstandingResult,
    questions: &[UnderstandingQuestion],
    source: UnderstandingSource,
    user_input: Option<&str>,
) -> ApiResult<RequirementVersion> {
    let id = new_id();
    conn.execute(
        "INSERT INTO requirement_versions (id, requirement_id, version, goal_summary, success_criteria, risks, ambiguities, questions, source, user_input, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            id,
            requirement_id,
            version,
            u.goal_summary,
            serde_json::to_string(&u.success_criteria)?,
            serde_json::to_string(&u.risks)?,
            serde_json::to_string(&u.ambiguities)?,
            serde_json::to_string(&questions)?,
            source.as_str(),
            user_input,
            now_iso()
        ],
    )?;
    let mut stmt = conn.prepare("SELECT * FROM requirement_versions WHERE id = ?1")?;
    let row = stmt.query_row(params![id], |row| {
        Ok(RequirementVersion {
            id: row.get("id")?,
            requirement_id: row.get("requirement_id")?,
            version: row.get("version")?,
            goal_summary: row.get("goal_summary")?,
            success_criteria: json_col(row, "success_criteria")?,
            risks: json_col(row, "risks")?,
            ambiguities: json_col(row, "ambiguities")?,
            questions: json_col(row, "questions")?,
            source: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>("source")?))
                .unwrap_or(UnderstandingSource::Initial),
            user_input: row.get("user_input")?,
            created_at: row.get("created_at")?,
        })
    })?;
    Ok(row)
}

pub fn list_requirement_versions(conn: &Connection, requirement_id: &str) -> ApiResult<Vec<RequirementVersion>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM requirement_versions WHERE requirement_id = ?1 ORDER BY version ASC",
    )?;
    let rows = stmt.query_map(params![requirement_id], |row| {
        Ok(RequirementVersion {
            id: row.get("id")?,
            requirement_id: row.get("requirement_id")?,
            version: row.get("version")?,
            goal_summary: row.get("goal_summary")?,
            success_criteria: json_col(row, "success_criteria")?,
            risks: json_col(row, "risks")?,
            ambiguities: json_col(row, "ambiguities")?,
            questions: json_col(row, "questions")?,
            source: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>("source")?))
                .unwrap_or(UnderstandingSource::Initial),
            user_input: row.get("user_input")?,
            created_at: row.get("created_at")?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn max_requirement_version(conn: &Connection, requirement_id: &str) -> ApiResult<i64> {
    let v: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM requirement_versions WHERE requirement_id = ?1",
        params![requirement_id],
        |row| row.get(0),
    )?;
    Ok(v)
}

// ---------------------------------------------------------------- Task

fn map_task(row: &Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get("id")?,
        requirement_id: row.get("requirement_id")?,
        step_type: StepType::parse(&row.get::<_, String>("step_type")?),
        title: row.get::<_, Option<String>>("title")?.unwrap_or_default(),
        description: row.get("description")?,
        status: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>("status")?))
            .unwrap_or(TaskStatus::Pending),
        result: json_col(row, "result")?,
        error_message: row.get("error_message")?,
        started_at: row.get("started_at")?,
        completed_at: row.get("completed_at")?,
        tokens: row.get("tokens")?,
        created_at: row.get("created_at")?,
    })
}

pub fn create_task(
    conn: &Connection,
    requirement_id: &str,
    step_type: StepType,
    title: &str,
    status: TaskStatus,
) -> ApiResult<Task> {
    let id = new_id();
    let ts = now_iso();
    let started = if status == TaskStatus::Running { Some(ts.clone()) } else { None };
    conn.execute(
        "INSERT INTO tasks (id, requirement_id, step_type, title, status, started_at, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![id, requirement_id, step_type.as_str(), title, status.as_str(), started, ts],
    )?;
    let mut stmt = conn.prepare("SELECT * FROM tasks WHERE id = ?1")?;
    Ok(stmt.query_row(params![id], map_task)?)
}

pub fn list_tasks_by_requirement(conn: &Connection, requirement_id: &str) -> ApiResult<Vec<Task>> {
    let mut stmt = conn
        .prepare("SELECT * FROM tasks WHERE requirement_id = ?1 ORDER BY created_at ASC")?;
    let rows = stmt.query_map(params![requirement_id], map_task)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_task(conn: &Connection, id: &str) -> ApiResult<Option<Task>> {
    let mut stmt = conn.prepare("SELECT * FROM tasks WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![id], map_task)?;
    match rows.next() {
        Some(Ok(t)) => Ok(Some(t)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

#[derive(Debug, Default, Clone)]
pub struct TaskUpdates {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub result: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub tokens: Option<i64>,
}

pub fn update_task(conn: &Connection, id: &str, u: &TaskUpdates) -> ApiResult<Option<Task>> {
    let current = match get_task(conn, id)? {
        Some(t) => t,
        None => return Ok(None),
    };
    let status = u.status.unwrap_or(current.status);
    let started_at = match (u.status, current.started_at.as_ref()) {
        (Some(TaskStatus::Running), None) => Some(now_iso()),
        _ => current.started_at,
    };
    let completed_at = match status {
        TaskStatus::Completed | TaskStatus::Failed => {
            current.completed_at.clone().or_else(|| Some(now_iso()))
        }
        _ => current.completed_at,
    };
    conn.execute(
        "UPDATE tasks SET title=?1, description=?2, status=?3, result=?4, error_message=?5, started_at=?6, completed_at=?7, tokens=?8 WHERE id=?9",
        params![
            u.title.clone().unwrap_or(current.title),
            u.description.clone().or(current.description),
            status.as_str(),
            serde_json::to_string(&u.result.clone().or(current.result))?,
            u.error_message.clone().or(current.error_message),
            started_at,
            completed_at,
            u.tokens.or(current.tokens),
            id
        ],
    )?;
    get_task(conn, id)
}

/// 聚合需求级指标：total_tokens = Σ tasks.tokens；actual_minutes = Σ 各任务 started→completed 时长（分钟，向下取整）。
/// 在需求进入终态（completed/failed）时调用。
pub fn refresh_requirement_metrics(conn: &Connection, requirement_id: &str) -> ApiResult<()> {
    let tasks = list_tasks_by_requirement(conn, requirement_id)?;
    let total_tokens: i64 = tasks.iter().filter_map(|t| t.tokens).sum();
    let mut actual_seconds: i64 = 0;
    for task in &tasks {
        let (Some(start), Some(end)) = (&task.started_at, &task.completed_at) else { continue };
        if let (Ok(s), Ok(e)) = (chrono::DateTime::parse_from_rfc3339(start), chrono::DateTime::parse_from_rfc3339(end)) {
            actual_seconds += (e - s).num_seconds().max(0);
        }
    }
    conn.execute(
        "UPDATE requirements SET total_tokens = ?1, actual_minutes = ?2, updated_at = ?3 WHERE id = ?4",
        params![total_tokens, actual_seconds / 60, now_iso(), requirement_id],
    )?;
    Ok(())
}

pub fn has_running_task(conn: &Connection, requirement_id: &str, step_types: &[StepType]) -> ApiResult<bool> {
    if step_types.is_empty() {
        return Ok(false);
    }
    // SQLite 占位符 1 起始：统一用匿名 `?` 按 position 绑定。
    let placeholders = vec!["?"; step_types.len()].join(",");
    let sql = format!(
        "SELECT COUNT(*) FROM tasks WHERE requirement_id = ? AND status = 'running' AND step_type IN ({placeholders})"
    );
    let mut params_vec: Vec<String> = vec![requirement_id.to_string()];
    params_vec.extend(step_types.iter().map(|s| s.as_str().to_string()));
    let count: i64 = conn.query_row(&sql, rusqlite::params_from_iter(params_vec), |row| row.get(0))?;
    Ok(count > 0)
}

// ---------------------------------------------------------------- Decision

fn map_decision(row: &Row) -> rusqlite::Result<Decision> {
    Ok(Decision {
        id: row.get("id")?,
        requirement_id: row.get("requirement_id")?,
        task_id: row.get("task_id")?,
        question: row.get("question")?,
        context: row.get("context")?,
        options: json_col(row, "options")?.unwrap_or_default(),
        recommended: row.get("recommended")?,
        status: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>("status")?))
            .unwrap_or(DecisionStatus::Pending),
        user_choice: json_col(row, "user_choice")?,
        resolved_at: row.get("resolved_at")?,
        created_at: row.get("created_at")?,
        requirement_content: None,
        project_name: None,
    })
}

pub fn create_decision(conn: &Connection, input: &CreateDecisionInput, task_id: Option<&str>) -> ApiResult<Decision> {
    let id = new_id();
    conn.execute(
        "INSERT INTO decisions (id, requirement_id, task_id, question, context, options, recommended, status, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,'pending',?8)",
        params![
            id,
            input.requirement_id,
            task_id,
            input.question,
            input.context,
            serde_json::to_string(&input.options)?,
            input.recommended,
            now_iso()
        ],
    )?;
    let mut stmt = conn.prepare("SELECT * FROM decisions WHERE id = ?1")?;
    Ok(stmt.query_row(params![id], map_decision)?)
}

pub fn list_decisions_by_requirement(conn: &Connection, requirement_id: &str) -> ApiResult<Vec<Decision>> {
    let mut stmt = conn
        .prepare("SELECT * FROM decisions WHERE requirement_id = ?1 ORDER BY created_at ASC")?;
    let rows = stmt.query_map(params![requirement_id], map_decision)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn list_pending_decisions_with_content(conn: &Connection, project_id: Option<&str>) -> ApiResult<Vec<Decision>> {
    let sql = format!(
        "SELECT d.*, r.content AS requirement_content, p.name AS project_name
         FROM decisions d JOIN requirements r ON r.id = d.requirement_id JOIN projects p ON p.id = r.project_id
         WHERE d.status = 'pending' {} ORDER BY d.created_at DESC",
        if project_id.is_some() { "AND r.project_id = ?1" } else { "" }
    );
    let mut stmt = conn.prepare(&sql)?;
    let map = |row: &Row| -> rusqlite::Result<Decision> {
        let mut d = map_decision(row)?;
        d.requirement_content = row.get("requirement_content").ok();
        d.project_name = row.get("project_name").ok();
        Ok(d)
    };
    let rows = if let Some(pid) = project_id {
        stmt.query_map(params![pid], map)?
    } else {
        stmt.query_map([], map)?
    };
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_decision(conn: &Connection, id: &str) -> ApiResult<Option<Decision>> {
    let mut stmt = conn.prepare("SELECT * FROM decisions WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![id], map_decision)?;
    match rows.next() {
        Some(Ok(d)) => Ok(Some(d)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

pub fn resolve_decision(
    conn: &Connection,
    id: &str,
    status: DecisionStatus,
    user_choice: Option<&DecisionOption>,
) -> ApiResult<Option<Decision>> {
    conn.execute(
        "UPDATE decisions SET status=?1, user_choice=?2, resolved_at=?3 WHERE id=?4",
        params![
            status.as_str(),
            user_choice.map(|c| serde_json::to_string(c).unwrap()),
            now_iso(),
            id
        ],
    )?;
    get_decision(conn, id)
}

// ---------------------------------------------------------------- Artifact

/// stage：阶段键（plan/implement/verify/review），结构化的版本作用域与回看匹配键；title 仅人读展示。
pub fn create_artifact(
    conn: &Connection,
    requirement_id: &str,
    task_id: Option<&str>,
    artifact_type: ArtifactType,
    stage: &str,
    title: &str,
    content: &str,
) -> ApiResult<Artifact> {
    let id = new_id();
    // version 原子化：计数与写入放同一 IMMEDIATE 事务，避免并发写同一 (requirement, stage) 时版本号撞车
    conn.execute("BEGIN IMMEDIATE", [])?;
    let version: i64 = match conn.query_row(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM artifacts WHERE requirement_id = ?1 AND stage = ?2",
        params![requirement_id, stage],
        |row| row.get(0),
    ) {
        Ok(v) => v,
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            return Err(e.into());
        }
    };
    if let Err(e) = conn.execute(
        "INSERT INTO artifacts (id, requirement_id, task_id, type, title, content, stage, version, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![id, requirement_id, task_id, artifact_type.as_str(), title, content, stage, version, now_iso()],
    ) {
        let _ = conn.execute("ROLLBACK", []);
        return Err(e.into());
    }
    conn.execute("COMMIT", [])?;
    let mut stmt = conn.prepare("SELECT * FROM artifacts WHERE id = ?1")?;
    Ok(stmt.query_row(params![id], |row| {
        Ok(Artifact {
            id: row.get("id")?,
            requirement_id: row.get("requirement_id")?,
            task_id: row.get("task_id")?,
            artifact_type: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>("type")?))
                .unwrap_or(ArtifactType::Document),
            title: row.get::<_, Option<String>>("title")?.unwrap_or_default(),
            content: row.get("content")?,
            url: row.get("url")?,
            metadata: json_col(row, "metadata")?,
            stage: row.get("stage")?,
            version: row.get("version")?,
            created_at: row.get("created_at")?,
        })
    })?)
}

pub fn list_artifacts_by_requirement(conn: &Connection, requirement_id: &str) -> ApiResult<Vec<Artifact>> {
    let mut stmt = conn
        .prepare("SELECT * FROM artifacts WHERE requirement_id = ?1 ORDER BY created_at ASC")?;
    let rows = stmt.query_map(params![requirement_id], |row| {
        Ok(Artifact {
            id: row.get("id")?,
            requirement_id: row.get("requirement_id")?,
            task_id: row.get("task_id")?,
            artifact_type: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>("type")?))
                .unwrap_or(ArtifactType::Document),
            title: row.get::<_, Option<String>>("title")?.unwrap_or_default(),
            content: row.get("content")?,
            url: row.get("url")?,
            metadata: json_col(row, "metadata")?,
            stage: row.get("stage")?,
            version: row.get("version")?,
            created_at: row.get("created_at")?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

// ---------------------------------------------------------------- ExecutionLog

pub fn create_log(
    conn: &Connection,
    requirement_id: &str,
    task_id: Option<&str>,
    step: &str,
    level: &str,
    message: &str,
    details: Option<&serde_json::Value>,
) -> ApiResult<()> {
    conn.execute(
        "INSERT INTO execution_logs (id, requirement_id, task_id, step, level, message, details, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            new_id(),
            requirement_id,
            task_id,
            step,
            level,
            message,
            details.map(|d| serde_json::to_string(d).unwrap()),
            now_iso()
        ],
    )?;
    Ok(())
}

pub fn list_logs_by_requirement(conn: &Connection, requirement_id: &str) -> ApiResult<Vec<ExecutionLog>> {
    let mut stmt = conn
        .prepare("SELECT * FROM execution_logs WHERE requirement_id = ?1 ORDER BY created_at ASC")?;
    let rows = stmt.query_map(params![requirement_id], |row| {
        Ok(ExecutionLog {
            id: row.get("id")?,
            requirement_id: row.get("requirement_id")?,
            task_id: row.get("task_id")?,
            step: row.get("step")?,
            level: row.get::<_, Option<String>>("level")?.unwrap_or_else(|| "info".into()),
            message: row.get("message")?,
            details: json_col(row, "details")?,
            created_at: row.get("created_at")?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

// ---------------------------------------------------------------- Settings KV

pub fn get_setting<T: serde::de::DeserializeOwned>(conn: &Connection, key: &str, fallback: T) -> T {
    let value: Option<String> = conn
        .query_row("SELECT value FROM app_settings WHERE key = ?1", params![key], |row| row.get(0))
        .ok();
    value.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(fallback)
}

pub fn save_setting(conn: &Connection, key: &str, value: &serde_json::Value) -> ApiResult<()> {
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1,?2,?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, serde_json::to_string(value)?, now_iso()],
    )?;
    Ok(())
}

pub fn get_orchestration_settings(conn: &Connection) -> OrchestrationSettings {
    get_setting(conn, "orchestration", OrchestrationSettings::default())
}

pub fn save_orchestration_settings(conn: &Connection, settings: &OrchestrationSettings) -> ApiResult<OrchestrationSettings> {
    save_setting(conn, "orchestration", &serde_json::to_value(settings)?)?;
    Ok(get_orchestration_settings(conn))
}

// ---------------------------------------------------------------- ProjectDoc（存量文档索引/全文缓存）

fn map_project_doc(row: &Row) -> rusqlite::Result<ProjectDoc> {
    let content_chars: Option<i64> = row.get("content_chars")?;
    let content_present: Option<String> = row.get("content")?;
    Ok(ProjectDoc {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        rel_path: row.get("rel_path")?,
        kind: row.get("kind")?,
        title: row.get("title")?,
        excerpt: row.get("excerpt")?,
        has_content: content_present.is_some(),
        content_chars,
        file_mtime: row.get("file_mtime")?,
        file_missing: row.get::<_, i64>("file_missing")? > 0,
        requirement_id: row.get("requirement_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_project_docs(conn: &Connection, project_id: &str) -> ApiResult<Vec<ProjectDoc>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM project_docs WHERE project_id = ?1 ORDER BY kind ASC, rel_path ASC",
    )?;
    let rows = stmt.query_map(params![project_id], map_project_doc)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_project_doc(conn: &Connection, id: &str) -> ApiResult<Option<ProjectDoc>> {
    let mut stmt = conn.prepare("SELECT * FROM project_docs WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![id], map_project_doc)?;
    match rows.next() {
        Some(Ok(d)) => Ok(Some(d)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

pub fn get_project_doc_content(conn: &Connection, id: &str) -> ApiResult<Option<String>> {
    let value: Option<String> = conn
        .query_row("SELECT content FROM project_docs WHERE id = ?1", params![id], |row| row.get(0))
        .map_err(ApiError::from)?;
    Ok(value)
}

pub fn upsert_project_doc(conn: &Connection, project_id: &str, doc: &crate::project_init::ScannedDoc) -> ApiResult<bool> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM project_docs WHERE project_id = ?1 AND rel_path = ?2",
            params![project_id, doc.rel_path],
            |row| row.get(0),
        )
        .ok();
    let ts = now_iso();
    match existing {
        Some(id) => {
            conn.execute(
                "UPDATE project_docs SET kind=?1, title=?2, excerpt=?3, content=?4, content_chars=?5, file_mtime=?6, file_missing=0, updated_at=?7 WHERE id=?8",
                params![
                    doc.kind,
                    doc.title,
                    doc.excerpt,
                    doc.content,
                    doc.content_chars,
                    doc.file_mtime,
                    ts,
                    id
                ],
            )?;
            Ok(false)
        }
        None => {
            conn.execute(
                "INSERT INTO project_docs (id, project_id, rel_path, kind, title, excerpt, content, content_chars, file_mtime, file_missing, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,0,?10,?10)",
                params![
                    new_id(),
                    project_id,
                    doc.rel_path,
                    doc.kind,
                    doc.title,
                    doc.excerpt,
                    doc.content,
                    doc.content_chars,
                    doc.file_mtime,
                    ts
                ],
            )?;
            Ok(true)
        }
    }
}

/// 目录里已消失的文档：保留记录与全文，标记 file_missing（数据在库，不删）。
pub fn mark_missing_project_docs(conn: &Connection, project_id: &str, present_rel_paths: &[String]) -> ApiResult<i64> {
    let existing: Vec<String> = {
        let mut stmt = conn.prepare("SELECT rel_path FROM project_docs WHERE project_id = ?1 AND file_missing = 0")?;
        let rows = stmt.query_map(params![project_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let mut marked = 0i64;
    for rel in existing {
        if !present_rel_paths.contains(&rel) {
            marked += conn.execute(
                "UPDATE project_docs SET file_missing = 1, updated_at = ?2 WHERE project_id = ?1 AND rel_path = ?3",
                params![project_id, now_iso(), rel],
            )? as i64;
        }
    }
    Ok(marked)
}

// ---------------------------------------------------------------- GovernanceItem（治理结构本体）

fn map_governance_item(row: &Row) -> rusqlite::Result<GovernanceItem> {
    let body_raw: String = row.get("body")?;
    Ok(GovernanceItem {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        kind: row.get("kind")?,
        key: row.get("key")?,
        title: row.get("title")?,
        body: serde_json::from_str(&body_raw).unwrap_or(serde_json::Value::Null),
        status: row.get("status")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_governance_items(conn: &Connection, project_id: &str, kind: Option<&str>) -> ApiResult<Vec<GovernanceItem>> {
    let sql = match kind {
        Some(_) => "SELECT * FROM governance_items WHERE project_id = ?1 AND kind = ?2 ORDER BY key ASC",
        None => "SELECT * FROM governance_items WHERE project_id = ?1 ORDER BY kind ASC, key ASC",
    };
    let mut stmt = conn.prepare(sql)?;
    let map = |row: &Row| map_governance_item(row);
    let rows = match kind {
        Some(k) => stmt.query_map(params![project_id, k], map)?,
        None => stmt.query_map(params![project_id], map)?,
    };
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_governance_item(conn: &Connection, id: &str) -> ApiResult<Option<GovernanceItem>> {
    let mut stmt = conn.prepare("SELECT * FROM governance_items WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![id], map_governance_item)?;
    match rows.next() {
        Some(Ok(item)) => Ok(Some(item)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// 幂等种子：同 (project_id, kind, key) 已存在则跳过，返回是否实际插入。
pub fn insert_governance_item_if_absent(
    conn: &Connection,
    project_id: &str,
    kind: &str,
    key: &str,
    title: &str,
    body: &serde_json::Value,
) -> ApiResult<bool> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM governance_items WHERE project_id = ?1 AND kind = ?2 AND key = ?3",
        params![project_id, kind, key],
        |row| row.get(0),
    )?;
    if exists > 0 {
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO governance_items (id, project_id, kind, key, title, body, status, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,'active',?7,?7)",
        params![new_id(), project_id, kind, key, title, serde_json::to_string(body)?, now_iso()],
    )?;
    Ok(true)
}

/// 标记文档已转为需求（防重复自动转换）。
pub fn link_doc_requirement(conn: &Connection, doc_id: &str, requirement_id: &str) -> ApiResult<()> {
    conn.execute(
        "UPDATE project_docs SET requirement_id = ?2, updated_at = ?3 WHERE id = ?1",
        params![doc_id, requirement_id, now_iso()],
    )?;
    Ok(())
}

/// 待定项作答：status → resolved，答案写回 body.answer。
pub fn resolve_governance_pending(conn: &Connection, id: &str, answer: &str) -> ApiResult<Option<GovernanceItem>> {
    let Some(mut item) = get_governance_item(conn, id)? else { return Ok(None) };
    if item.kind != "pending" {
        return Err(ApiError::bad_request("仅待定项可以作答"));
    }
    if item.status == "resolved" {
        return Err(ApiError::conflict("该待定项已解决"));
    }
    if let Some(obj) = item.body.as_object_mut() {
        obj.insert("answer".into(), serde_json::Value::String(answer.to_string()));
    }
    conn.execute(
        "UPDATE governance_items SET body = ?1, status = 'resolved', updated_at = ?2 WHERE id = ?3",
        params![serde_json::to_string(&item.body)?, now_iso(), id],
    )?;
    get_governance_item(conn, id)
}

/// 新增领域词条（幂等：同名 term 已存在则返回既有项）。
pub fn insert_term_if_absent(conn: &Connection, project_id: &str, term: &str, definition: &str, avoid: Option<&str>) -> ApiResult<GovernanceItem> {
    let key = term.trim();
    if key.is_empty() {
        return Err(ApiError::bad_request("词条名不能为空"));
    }
    let existing = list_governance_items(conn, project_id, Some("term"))?
        .into_iter()
        .find(|i| i.key == key);
    if let Some(item) = existing {
        return Ok(item);
    }
    let body = serde_json::json!({ "definition": definition, "avoid": avoid.unwrap_or_default() });
    insert_governance_item_if_absent(conn, project_id, "term", key, key, &body)?;
    Ok(list_governance_items(conn, project_id, Some("term"))?
        .into_iter()
        .find(|i| i.key == key)
        .expect("term just inserted"))
}

// ---------------------------------------------------------------- Dashboards

const IN_FLIGHT: &str =
    "('pending','understanding','questioning','awaiting_confirmation','planning','implementing','verifying','waiting_decision')";

fn compute_stats(conn: &Connection, project_id: Option<&str>) -> ApiResult<WorkspaceStats> {
    // 统计口径与需求列表一致：initializing（文档引入、理解归类中）不计入任何可见统计
    let visible_clause = if project_id.is_some() {
        "WHERE project_id = ?1 AND status != 'initializing'"
    } else {
        "WHERE status != 'initializing'"
    };
    let and = "AND";
    let count = |sql: &str, pid: Option<&str>| -> ApiResult<i64> {
        Ok(match pid {
            Some(pid) => conn.query_row(sql, params![pid], |row| row.get(0))?,
            None => conn.query_row(sql, [], |row| row.get(0))?,
        })
    };
    let pending_decisions = match project_id {
        Some(pid) => {
            conn.query_row(
                "SELECT COUNT(*) FROM decisions d JOIN requirements r ON r.id = d.requirement_id
                 WHERE d.status = 'pending' AND r.project_id = ?1",
                params![pid],
                |row| row.get(0),
            )?
        }
        None => conn.query_row("SELECT COUNT(*) FROM decisions WHERE status = 'pending'", [], |row| row.get(0))?,
    };
    Ok(WorkspaceStats {
        project_count: count("SELECT COUNT(*) FROM projects", None)?,
        total_requirements: count(&format!("SELECT COUNT(*) FROM requirements {visible_clause}"), project_id)?,
        in_progress: count(
            &format!("SELECT COUNT(*) FROM requirements {visible_clause} {and} status IN {IN_FLIGHT}"),
            project_id,
        )?,
        completed: count(
            &format!("SELECT COUNT(*) FROM requirements {visible_clause} {and} status = 'completed'"),
            project_id,
        )?,
        failed: count(
            &format!("SELECT COUNT(*) FROM requirements {visible_clause} {and} status = 'failed'"),
            project_id,
        )?,
        pending_decisions,
    })
}

pub fn get_workspace_data(conn: &Connection) -> ApiResult<WorkspaceData> {
    Ok(WorkspaceData {
        stats: compute_stats(conn, None)?,
        projects: list_projects(conn)?,
        requirements: list_all_requirements(conn)?,
        pending_decisions: list_pending_decisions_with_content(conn, None)?,
    })
}

pub fn get_project_dashboard(conn: &Connection, project_id: &str) -> ApiResult<Option<ProjectDashboard>> {
    let Some(project) = get_project(conn, project_id)? else {
        return Ok(None);
    };
    let stats = compute_stats(conn, Some(project_id))?;
    let mut project = project;
    project.req_count = Some(stats.total_requirements);
    Ok(Some(ProjectDashboard {
        project,
        stats,
        requirements: list_requirements_by_project(conn, project_id)?,
        pending_decisions: list_pending_decisions_with_content(conn, Some(project_id))?,
    }))
}

pub fn get_requirement_detail(conn: &Connection, id: &str) -> ApiResult<Option<RequirementDetail>> {
    let Some(requirement) = get_requirement(conn, id)? else {
        return Ok(None);
    };
    Ok(Some(RequirementDetail {
        requirement,
        tasks: list_tasks_by_requirement(conn, id)?,
        decisions: list_decisions_by_requirement(conn, id)?,
        artifacts: list_artifacts_by_requirement(conn, id)?,
        logs: list_logs_by_requirement(conn, id)?,
        versions: list_requirement_versions(conn, id)?,
    }))
}
