use crate::db::{self, AppState};
use crate::error::{ApiError, ApiResult};
use crate::orchestrator;
use crate::task_engine;
use crate::types::*;
use serde_json::json;
use tauri::{AppHandle, Manager, State};

/// Tauri commands —— 与 PRD《03-后端API契约》路由一一映射；
/// 错误以 ApiError{code,message} 序列化给前端（400/404/409 语义一致）。

fn with_conn<T>(state: &State<std::sync::Arc<AppState>>, f: impl FnOnce(&rusqlite::Connection) -> ApiResult<T>) -> ApiResult<T> {
    let conn = state.conn.lock().unwrap();
    f(&conn)
}

/// 唤醒理解泵（入队后调用；泵已在跑则空转返回）。
fn wake_pump(state: &State<std::sync::Arc<AppState>>) {
    crate::understanding_queue::spawn_pump_state(state.inner().clone());
}

#[tauri::command]
pub fn workspace_get(state: State<std::sync::Arc<AppState>>) -> ApiResult<WorkspaceData> {
    with_conn(&state, |conn| db::get_workspace_data(conn))
}

#[tauri::command]
pub fn projects_list(state: State<std::sync::Arc<AppState>>) -> ApiResult<Vec<Project>> {
    with_conn(&state, |conn| db::list_projects(conn))
}

#[tauri::command]
pub fn projects_create(state: State<std::sync::Arc<AppState>>, name: String, description: Option<String>) -> ApiResult<Project> {
    if name.trim().is_empty() {
        return Err(ApiError::bad_request("项目名不能为空"));
    }
    with_conn(&state, |conn| db::create_project(conn, name.trim(), description.as_deref().map(str::trim).filter(|s| !s.is_empty())))
}

/// 批量入队一批需求的初始理解：全部进 understanding_queue，由单例泵串行消化。
/// 入队即返回，泵在后台逐条执行（不并发打爆本机 runtime）。
fn enqueue_understanding(state: &State<std::sync::Arc<AppState>>, ids: Vec<String>) {
    if ids.is_empty() {
        return;
    }
    let conn = state.conn.lock().unwrap();
    for id in &ids {
        if let Err(e) = crate::understanding_queue::enqueue(&conn, id) {
            eprintln!("[understanding-queue] 入队失败 {id}: {e}");
        }
    }
    drop(conn);
    wake_pump(state);
}

/// 初始化目录为项目：选文件夹 → 建目录/git init → 扫描文档入库 → 治理种子 → 登记 → 需求类文档自动转需求。
#[tauri::command]
pub fn projects_init(state: State<std::sync::Arc<AppState>>, path: String) -> ApiResult<crate::types::InitProjectOutcome> {
    let outcome = crate::project_init::init_project(&state, &path)?;
    if !outcome.already_registered {
        enqueue_understanding(&state, outcome.report.converted_requirement_ids.clone());
    }
    Ok(outcome)
}

#[tauri::command]
pub fn projects_discover(path: String) -> ApiResult<crate::project_discovery::ProjectDiscovery> {
    crate::project_discovery::discover(&path)
}

#[tauri::command]
pub fn projects_import_confirm(
    state: State<std::sync::Arc<AppState>>,
    input: crate::project_discovery::ImportProjectInput,
) -> ApiResult<crate::project_discovery::ImportProjectResult> {
    crate::project_discovery::confirm_and_register(&state, &input)
}

#[tauri::command]
pub fn projects_reinit(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<crate::types::InitReport> {
    let report = crate::project_init::reinit_project(&state, &id)?;
    enqueue_understanding(&state, report.converted_requirement_ids.clone());
    Ok(report)
}

#[tauri::command]
pub fn projects_docs(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<Vec<crate::types::ProjectDoc>> {
    with_conn(&state, |conn| {
        if db::get_project(conn, &id)?.is_none() {
            return Err(ApiError::not_found("项目不存在"));
        }
        db::list_project_docs(conn, &id)
    })
}

/// 治理票只读源（票 #1）：读目标项目 docs/issues/index.json 映射为可上板展示的卡。
/// load_bodies 控制是否逐票读票体（首屏 false，展开详情 true）。
#[tauri::command]
pub fn projects_tickets(state: State<std::sync::Arc<AppState>>, id: String, load_bodies: Option<bool>) -> ApiResult<serde_json::Value> {
    with_conn(&state, |conn| {
        if db::get_project(conn, &id)?.is_none() {
            return Err(ApiError::not_found("项目不存在"));
        }
        crate::ticket_source::load_governance_tickets(conn, &id, load_bodies.unwrap_or(false))
    })
}

/// 读文档全文：文本类取库中缓存；文件已消失时仍可读（数据在库）。
#[tauri::command]
pub fn projects_doc_read(state: State<std::sync::Arc<AppState>>, doc_id: String) -> ApiResult<serde_json::Value> {
    with_conn(&state, |conn| {
        let doc = db::get_project_doc(conn, &doc_id)?.ok_or_else(|| ApiError::not_found("文档不存在"))?;
        if !doc.has_content {
            return Err(ApiError::bad_request("二进制文档（pdf/docx）仅登记未存全文，请在原目录查看"));
        }
        let content = db::get_project_doc_content(conn, &doc_id)?.unwrap_or_default();
        Ok(json!({ "id": doc.id, "rel_path": doc.rel_path, "kind": doc.kind, "title": doc.title, "content": content }))
    })
}

/// 需求类文档 → 需求草稿：全文（截 8000 字符）落成一条标准需求，进入正常流水线。
#[tauri::command]
pub fn projects_doc_to_requirement(state: State<std::sync::Arc<AppState>>, project_id: String, doc_id: String) -> ApiResult<Requirement> {
    let content = with_conn(&state, |conn| {
        if db::get_project(conn, &project_id)?.is_none() {
            return Err(ApiError::not_found("项目不存在"));
        }
        let doc = db::get_project_doc(conn, &doc_id)?.ok_or_else(|| ApiError::not_found("文档不存在"))?;
        if doc.project_id != project_id {
            return Err(ApiError::bad_request("文档不属于该项目"));
        }
        if !doc.has_content {
            return Err(ApiError::bad_request("二进制文档不支持转为需求"));
        }
        let full = db::get_project_doc_content(conn, &doc_id)?.unwrap_or_default();
        let mut text: String = full.chars().take(8000).collect();
        if full.chars().count() > 8000 {
            text.push_str("\n\n【已截断：全文见项目文档】");
        }
        Ok((format!("来自存量文档「{}」：\n\n{}", doc.rel_path, text), doc.rel_path))
    })?;
    let requirement = with_conn(&state, |conn| {
        let requirement = db::create_requirement_with_source(conn, &project_id, &content.0, Some(&content.1), None, RequirementMode::Standard, None)?;
        // 记录转换链接：手动转换同样防止后续自动重复转换
        db::link_doc_requirement(conn, &doc_id, &requirement.id)?;
        Ok::<_, ApiError>(requirement)
    })?;
    // 文档转需求同样进理解队列
    enqueue_understanding(&state, vec![requirement.id.clone()]);
    Ok(requirement)
}

/// 初始化进度入口：取 initializing 队列（主列表不可见，此命令供进度面板）。
#[tauri::command]
pub fn projects_initializing(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<Vec<Requirement>> {
    with_conn(&state, |conn| {
        if db::get_project(conn, &id)?.is_none() {
            return Err(ApiError::not_found("项目不存在"));
        }
        db::list_initializing_requirements(conn, &id)
    })
}

/// 初始化队列重试：initializing 的需求（理解失败）重新入理解队列，保持隐藏语义。
#[tauri::command]
pub fn requirements_retry_initializing(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<Requirement> {
    {
        let conn = state.conn.lock().unwrap();
        let requirement = db::get_requirement(&conn, &id)?.ok_or_else(|| ApiError::not_found("需求不存在"))?;
        if requirement.status != RequirementStatus::Initializing {
            return Err(ApiError::conflict("仅初始化队列中的需求可以重试"));
        }
        db::create_log(&conn, &id, None, "retry", "info", "初始化队列重试：重新理解", None)?;
        crate::understanding_queue::enqueue(&conn, &id)?;
    }
    wake_pump(&state);
    let requirement = with_conn(&state, |conn| db::get_requirement(conn, &id))?.unwrap();
    Ok(requirement)
}

/// 初始化队列批量重试：全部校验为 initializing 后入**串行**理解队列（泵逐条执行，不并发打爆 runtime）。
/// 立即返回排队条数；前端以 updated_at 是否刷新区分「排队中/理解中」。
#[tauri::command]
pub fn requirements_retry_initializing_batch(state: State<std::sync::Arc<AppState>>, ids: Vec<String>) -> ApiResult<serde_json::Value> {
    if ids.is_empty() {
        return Err(ApiError::bad_request("没有要重试的需求"));
    }
    {
        let conn = state.conn.lock().unwrap();
        for id in &ids {
            let requirement = db::get_requirement(&conn, id)?
                .ok_or_else(|| ApiError::not_found(format!("需求不存在: {id}")))?;
            if requirement.status != RequirementStatus::Initializing {
                return Err(ApiError::conflict(format!("需求 {id} 已不在初始化队列（状态 {}）", requirement.status.as_str())));
            }
        }
        for id in &ids {
            db::create_log(&conn, id, None, "retry", "info", "初始化队列批量重试：进入串行理解队列", None)?;
            crate::understanding_queue::enqueue(&conn, id)?;
        }
    }
    wake_pump(&state);
    Ok(json!({ "queued": ids.len() }))
}

#[tauri::command]
pub fn projects_governance(state: State<std::sync::Arc<AppState>>, id: String, kind: Option<String>) -> ApiResult<Vec<crate::types::GovernanceItem>> {
    with_conn(&state, |conn| {
        if db::get_project(conn, &id)?.is_none() {
            return Err(ApiError::not_found("项目不存在"));
        }
        db::list_governance_items(conn, &id, kind.as_deref())
    })
}

/// 待定项人工作答：pending → resolved，答案写回 body。
#[tauri::command]
pub fn projects_governance_answer(state: State<std::sync::Arc<AppState>>, item_id: String, answer: String) -> ApiResult<crate::types::GovernanceItem> {
    if answer.trim().is_empty() {
        return Err(ApiError::bad_request("回答内容不能为空"));
    }
    with_conn(&state, |conn| {
        db::resolve_governance_pending(conn, &item_id, answer.trim())?
            .ok_or_else(|| ApiError::not_found("待定项不存在"))
    })
}

/// 治理补全：自动发一条补全需求，走现有流水线（codex 查证自答，查不到走 question 问用户），结果写回治理条目。
#[tauri::command]
pub fn projects_governance_complete(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<Requirement> {
    let pending = with_conn(&state, |conn| {
        Ok::<_, ApiError>(
            db::list_governance_items(conn, &id, Some("pending"))?
                .into_iter()
                .filter(|i| i.status == "active")
                .collect::<Vec<_>>(),
        )
    })?;
    if pending.is_empty() {
        return Err(ApiError::conflict("没有待补全的治理条目"));
    }
    let list: Vec<String> = pending
        .iter()
        .map(|i| {
            let question = i.body.get("question").and_then(|v| v.as_str()).unwrap_or(&i.title);
            let hint = i.body.get("hint").and_then(|v| v.as_str()).unwrap_or("");
            format!("- [{}] {}（提示：{hint}）", i.key, question)
        })
        .collect();
    let content = format!(
        "治理补全：逐条回答以下待定项并给出答案 JSON。\n铁律（agent-up 协议）：仓库里能查证的事实自己查证后作答（如验证命令查 package.json scripts / Cargo.toml / Makefile）；\
查不到、需要人拍板的不得编造，标注「需用户裁定」。遵守三道门禁与措辞禁令。\n\n待定项：\n{}\n\n输出 JSON 契约（强制）：\
{{\"answers\":[{{\"key\":string,\"answer\":string,\"needs_user\":boolean}}],\"terms\":[{{\"term\":string,\"definition\":string,\"avoid\":string}}]}}\n只输出 JSON。",
        list.join("\n")
    );
    let requirement = with_conn(&state, |conn| {
        let requirement = db::create_requirement(conn, &id, &content, None, RequirementMode::Standard, Some(10))?;
        db::create_log(conn, &requirement.id, None, "init", "info", "治理补全需求已创建，开始理解", None)?;
        Ok::<_, ApiError>(requirement)
    })?;
    enqueue_understanding(&state, vec![requirement.id.clone()]);
    Ok(requirement)
}

/// 导出：agentup-files（技能兼容文件集投影到项目目录）或 json-snapshot（全量 JSON 到指定路径）。
#[tauri::command]
pub fn projects_export(state: State<std::sync::Arc<AppState>>, id: String, kind: String, target: Option<String>) -> ApiResult<serde_json::Value> {
    match kind.as_str() {
        "agentup-files" => {
            let outcome = crate::project_init::export_agentup_files(&state, &id)?;
            Ok(json!(outcome))
        }
        "json-snapshot" => {
            let target = target.filter(|t| !t.trim().is_empty()).ok_or_else(|| ApiError::bad_request("JSON 快照需要指定目标路径 target"))?;
            let outcome = crate::project_init::export_json_snapshot(&state, &id, &target)?;
            Ok(json!(outcome))
        }
        other => Err(ApiError::bad_request(format!("未知导出类型: {other}"))),
    }
}

#[tauri::command]
pub fn projects_get(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<ProjectDashboard> {
    with_conn(&state, |conn| {
        db::get_project_dashboard(conn, &id)?.ok_or_else(|| ApiError::not_found("项目不存在"))
    })
}

#[tauri::command]
pub fn projects_update(state: State<std::sync::Arc<AppState>>, id: String, updates: UpdateProjectInput) -> ApiResult<Project> {
    with_conn(&state, |conn| {
        db::update_project(conn, &id, &updates)?.ok_or_else(|| ApiError::not_found("项目不存在"))
    })
}

#[tauri::command]
pub fn projects_delete(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<serde_json::Value> {
    {
        let conn = state.conn.lock().unwrap();
        let conn = &*conn;
        if db::get_project(conn, &id)?.is_none() {
            return Err(ApiError::not_found("项目不存在"));
        }
        // 级联删库行前先收集附件 key（requirements/projects 级联删除后无从查起）
        let requirement_ids: Vec<String> = conn
            .prepare("SELECT id FROM requirements WHERE project_id = ?1")?
            .query_map(rusqlite::params![id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let keys = crate::attachments::collect_attachment_keys(conn, &requirement_ids);
        db::delete_project(conn, &id)?;
        crate::attachments::delete_attachment_files(&state.attachments_dir, &keys);
    }
    Ok(json!({ "deleted": true }))
}

// Native projects must never enter the legacy auto-execution pipeline, even
// through stale frontend state or a direct IPC request.
fn ensure_legacy_requirement_creation(path: Option<&str>) -> ApiResult<()> {
    let Some(path) = path else { return Ok(()); };
    match std::fs::symlink_metadata(std::path::Path::new(path).join(".agentup-app")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
        Ok(_) => {},
    }
    if crate::project_discovery::open_native_project(path)?.1.is_some() {
        return Err(ApiError::conflict("此项目已使用原生目标工作区，请进入项目新建目标并确认任务计划"));
    }
    Ok(())
}

#[tauri::command]
pub async fn requirements_create(
    app: AppHandle,
    input: CreateRequirementInput,
) -> ApiResult<serde_json::Value> {
    let state: State<std::sync::Arc<AppState>> = app.state();
    if input.content.trim().chars().count() < 4 {
        return Err(ApiError::bad_request("需求内容至少 4 个字"));
    }
    let project = with_conn(&state, |conn| db::get_project(conn, &input.project_id))?
        .ok_or_else(|| ApiError::not_found("归属项目不存在"))?;
    ensure_legacy_requirement_creation(project.path.as_deref())?;
    let attachments = db::with_attachments_dir(&state, |dir| crate::attachments::persist_attachments(dir, &input.attachments))?;
    let (requirement, count) = with_conn(&state, |conn| {
        if db::get_project(conn, &input.project_id)?.is_none() {
            return Err(ApiError::not_found("归属项目不存在"));
        }
        let attachments_opt = if attachments.is_empty() { None } else { Some(attachments.clone()) };
        let requirement = db::create_requirement(
            conn,
            &input.project_id,
            input.content.trim(),
            attachments_opt.as_ref(),
            RequirementMode::Standard,
            input.estimated_minutes,
        )?;
        db::create_log(conn, &requirement.id, None, "init", "info", "需求已创建，开始理解", None)?;
        Ok::<_, ApiError>((requirement, attachments.len()))
    })?;

    // 入理解队列：需求立刻返回，泵串行消化，前端 5s 轮询刷新
    enqueue_understanding(&state, vec![requirement.id.clone()]);

    let fresh = with_conn(&state, |conn| db::get_requirement(conn, &requirement.id))?.unwrap_or(requirement);
    Ok(json!({
        "requirement": fresh,
        "version": { "version": 1 },
        "attachment_count": count,
    }))
}

#[tauri::command]
pub fn requirements_get(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<RequirementDetail> {
    with_conn(&state, |conn| {
        db::get_requirement_detail(conn, &id)?.ok_or_else(|| ApiError::not_found("需求不存在"))
    })
}

#[tauri::command]
pub fn requirements_list_tasks(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<Vec<Task>> {
    with_conn(&state, |conn| db::list_tasks_by_requirement(conn, &id))
}

#[tauri::command]
pub fn requirements_list_decisions(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<Vec<Decision>> {
    with_conn(&state, |conn| db::list_decisions_by_requirement(conn, &id))
}

#[tauri::command]
pub async fn requirements_retry(app: AppHandle, state: State<'_, std::sync::Arc<AppState>>, id: String) -> ApiResult<serde_json::Value> {
    let (requirement, route) = orchestrator::retry_failed(&state, &id)?;
    match route {
        orchestrator::RetryRoute::Pipeline => {
            orchestrator::spawn_pipeline(app, id.clone());
        }
        orchestrator::RetryRoute::Understanding => {
            {
                let conn = state.conn.lock().unwrap();
                crate::understanding_queue::enqueue(&conn, &id)?;
            }
            wake_pump(&state);
        }
    }
    Ok(serde_json::json!({ "requirement": requirement, "route": match route {
        orchestrator::RetryRoute::Pipeline => "pipeline",
        orchestrator::RetryRoute::Understanding => "understanding",
    } }))
}

#[tauri::command]
pub fn requirements_confirm(app: AppHandle, state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<serde_json::Value> {
    {
        let conn = state.conn.lock().unwrap();
        let requirement = db::get_requirement(&conn, &id)?.ok_or_else(|| ApiError::not_found("需求不存在"))?;
        if !matches!(requirement.status, RequirementStatus::Understanding | RequirementStatus::AwaitingConfirmation) {
            return Err(ApiError::conflict(format!("当前状态({})不允许确认需求", requirement.status.as_str())));
        }
    }
    let Some((plan_task, pending_tasks)) = orchestrator::start_planning_sync(&state, &id)? else {
        return Err(ApiError::not_found("需求不存在"));
    };
    orchestrator::spawn_pipeline(app, id.clone());
    let requirement = with_conn(&state, |conn| db::get_requirement(conn, &id))?.unwrap();
    Ok(json!({
        "requirement": requirement,
        "planTask": plan_task,
        "pendingTasks": pending_tasks,
    }))
}

#[tauri::command]
pub async fn requirements_reunderstand(app: AppHandle, id: String, input: ReunderstandInput) -> ApiResult<serde_json::Value> {
    let state: State<std::sync::Arc<AppState>> = app.state();
    {
        let conn = state.conn.lock().unwrap();
        if db::get_requirement(&conn, &id)?.is_none() {
            return Err(ApiError::not_found("需求不存在"));
        }
    }
    if input.feedback.is_none() && !input.manual.unwrap_or(false) {
        return Err(ApiError::bad_request("缺少反馈内容"));
    }
    if orchestrator::has_in_flight_understanding(&state, &id)? {
        return Err(ApiError::conflict("上一次理解/迭代仍在处理中，请稍候再试"));
    }
    let stored = db::with_attachments_dir(&state, |dir| crate::attachments::persist_attachments(dir, &input.attachments))?;
    let mut ctx_input = input.clone();
    if !stored.is_empty() {
        // 将新附件合并进 requirements.attachments
        let conn = state.conn.lock().unwrap();
        let current = db::get_requirement(&conn, &id)?.map(|r| r.attachments.unwrap_or_default()).unwrap_or_default();
        let mut merged = current;
        merged.extend(stored);
        db::update_requirement(&conn, &id, &db::RequirementUpdates { attachments: Some(merged), ..Default::default() })?;
        ctx_input.attachments.clear();
    }
    let state_inner = state.inner().clone();
    let id_inner = id.clone();
    orchestrator::start_reunderstand(&state_inner, &id_inner, &ctx_input).await?;

    let conn = state.conn.lock().unwrap();
    let updated = db::get_requirement(&conn, &id)?.ok_or_else(|| ApiError::not_found("需求不存在"))?;
    let versions = db::list_requirement_versions(&conn, &id)?;
    let latest = versions.last();
    Ok(json!({
        "requirement": updated,
        "understanding": {
            "goal_summary": updated.goal_summary,
            "success_criteria": updated.success_criteria,
            "risks": updated.risks,
            "ambiguities": updated.ambiguities,
            "questions": latest.and_then(|v| v.questions.clone()).unwrap_or_default(),
            "mode": updated.mode,
        },
        "version": { "version": latest.map(|v| v.version).unwrap_or(updated.current_version) },
        "has_questions": latest.and_then(|v| v.questions.as_ref()).map(|q| !q.is_empty()).unwrap_or(false),
    }))
}

#[tauri::command]
pub async fn requirements_iterate(app: AppHandle, id: String, input: IterateInput) -> ApiResult<serde_json::Value> {
    let state: State<std::sync::Arc<AppState>> = app.state();
    {
        let conn = state.conn.lock().unwrap();
        if db::get_requirement(&conn, &id)?.is_none() {
            return Err(ApiError::not_found("需求不存在"));
        }
    }
    if input.feedback.trim().is_empty() && input.attachments.is_empty() {
        return Err(ApiError::bad_request("反馈内容不能为空"));
    }
    if orchestrator::has_in_flight_understanding(&state, &id)? {
        return Err(ApiError::conflict("上一次理解/迭代仍在处理中，请稍候再试"));
    }
    let stored = db::with_attachments_dir(&state, |dir| crate::attachments::persist_attachments(dir, &input.attachments))?;
    if !stored.is_empty() {
        let conn = state.conn.lock().unwrap();
        let current = db::get_requirement(&conn, &id)?.map(|r| r.attachments.unwrap_or_default()).unwrap_or_default();
        let mut merged = current;
        merged.extend(stored.clone());
        db::update_requirement(&conn, &id, &db::RequirementUpdates { attachments: Some(merged), ..Default::default() })?;
    }
    let state_inner = state.inner().clone();
    let id_inner = id.clone();
    orchestrator::start_iterate(&state_inner, &id_inner, &input, stored).await?;

    let conn = state.conn.lock().unwrap();
    let updated = db::get_requirement(&conn, &id)?.ok_or_else(|| ApiError::not_found("需求不存在"))?;
    let versions = db::list_requirement_versions(&conn, &id)?;
    let latest = versions.last();
    Ok(json!({
        "requirement": updated,
        "understanding": {
            "goal_summary": updated.goal_summary,
            "success_criteria": updated.success_criteria,
            "risks": updated.risks,
            "ambiguities": updated.ambiguities,
            "questions": latest.and_then(|v| v.questions.clone()).unwrap_or_default(),
            "mode": updated.mode,
        },
        "version": { "version": latest.map(|v| v.version).unwrap_or(updated.current_version) },
    }))
}

#[tauri::command]
pub fn requirements_create_decision(state: State<std::sync::Arc<AppState>>, input: CreateDecisionInput) -> ApiResult<Decision> {
    if input.question.trim().is_empty() || input.options.is_empty() {
        return Err(ApiError::bad_request("缺少必要字段（requirement_id/question/options）"));
    }
    with_conn(&state, |conn| db::create_decision(conn, &input, None))
}

#[tauri::command]
pub async fn decisions_resolve(app: AppHandle, input: ResolveDecisionInput) -> ApiResult<Decision> {
    let state: State<std::sync::Arc<AppState>> = app.state();
    let (decision, _choice) = {
        let conn = state.conn.lock().unwrap();
        let decision = db::get_decision(&conn, &input.id)?.ok_or_else(|| ApiError::not_found("决策不存在"))?;
        if decision.status != DecisionStatus::Pending {
            return Err(ApiError::conflict("该决策已处理"));
        }
        (decision, input.choice.clone())
    };
    let resolved = with_conn(&state, |conn| {
        let resolved = db::resolve_decision(conn, &input.id, DecisionStatus::Resolved, Some(&input.choice))?
            .ok_or_else(|| ApiError::not_found("决策不存在"))?;
        db::create_log(
            conn,
            &decision.requirement_id,
            decision.task_id.as_deref(),
            "decision",
            "info",
            &format!("决策已解决：{}", input.choice.label),
            Some(&json!({ "question": decision.question, "choice": input.choice })),
        )?;
        Ok::<_, ApiError>(resolved)
    })?;
    // 决策后恢复实施流水线
    let arc = state.inner().clone();
    orchestrator::resume_pipeline(&arc, &decision.requirement_id).await;
    Ok(resolved)
}

// ---------------------------------------------------------------- Orchestration & Roles

#[tauri::command]
pub fn settings_get_orchestration(state: State<std::sync::Arc<AppState>>) -> ApiResult<OrchestrationSettings> {
    with_conn(&state, |conn| Ok(db::get_orchestration_settings(conn)))
}

#[tauri::command]
pub fn settings_save_orchestration(state: State<std::sync::Arc<AppState>>, input: SaveOrchestrationInput) -> ApiResult<OrchestrationSettings> {
    with_conn(&state, |conn| {
        let mut current = db::get_orchestration_settings(conn);
        if let Some(default_runtime) = input.default_runtime {
            // 空 string = 清除回落注册表默认；非空必须是注册表内的 id
            if default_runtime.as_deref().unwrap_or("").is_empty() {
                current.default_runtime = None;
            } else {
                let id = default_runtime.unwrap();
                crate::agent_runtime::spec_by_id(&id)
                    .ok_or_else(|| ApiError::bad_request(format!("未知的 runtime: {id}")))?;
                current.default_runtime = Some(id);
            }
        }
        if let Some(stage_runtimes) = input.stage_runtimes {
            for (stage, value) in stage_runtimes {
                match value {
                    Some(id) if !id.is_empty() => {
                        if crate::roles::RoleStage::parse(&stage).is_none() {
                            return Err(ApiError::bad_request(format!("未知的阶段: {stage}")));
                        }
                        crate::agent_runtime::spec_by_id(&id)
                            .ok_or_else(|| ApiError::bad_request(format!("未知的 runtime: {id}")))?;
                        current.stage_runtimes.insert(stage, id);
                    }
                    _ => {
                        current.stage_runtimes.remove(&stage);
                    }
                }
            }
        }
        db::save_orchestration_settings(conn, &current)
    })
}

#[tauri::command]
pub fn roles_list(state: State<std::sync::Arc<AppState>>) -> ApiResult<Vec<crate::roles::RoleInfo>> {
    crate::roles::list_roles(&state)
}

#[tauri::command]
pub fn roles_get(state: State<std::sync::Arc<AppState>>, stage: String) -> ApiResult<crate::roles::RoleInfo> {
    let stage = crate::roles::RoleStage::parse(&stage).ok_or_else(|| ApiError::bad_request(format!("未知的阶段: {stage}")))?;
    crate::roles::get_role(&state, stage)
}

#[tauri::command]
pub fn roles_save(state: State<std::sync::Arc<AppState>>, stage: String, content: String) -> ApiResult<crate::roles::RoleInfo> {
    let stage = crate::roles::RoleStage::parse(&stage).ok_or_else(|| ApiError::bad_request(format!("未知的阶段: {stage}")))?;
    crate::roles::save_role(&state, stage, &content)
}

#[tauri::command]
pub fn roles_reset(state: State<std::sync::Arc<AppState>>, stage: String) -> ApiResult<crate::roles::RoleInfo> {
    let stage = crate::roles::RoleStage::parse(&stage).ok_or_else(|| ApiError::bad_request(format!("未知的阶段: {stage}")))?;
    crate::roles::reset_role(&state, stage)
}

#[tauri::command]
pub fn runtimes_list() -> Vec<crate::agent_runtime::RuntimeInfo> {
    crate::agent_runtime::probe_all()
}

/// 取消需求当前运行中的 runtime 调用：置取消标志，等待侧轮询到即 kill 进程，
/// orchestrator 以「用户取消」落 failed 任务（理解阶段回 awaiting_confirmation）。
/// 无运行中调用（已完成/未开始/mock 路径）返回 409。
#[tauri::command]
pub fn requirements_cancel(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<serde_json::Value> {
    // 找到该需求当前 running 的任务，按「requirement:task」粒度取消（理解阶段才有 task 级；
    // 流水线阶段 cancel key 用 requirement:stage，逐个尝试）
    let running_tasks: Vec<(String, String)> = with_conn(&state, |conn| {
        Ok(db::list_tasks_by_requirement(conn, &id)?
            .into_iter()
            .filter(|t| t.status == TaskStatus::Running)
            .map(|t| (t.id, t.step_type.as_str().to_string()))
            .collect())
    })?;
    let mut cancelled = false;
    if running_tasks.iter().any(|(_, step)| step == "understand" || step == "iterate") {
        for (task_id, _) in &running_tasks {
            if crate::agent_runtime::cancel(&id, task_id) {
                cancelled = true;
            }
        }
    }
    // 流水线阶段（plan/implement/verify/review）+ 理解兜底：逐 stage key 尝试（key 格式与后缀出处见 agent_runtime::child_key）
    for stage in crate::agent_runtime::CANCEL_STAGE_KEYS {
        if crate::agent_runtime::cancel(&id, stage) {
            cancelled = true;
        }
    }
    if !cancelled {
        return Err(ApiError::conflict("当前没有可取消的运行中调用"));
    }
    let _ = db::create_log(
        &state.conn.lock().unwrap(),
        &id,
        None,
        "cancel",
        "warn",
        "用户请求取消当前运行",
        None,
    );
    Ok(json!({ "cancelled": true }))
}

/// 连通性检测：向指定 runtime 发最小 prompt，验证 CLI/API/模型全链路。
#[tauri::command]
pub async fn runtimes_check(app: tauri::AppHandle, id: String) -> ApiResult<serde_json::Value> {
    let _state: tauri::State<std::sync::Arc<AppState>> = app.state();
    let spec = crate::agent_runtime::spec_by_id(&id)
        .ok_or_else(|| ApiError::bad_request(format!("未知的 runtime: {id}")))?;
    let runtime_name = spec.name;
    match crate::agent_runtime::check_connectivity(spec).await {
        Ok((reply, latency_ms)) => Ok(json!({
            "ok": true,
            "runtime": runtime_name,
            "reply": reply.chars().take(120).collect::<String>(),
            "latency_ms": latency_ms,
        })),
        Err(e) => Ok(json!({
            "ok": false,
            "runtime": runtime_name,
            "error": e.message,
        })),
    }
}

/// 打开 App 类运行时的宿主应用（zcode-app → ZCode 桌面），供「查看会话」跳转。
#[tauri::command]
pub fn runtimes_open_app(id: String) -> ApiResult<serde_json::Value> {
    let app_name = match id.as_str() {
        "zcode-app" => "ZCode",
        other => return Err(ApiError::bad_request(format!("该运行时无宿主 App 可打开: {other}"))),
    };
    let status = std::process::Command::new("open")
        .arg("-a")
        .arg(app_name)
        .status()
        .map_err(|e| ApiError::internal(format!("打开 {app_name} 失败: {e}")))?;
    if !status.success() {
        return Err(ApiError::internal(format!("打开 {app_name} 退出码非 0")));
    }
    Ok(json!({ "opened": true, "app": app_name }))
}

#[tauri::command]
pub fn settings_get_runtime(state: State<std::sync::Arc<AppState>>) -> ApiResult<String> {
    let settings = with_conn(&state, |conn| Ok(db::get_orchestration_settings(conn)))?;
    Ok(settings
        .default_runtime
        .unwrap_or_else(|| crate::agent_runtime::DEFAULT_RUNTIME_ID.to_string()))
}

#[tauri::command]
pub fn settings_set_runtime(state: State<std::sync::Arc<AppState>>, id: String) -> ApiResult<String> {
    // 只接受注册表里的 id（票 44 的边界：不做自由路径输入）
    let spec = crate::agent_runtime::spec_by_id(&id)
        .ok_or_else(|| ApiError::bad_request(format!("未知的 runtime: {id}")))?;
    let info = crate::agent_runtime::probe_runtime(spec);
    if !info.available {
        return Err(ApiError::bad_request(format!("{} 在本机不可用，无法选择", spec.name)));
    }
    with_conn(&state, |conn| {
        let mut current = db::get_orchestration_settings(conn);
        current.default_runtime = Some(id.clone());
        db::save_orchestration_settings(conn, &current)?;
        Ok(id)
    })
}

#[tauri::command]
pub fn mode_steps(mode: RequirementMode) -> serde_json::Value {
    let steps = task_engine::get_mode_steps(mode);
    json!({
        "mode": steps.mode.as_str(),
        "steps": steps.steps.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        "requires_user_confirmation": steps.requires_user_confirmation,
        "auto_verify": steps.auto_verify,
    })
}

#[cfg(test)]
mod native_requirement_guard_tests {
    use super::ensure_legacy_requirement_creation;

    #[test]
    fn native_projects_cannot_enter_legacy_creation() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().to_str().unwrap();
        assert!(ensure_legacy_requirement_creation(None).is_ok());
        assert!(ensure_legacy_requirement_creation(Some(path)).is_ok());
        assert!(std::process::Command::new("git").args(["init", "-q", path]).status().unwrap().success());
        let preview = crate::project_discovery::discover(path).unwrap();
        crate::project_discovery::confirm(&crate::project_discovery::ImportProjectInput {
            path: path.into(), fingerprint: preview.fingerprint, selected_sources: vec![],
        }).unwrap();
        assert_eq!(ensure_legacy_requirement_creation(Some(path)).unwrap_err().code, 409);
        std::fs::write(temp.path().join(".agentup-app/project.json"), "malformed").unwrap();
        assert_eq!(ensure_legacy_requirement_creation(Some(path)).unwrap_err().code, 400);
        assert_eq!(std::fs::read_to_string(temp.path().join(".agentup-app/project.json")).unwrap(), "malformed");
    }
}
