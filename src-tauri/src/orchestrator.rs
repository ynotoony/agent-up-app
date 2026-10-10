use crate::agent_runtime;
use crate::db::{self, AppState};
use crate::error::{ApiError, ApiResult};
use crate::roles::RoleStage;
use crate::task_engine::{self, get_mode_steps};
use crate::types::*;
use serde_json::json;
use tauri::Manager;

/// 执行编排器：理解/方案/实施/验证/审查异步推进，状态落 SQLite，前端 5s 轮询消费。
/// 对应 PRD《04》§4 + 《03》confirm/reunderstand/iterate/resolve 的服务端语义。

fn log(conn: &rusqlite::Connection, requirement_id: &str, step: &str, message: &str) {
    let _ = db::create_log(conn, requirement_id, None, step, "info", message, None);
}

fn log_task(
    conn: &rusqlite::Connection,
    requirement_id: &str,
    task_id: &str,
    step: &str,
    level: &str,
    message: &str,
) {
    let _ = db::create_log(
        conn,
        requirement_id,
        Some(task_id),
        step,
        level,
        message,
        None,
    );
}

// ---------------------------------------------------------------- 运行时与角色

/// 按阶段解析运行时：stage_runtimes[stage] 优先，review 跟随 verify，最后落 default_runtime（再落注册表默认）。
fn resolve_stage_runtime(
    settings: &OrchestrationSettings,
    stage: RoleStage,
) -> &'static agent_runtime::RuntimeSpec {
    let configured = |key: &str| -> Option<&'static agent_runtime::RuntimeSpec> {
        settings
            .stage_runtimes
            .get(key)
            .and_then(|id| agent_runtime::spec_by_id(id))
    };
    let default = || {
        settings
            .default_runtime
            .as_deref()
            .and_then(agent_runtime::spec_by_id)
            .unwrap_or_else(|| agent_runtime::resolve_spec(None))
    };
    match stage {
        RoleStage::Review => configured("review")
            .or_else(|| configured("verify"))
            .unwrap_or_else(default),
        stage => configured(stage.key()).unwrap_or_else(default),
    }
}

/// 组装一次调用的完整 prompt：读取阶梯上下文（治理规则→文档→目录）+ 角色 agentmd + 引擎载荷（数据 + JSON 契约）。
/// 契约由代码追加在末尾，用户编辑角色文件不会破坏输出解析。
fn stage_prompt(
    state: &AppState,
    stage: RoleStage,
    payload: String,
    json_contract: &str,
    context_block: &str,
) -> String {
    let role_md = crate::roles::get_role(state, stage)
        .map(|r| r.content)
        .unwrap_or_default();
    let role_md = role_md.trim();
    let role_block = if role_md.is_empty() {
        // 空文件视同默认：回种子内容
        crate::roles::default_role_md(stage).to_string()
    } else {
        role_md.to_string()
    };
    let context = context_block.trim();
    let context_section = if context.is_empty() {
        String::new()
    } else {
        format!("{context}\n\n---\n\n")
    };
    format!("{context_section}{role_block}\n\n---\n\n{payload}\n\n输出 JSON 契约（强制）：{json_contract}\n只输出 JSON，不要输出任何其它文字、解释或 markdown 代码块。")
}

/// 项目工作目录（workdir）+ 读取阶梯上下文块；未绑定目录的项目两者皆空。
fn project_context(
    state: &AppState,
    project_id: &str,
    stage_key: &str,
) -> (Option<std::path::PathBuf>, String) {
    let conn = state.conn.lock().unwrap();
    let workdir = db::get_project_path(&conn, project_id)
        .ok()
        .flatten()
        .map(std::path::PathBuf::from);
    let block = crate::project_init::build_context_block(&conn, project_id, stage_key);
    (workdir, block)
}

/// 模拟引擎的确定性 token 估算（非真实计数：按字符数/4 估）。
fn estimate_tokens(prompt_len: usize, output_len: usize) -> i64 {
    ((prompt_len + output_len + 3) / 4) as i64
}

/// 记录每阶段实际使用的运行时 + 角色 mtime，便于归因。
/// 结构化数据落 details JSON（executor/runtime_kind/role_mtime），message 只是人读镜像——前端不得反解析 message。
fn log_stage_execution(
    conn: &rusqlite::Connection,
    requirement_id: &str,
    task_id: Option<&str>,
    step: &str,
    spec: &agent_runtime::RuntimeSpec,
    role_mtime: Option<&str>,
) {
    let mtime_note = role_mtime
        .map(|t| format!("，角色版本 {t}"))
        .unwrap_or_default();
    let details = serde_json::json!({
        "executor": spec.name,
        "runtime_kind": spec.kind,
        "runtime_id": spec.id,
        "role_mtime": role_mtime,
    });
    let _ = db::create_log(
        conn,
        requirement_id,
        task_id,
        step,
        "info",
        &format!("执行者：{}{mtime_note}", spec.name),
        Some(&details),
    );
}

// ---------------------------------------------------------------- 理解

struct UnderstandingOutcome {
    requirement: Requirement,
    #[allow(dead_code)]
    understanding: UnderstandingResult,
    version: i64,
    #[allow(dead_code)]
    questions: Vec<UnderstandingQuestion>,
}

async fn run_understanding(
    state: &AppState,
    requirement_id: &str,
    source: UnderstandingSource,
    processing_status: RequirementStatus,
    final_status: RequirementStatus,
    failure_status: RequirementStatus,
    user_input: Option<String>,
    context: crate::understanding::UnderstandingContext,
) -> ApiResult<Option<UnderstandingOutcome>> {
    let settings = db::get_orchestration_settings(&state.conn.lock().unwrap());
    let runtime_spec = resolve_stage_runtime(&settings, RoleStage::Understand);

    // 文档引入的需求（initializing）全程保持隐藏：理解期间状态不变，结束时才转 final（待确认/已归档）；
    // 失败也留在 initializing（可由队列重试），不把半成品漏进需求列表。
    let (processing_status, final_status, failure_status) = {
        let conn = state.conn.lock().unwrap();
        let Some(requirement) = db::get_requirement(&conn, requirement_id)? else {
            return Ok(None);
        };
        if requirement.status == RequirementStatus::Initializing
            && matches!(source, UnderstandingSource::Initial)
        {
            (
                RequirementStatus::Initializing,
                final_status,
                RequirementStatus::Initializing,
            )
        } else {
            (processing_status, final_status, failure_status)
        }
    };

    let task = {
        let conn = state.conn.lock().unwrap();
        let Some(_requirement) = db::get_requirement(&conn, requirement_id)? else {
            return Ok(None);
        };
        db::create_task(
            &conn,
            requirement_id,
            StepType::Understand,
            "理解需求",
            TaskStatus::Running,
        )?
    };

    {
        let conn = state.conn.lock().unwrap();
        db::update_requirement(
            &conn,
            requirement_id,
            &db::RequirementUpdates {
                status: Some(processing_status),
                current_step: Some("understand".into()),
                ..Default::default()
            },
        )?;
        let message = if matches!(source, UnderstandingSource::Initial) {
            "开始理解需求".to_string()
        } else {
            format!("触发重新理解（来源：{}）", source.as_str())
        };
        log(&conn, requirement_id, "understand", &message);
    }

    let result = (|| async {
        let (content, prior_versions, next_version, fallback_mode, workdir, context_block) = {
            let conn = state.conn.lock().unwrap();
            let requirement = db::get_requirement(&conn, requirement_id)?
                .ok_or_else(|| ApiError::not_found("需求不存在"))?;
            let prior_versions = db::list_requirement_versions(&conn, requirement_id)?;
            let next_version = prior_versions.iter().map(|v| v.version).max().unwrap_or(0) + 1;
            // 读取阶梯上下文：在已持有的锁内构建（conn 不可重入，禁止再经 project_context 二次加锁）
            let workdir = db::get_project_path(&conn, &requirement.project_id)
                .ok()
                .flatten()
                .map(std::path::PathBuf::from);
            let context_block = crate::project_init::build_context_block(&conn, &requirement.project_id, "understand");
            Ok::<_, ApiError>((requirement.content.unwrap_or_default(), prior_versions, next_version, requirement.mode, workdir, context_block))
        }?;

        // LLM 调用失败时自动降级到本地模拟引擎，保证链路不中断（日志中说明降级原因）。
        let mut payload = crate::understanding::build_understanding_payload(
            &content,
            &crate::understanding::UnderstandOptions {
                context: context.clone(),
                prior_versions,
                fallback_mode,
            },
        );
        // 自动归类任务：结合目录条目与文档内容判断该需求是否"已经做完/纯资料"
        payload.push_str(
            "\n\n补充判定任务：already_done = 该需求描述的工作是否已在当前项目目录中完成，或该内容仅为既存事实/资料的记录。\
结合上方「项目目录条目」「存量文档摘要」与需求内容判断：已实现/已上线/纯资料记录 → true；有待实施的新工作 → false。\
already_done_reason 用一句话给出判定依据（指向目录或内容证据）。",
        );
        let prompt = stage_prompt(
            state,
            RoleStage::Understand,
            payload,
            r#"{"goal_summary":string,"success_criteria":[{"id":string,"criteria":string}],"risks":[{"id":string,"risk":string,"level":"low"|"medium"|"high"}],"ambiguities":[{"item":string,"clarification":string}],"questions":[{"id":string,"question":string,"reason":string,"options":string[]}],"mode":"fast"|"standard"|"high_risk"|"emergency","complexity":"C0"|"C1"|"C2"|"C3","lane":"full"|"user_review"|"micro","risk_surfaces":string[],"profile":[{"dimension":"D"|"B"|"I"|"U"|"S"|"M"|"O","status":"required"|"conditional"|"advisory"|"na","reason":string}],"already_done":boolean,"already_done_reason":string}"#,
            &context_block,
        );
        log_stage_execution(
            &state.conn.lock().unwrap(),
            requirement_id,
            Some(&task.id),
            "understand",
            runtime_spec,
            crate::roles::get_role(state, RoleStage::Understand).ok().and_then(|r| r.modified_at).as_deref(),
        );
        let mut simulated = false;
        let understanding_tokens: i64;
        let cancel_key = agent_runtime::child_key(requirement_id, &task.id);
        let emit_rid = requirement_id.to_string();
        let emit_tid = task.id.clone();
        let understanding = match agent_runtime::invoke_streaming(
            runtime_spec,
            &prompt,
            true,
            workdir.as_deref(),
            &cancel_key,
            move |event| agent_runtime::emit_stream(&emit_rid, &emit_tid, "understand", &event),
        )
        .await
        {
            Ok(outcome) => {
                understanding_tokens = outcome.tokens.unwrap_or_else(|| estimate_tokens(prompt.len(), 800));
                match crate::understanding::parse_and_normalize(&outcome.text, fallback_mode) {
                    Ok(u) => u,
                    Err(e) => {
                        simulated = true;
                        log_task(&state.conn.lock().unwrap(), requirement_id, &task.id, "understand", "warn",
                            &format!("runtime 输出无法解析（{}），已降级到本地模拟引擎", e.message));
                        crate::understanding::mock_understanding(&content, &context)
                    }
                }
            }
            Err(e) if agent_runtime::is_cancelled(&e) => return Err(e),
            Err(e) => {
                simulated = true;
                understanding_tokens = estimate_tokens(prompt.len(), 800);
                log_task(&state.conn.lock().unwrap(), requirement_id, &task.id, "understand", "warn",
                    &format!("runtime 调用失败（{}），已自动降级到本地模拟引擎", e.message));
                crate::understanding::mock_understanding(&content, &context)
            }
        };

        let questions = understanding.questions.clone();
        // 判级字段兜底：LLM 未产出时按模式推导，保证列不空缺
        let complexity = understanding.complexity.clone().or_else(|| match understanding.mode {
            RequirementMode::Fast => Some("C0".into()),
            RequirementMode::HighRisk => Some("C2".into()),
            _ => Some("C1".into()),
        });
        let lane = understanding.lane.clone().or_else(|| match understanding.mode {
            RequirementMode::Fast | RequirementMode::Emergency => Some("micro".into()),
            RequirementMode::HighRisk => Some("full".into()),
            RequirementMode::Standard => Some("user_review".into()),
        });
        let complexity_reason = understanding.complexity.as_ref().map(|_| {
            if simulated { "本地模拟引擎判级（确定性启发）".to_string() } else { "由理解阶段按 agent-up 判级规则评定".to_string() }
        });

        {
            let conn = state.conn.lock().unwrap();
            db::create_requirement_version(&conn, requirement_id, next_version, &understanding, &questions, source, user_input.as_deref())?;
            db::update_requirement(
                &conn,
                requirement_id,
                &db::RequirementUpdates {
                    status: Some(final_status),
                    current_step: Some("understand".into()),
                    current_version: Some(next_version),
                    mode: Some(understanding.mode),
                    goal_summary: Some(understanding.goal_summary.clone()),
                    success_criteria: Some(understanding.success_criteria.clone()),
                    risks: Some(understanding.risks.clone()),
                    ambiguities: Some(understanding.ambiguities.clone()),
                    complexity: complexity.clone(),
                    complexity_reason,
                    profile: understanding.profile.clone(),
                    risk_surfaces: understanding.risk_surfaces.clone(),
                    lane: lane.clone(),
                    ..Default::default()
                },
            )?;
            db::update_task(
                &conn,
                &task.id,
                &db::TaskUpdates {
                    status: Some(TaskStatus::Completed),
                    result: Some(serde_json::to_value(&understanding)?),
                    tokens: Some(understanding_tokens),
                    ..Default::default()
                },
            )?;
            log(&conn, requirement_id, "understand", &format!(
                "理解完成，生成理解版本 v{next_version}（{} 判级·{} 车道 · tokens {understanding_tokens}）",
                complexity.as_deref().unwrap_or("未定级"),
                lane.as_deref().unwrap_or("未声明"),
            ));

            // 自动归类：理解判定"已经做完/纯资料"的需求直接归档为已完成，不进实施流水线。
            // 仅作用于初始理解（来源 Initial）——用户主动重新理解/迭代视为在推进工作，不自动归档。
            if understanding.already_done == Some(true) && matches!(source, UnderstandingSource::Initial) {
                let reason = understanding
                    .already_done_reason
                    .clone()
                    .unwrap_or_else(|| "理解判定：该需求无待实施的新工作".to_string());
                db::update_requirement(
                    &conn,
                    requirement_id,
                    &db::RequirementUpdates { status: Some(RequirementStatus::Completed), current_step: None, ..Default::default() },
                )?;
                db::refresh_requirement_metrics(&conn, requirement_id)?;
                log(&conn, requirement_id, "understand", &format!("自动归类：{reason} → 已归档为已完成"));
            }
        }

        let requirement = {
            let conn = state.conn.lock().unwrap();
            db::get_requirement(&conn, requirement_id)?.expect("requirement exists")
        };
        Ok::<_, ApiError>(Some(UnderstandingOutcome { requirement, understanding, version: next_version, questions }))
    })()
    .await;

    match result {
        Ok(outcome) => Ok(outcome),
        Err(err) => {
            let message = err.message.clone();
            {
                let conn = state.conn.lock().unwrap();
                if agent_runtime::is_cancelled(&err) {
                    // 用户取消：任务置 failed（原因=用户取消），需求回到 awaiting_confirmation 可重试，
                    // 不做本地模式分析回退（取消不是模型失败）。
                    let _ = db::update_task(
                        &conn,
                        &task.id,
                        &db::TaskUpdates {
                            status: Some(TaskStatus::Failed),
                            error_message: Some(message.clone()),
                            ..Default::default()
                        },
                    );
                    let _ = db::update_requirement(
                        &conn,
                        requirement_id,
                        &db::RequirementUpdates {
                            status: Some(RequirementStatus::AwaitingConfirmation),
                            ..Default::default()
                        },
                    );
                    log_task(
                        &conn,
                        requirement_id,
                        &task.id,
                        "understand",
                        "warn",
                        "理解已被用户取消，可重新发起理解或重试",
                    );
                    return Err(err);
                }
                let analysis = task_engine::analyze_requirement_mode(
                    &db::get_requirement(&conn, requirement_id)
                        .ok()
                        .and_then(|r| r)
                        .and_then(|r| r.content)
                        .unwrap_or_default(),
                );
                let _ = db::update_task(
                    &conn,
                    &task.id,
                    &db::TaskUpdates {
                        status: Some(TaskStatus::Failed),
                        error_message: Some(message.clone()),
                        ..Default::default()
                    },
                );
                let _ = db::update_requirement(
                    &conn,
                    requirement_id,
                    &db::RequirementUpdates {
                        status: Some(failure_status),
                        mode: Some(analysis.mode),
                        ..Default::default()
                    },
                );
                log_task(
                    &conn,
                    requirement_id,
                    &task.id,
                    "understand",
                    "error",
                    &format!("需求理解失败：{message}，已回退到本地模式分析"),
                );
            }
            Err(err)
        }
    }
}

pub async fn start_initial_understanding(state: &AppState, requirement_id: String) {
    let _ = run_understanding(
        &state,
        &requirement_id,
        UnderstandingSource::Initial,
        RequirementStatus::Understanding,
        RequirementStatus::AwaitingConfirmation,
        RequirementStatus::AwaitingConfirmation,
        None,
        crate::understanding::UnderstandingContext::default(),
    )
    .await;
}

// ---------------------------------------------------------------- 方案/交付/验证产物（mock + LLM 双实现）

struct PlanOutcome {
    plan: serde_json::Value,
    decision: Option<CreateDecisionInput>,
}

/// 全量原文读取：pipeline 各阶段直接从详情查询拿到的 Requirement 上取 content。
fn requirement_content(requirement: &Requirement) -> &str {
    requirement.content.as_deref().unwrap_or_default()
}

fn current_understanding(requirement: &Requirement) -> UnderstandingResult {
    UnderstandingResult {
        goal_summary: requirement.goal_summary.clone().unwrap_or_default(),
        success_criteria: requirement.success_criteria.clone().unwrap_or_default(),
        risks: requirement.risks.clone().unwrap_or_default(),
        ambiguities: requirement.ambiguities.clone().unwrap_or_default(),
        questions: vec![],
        mode: requirement.mode,
        complexity: requirement.complexity.clone(),
        lane: requirement.lane.clone(),
        risk_surfaces: requirement.risk_surfaces.clone(),
        profile: requirement.profile.clone(),
        already_done: None,
        already_done_reason: None,
    }
}

async fn produce_plan(
    state: &AppState,
    requirement: &Requirement,
    understanding: &UnderstandingResult,
) -> ApiResult<(PlanOutcome, i64)> {
    let settings = db::get_orchestration_settings(&state.conn.lock().unwrap());
    let runtime_spec = resolve_stage_runtime(&settings, RoleStage::Plan);
    let runtime_ok = agent_runtime::resolve_binary(runtime_spec.kind).is_some();
    if !runtime_ok {
        return Err(ApiError::bad_request(
            "方案 runtime 不可用，不能生成模拟方案",
        ));
    }

    let (workdir, context_block) = project_context(state, &requirement.project_id, "plan");
    let payload = format!(
        "需求：{}\n理解：{}",
        requirement_content(&requirement),
        serde_json::to_string(understanding)?
    );
    let prompt = stage_prompt(
        state,
        RoleStage::Plan,
        payload,
        r#"{"approach":string,"steps":string[],"considerations":string[],"decision":null 或 {"question":string,"context":string,"options":[{"label":string,"value":string,"description":string}],"recommended":string}}"#,
        &context_block,
    );
    let cancel_key = agent_runtime::child_key(&requirement.id, "plan");
    let rid = requirement.id.clone();
    let outcome = agent_runtime::invoke_streaming(
        runtime_spec,
        &prompt,
        true,
        workdir.as_deref(),
        &cancel_key,
        move |event| agent_runtime::emit_stream(&rid, "", "plan", &event),
    )
    .await?;
    let raw = outcome.text;
    let start = raw.find('{').unwrap_or(0);
    let end = raw.rfind('}').map(|i| i + 1).unwrap_or(raw.len());
    let parsed: serde_json::Value = serde_json::from_str(&raw[start..end])?;
    let options: Vec<DecisionOption> = parsed
        .pointer("/decision/options")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .enumerate()
                .filter_map(|(i, o)| {
                    if o.is_string() {
                        Some(DecisionOption {
                            label: o.as_str().unwrap().to_string(),
                            value: format!("option-{}", i + 1),
                            description: None,
                            risk: None,
                        })
                    } else {
                        let label = o.get("label").and_then(|v| v.as_str())?;
                        Some(DecisionOption {
                            label: label.to_string(),
                            value: o
                                .get("value")
                                .and_then(|v| v.as_str())
                                .map(String::from)
                                .unwrap_or_else(|| format!("option-{}", i + 1)),
                            description: o
                                .get("description")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                            risk: o.get("risk").and_then(|v| v.as_str()).map(String::from),
                        })
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let decision = if options.len() >= 2 {
        parsed.get("decision").map(|d| CreateDecisionInput {
            requirement_id: requirement.id.clone(),
            question: d
                .get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("需要你的决定")
                .to_string(),
            context: d.get("context").and_then(|v| v.as_str()).map(String::from),
            options: options.clone(),
            recommended: Some(
                d.get("recommended")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| options[0].value.as_str())
                    .to_string(),
            ),
        })
    } else {
        None
    };
    Ok((
        PlanOutcome {
            plan: json!({
                "approach": parsed.get("approach").and_then(|v| v.as_str()).unwrap_or_default(),
                "steps": parsed.get("steps").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>()).unwrap_or_default(),
                "considerations": parsed.get("considerations").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>()).unwrap_or_default(),
            }),
            decision,
        },
        outcome
            .tokens
            .unwrap_or_else(|| estimate_tokens(prompt.len(), 1200)),
    ))
}

async fn produce_deliverable(
    state: &AppState,
    requirement: &Requirement,
    understanding: &UnderstandingResult,
    plan: &serde_json::Value,
) -> ApiResult<(serde_json::Value, i64)> {
    let settings = db::get_orchestration_settings(&state.conn.lock().unwrap());
    let runtime_spec = resolve_stage_runtime(&settings, RoleStage::Implement);
    let runtime_ok = agent_runtime::resolve_binary(runtime_spec.kind).is_some();
    if !runtime_ok {
        return Err(ApiError::bad_request(
            "实施 runtime 不可用，不能生成模拟完成结果",
        ));
    }
    let (workdir, context_block) = project_context(state, &requirement.project_id, "implement");
    let payload = format!(
        "需求：{}\n理解：{}\n方案：{plan}",
        requirement_content(&requirement),
        serde_json::to_string(understanding)?
    );
    let prompt = stage_prompt(
        state,
        RoleStage::Implement,
        payload,
        r#"{"summary":string,"changes":string[],"files":[{"path":string,"description":string}],"test_plan":string}"#,
        &context_block,
    );
    // 实施阶段工作目录 = 项目目录：runtime 真实读写项目文件（唯一非只读阶段）
    let cancel_key = agent_runtime::child_key(&requirement.id, "implement");
    let rid = requirement.id.clone();
    let outcome = agent_runtime::invoke_streaming(
        runtime_spec,
        &prompt,
        false,
        workdir.as_deref(),
        &cancel_key,
        move |event| agent_runtime::emit_stream(&rid, "", "implement", &event),
    )
    .await?;
    Ok((
        extract_json(&outcome.text)?,
        outcome
            .tokens
            .unwrap_or_else(|| estimate_tokens(prompt.len(), 2000)),
    ))
}

async fn produce_verify(
    state: &AppState,
    requirement: &Requirement,
    understanding: &UnderstandingResult,
    deliverable: &serde_json::Value,
) -> ApiResult<(serde_json::Value, i64)> {
    let settings = db::get_orchestration_settings(&state.conn.lock().unwrap());
    let runtime_spec = resolve_stage_runtime(&settings, RoleStage::Verify);
    let runtime_ok = agent_runtime::resolve_binary(runtime_spec.kind).is_some();
    if !runtime_ok {
        return Err(ApiError::bad_request(
            "验证 runtime 不可用，不能生成模拟通过结果",
        ));
    }
    let (workdir, context_block) = project_context(state, &requirement.project_id, "verify");
    let payload = format!(
        "需求：{}\n成功标准：{}\n实施结果：{deliverable}",
        requirement_content(&requirement),
        serde_json::to_string(&understanding.success_criteria)?
    );
    let prompt = stage_prompt(
        state,
        RoleStage::Verify,
        payload,
        r#"{"passed":boolean,"results":[{"criteria":string,"passed":boolean,"note":string}],"conclusion":string}"#,
        &context_block,
    );
    // 验证只读：物理沙箱禁止验证方改动代码（codex --sandbox read-only）
    let cancel_key = agent_runtime::child_key(&requirement.id, "verify");
    let rid = requirement.id.clone();
    let outcome = agent_runtime::invoke_streaming(
        runtime_spec,
        &prompt,
        true,
        workdir.as_deref(),
        &cancel_key,
        move |event| agent_runtime::emit_stream(&rid, "", "verify", &event),
    )
    .await?;
    Ok((
        extract_json(&outcome.text)?,
        outcome
            .tokens
            .unwrap_or_else(|| estimate_tokens(prompt.len(), 700)),
    ))
}

async fn produce_review(
    state: &AppState,
    requirement: &Requirement,
    deliverable: &serde_json::Value,
) -> ApiResult<(serde_json::Value, i64)> {
    let settings = db::get_orchestration_settings(&state.conn.lock().unwrap());
    let runtime_spec = resolve_stage_runtime(&settings, RoleStage::Review);
    let runtime_ok = agent_runtime::resolve_binary(runtime_spec.kind).is_some();
    if !runtime_ok {
        return Err(ApiError::bad_request(
            "审查 runtime 不可用，不能生成模拟通过结果",
        ));
    }
    let (workdir, context_block) = project_context(state, &requirement.project_id, "review");
    let payload = format!(
        "需求：{}\n实施结果：{deliverable}",
        requirement_content(&requirement)
    );
    let prompt = stage_prompt(
        state,
        RoleStage::Review,
        payload,
        r#"{"findings":string[],"conclusion":string}"#,
        &context_block,
    );
    // 审查与验证同为只读分析阶段
    let cancel_key = agent_runtime::child_key(&requirement.id, "review");
    let rid = requirement.id.clone();
    let outcome = agent_runtime::invoke_streaming(
        runtime_spec,
        &prompt,
        true,
        workdir.as_deref(),
        &cancel_key,
        move |event| agent_runtime::emit_stream(&rid, "", "review", &event),
    )
    .await?;
    Ok((
        extract_json(&outcome.text)?,
        outcome
            .tokens
            .unwrap_or_else(|| estimate_tokens(prompt.len(), 600)),
    ))
}

fn extract_json(raw: &str) -> ApiResult<serde_json::Value> {
    let start = raw
        .find('{')
        .ok_or_else(|| ApiError::internal("模型输出缺少 JSON"))?;
    let end = raw
        .rfind('}')
        .map(|i| i + 1)
        .ok_or_else(|| ApiError::internal("模型输出缺少 JSON"))?;
    Ok(serde_json::from_str(&raw[start..end])?)
}

// ---------------------------------------------------------------- 主流水线

pub async fn pipeline_inner(state: &AppState, requirement_id: &str) -> ApiResult<()> {
    loop {
        let requirement = {
            let conn = state.conn.lock().unwrap();
            match db::get_requirement(&conn, requirement_id)? {
                Some(r) => r,
                None => return Ok(()),
            }
        };

        // 1) 方案阶段
        if requirement.status == RequirementStatus::Planning {
            let plan_task = {
                let conn = state.conn.lock().unwrap();
                db::list_tasks_by_requirement(&conn, requirement_id)?
                    .into_iter()
                    .find(|t| t.step_type == StepType::Plan && t.status == TaskStatus::Running)
            };
            let Some(plan_task) = plan_task else {
                return Ok(());
            };
            let (outcome, plan_tokens) =
                match produce_plan(state, &requirement, &current_understanding(&requirement)).await
                {
                    Ok((o, tokens)) => (o, tokens),
                    Err(e) => {
                        let conn = state.conn.lock().unwrap();
                        db::update_task(
                            &conn,
                            &plan_task.id,
                            &db::TaskUpdates {
                                status: Some(TaskStatus::Failed),
                                error_message: Some(e.message.clone()),
                                ..Default::default()
                            },
                        )?;
                        db::update_requirement(
                            &conn,
                            requirement_id,
                            &db::RequirementUpdates {
                                status: Some(RequirementStatus::Failed),
                                current_step: Some("plan".into()),
                                ..Default::default()
                            },
                        )?;
                        return Ok(());
                    }
                };
            {
                let conn = state.conn.lock().unwrap();
                db::update_task(
                    &conn,
                    &plan_task.id,
                    &db::TaskUpdates {
                        status: Some(TaskStatus::Completed),
                        result: Some(outcome.plan.clone()),
                        tokens: Some(plan_tokens),
                        ..Default::default()
                    },
                )?;
                db::create_artifact(
                    &conn,
                    requirement_id,
                    Some(&plan_task.id),
                    ArtifactType::Document,
                    "plan",
                    "实施方案",
                    &render_plan_markdown(&outcome.plan),
                )?;
                log_task(
                    &conn,
                    requirement_id,
                    &plan_task.id,
                    "plan",
                    "info",
                    "方案制定完成，进入实施",
                );

                let implement_task = ensure_task(&conn, requirement_id, StepType::Implement)?;
                match outcome.decision {
                    Some(mut decision_input) => {
                        decision_input.requirement_id = requirement_id.to_string();
                        db::create_decision(&conn, &decision_input, Some(&implement_task.id))?;
                        db::update_task(
                            &conn,
                            &implement_task.id,
                            &db::TaskUpdates {
                                status: Some(TaskStatus::WaitingDecision),
                                ..Default::default()
                            },
                        )?;
                        db::update_requirement(
                            &conn,
                            requirement_id,
                            &db::RequirementUpdates {
                                status: Some(RequirementStatus::WaitingDecision),
                                current_step: Some("implement".into()),
                                ..Default::default()
                            },
                        )?;
                        log_task(
                            &conn,
                            requirement_id,
                            &implement_task.id,
                            "implement",
                            "info",
                            &format!("挂起决策点等待用户定夺：{}", decision_input.question),
                        );
                        return Ok(()); // 决策解决后由 resume_pipeline 继续
                    }
                    None => {
                        db::update_requirement(
                            &conn,
                            requirement_id,
                            &db::RequirementUpdates {
                                status: Some(RequirementStatus::Implementing),
                                current_step: Some("implement".into()),
                                ..Default::default()
                            },
                        )?;
                        db::update_task(
                            &conn,
                            &implement_task.id,
                            &db::TaskUpdates {
                                status: Some(TaskStatus::Running),
                                ..Default::default()
                            },
                        )?;
                    }
                }
            }
            continue;
        }

        // 2) 实施阶段
        if requirement.status == RequirementStatus::Implementing {
            let implement_task = {
                let conn = state.conn.lock().unwrap();
                ensure_running(&conn, requirement_id, StepType::Implement)?
            };
            let (plan, understanding) = {
                let conn = state.conn.lock().unwrap();
                let plan = db::list_tasks_by_requirement(&conn, requirement_id)?
                    .into_iter()
                    .find(|t| t.step_type == StepType::Plan && t.status == TaskStatus::Completed)
                    .and_then(|t| t.result)
                    .unwrap_or(json!({ "approach": "", "steps": [], "considerations": [] }));
                (plan, current_understanding(&requirement))
            };
            let (deliverable, implement_tokens) =
                match produce_deliverable(state, &requirement, &understanding, &plan).await {
                    Ok((d, tokens)) => (d, tokens),
                    Err(e) => {
                        let conn = state.conn.lock().unwrap();
                        db::update_task(
                            &conn,
                            &implement_task.id,
                            &db::TaskUpdates {
                                status: Some(TaskStatus::Failed),
                                error_message: Some(e.message.clone()),
                                ..Default::default()
                            },
                        )?;
                        db::update_requirement(
                            &conn,
                            requirement_id,
                            &db::RequirementUpdates {
                                status: Some(RequirementStatus::Failed),
                                current_step: Some("implement".into()),
                                ..Default::default()
                            },
                        )?;
                        return Ok(());
                    }
                };
            {
                let conn = state.conn.lock().unwrap();
                db::update_task(
                    &conn,
                    &implement_task.id,
                    &db::TaskUpdates {
                        status: Some(TaskStatus::Completed),
                        result: Some(deliverable.clone()),
                        tokens: Some(implement_tokens),
                        ..Default::default()
                    },
                )?;
                db::create_artifact(
                    &conn,
                    requirement_id,
                    Some(&implement_task.id),
                    ArtifactType::Code,
                    "implement",
                    "交付产物",
                    &render_deliverable_markdown(&deliverable),
                )?;
                log_task(
                    &conn,
                    requirement_id,
                    &implement_task.id,
                    "implement",
                    "info",
                    "实施完成，进入验证",
                );

                let verify_task = ensure_task(&conn, requirement_id, StepType::Verify)?;
                db::update_requirement(
                    &conn,
                    requirement_id,
                    &db::RequirementUpdates {
                        status: Some(RequirementStatus::Verifying),
                        current_step: Some("verify".into()),
                        ..Default::default()
                    },
                )?;
                db::update_task(
                    &conn,
                    &verify_task.id,
                    &db::TaskUpdates {
                        status: Some(TaskStatus::Running),
                        ..Default::default()
                    },
                )?;
            }
            continue;
        }

        // 3) 验证阶段
        if requirement.status == RequirementStatus::Verifying {
            let verify_task = {
                let conn = state.conn.lock().unwrap();
                ensure_running(&conn, requirement_id, StepType::Verify)?
            };
            let (deliverable, understanding) = {
                let conn = state.conn.lock().unwrap();
                let deliverable = db::list_tasks_by_requirement(&conn, requirement_id)?
                    .into_iter()
                    .filter(|t| {
                        t.step_type == StepType::Implement && t.status == TaskStatus::Completed
                    })
                    .last()
                    .and_then(|t| t.result)
                    .unwrap_or(
                        json!({ "summary": "", "changes": [], "files": [], "test_plan": "" }),
                    );
                (deliverable, current_understanding(&requirement))
            };
            let (verify, verify_tokens) =
                match produce_verify(state, &requirement, &understanding, &deliverable).await {
                    Ok((v, tokens)) => (v, tokens),
                    Err(e) => {
                        let conn = state.conn.lock().unwrap();
                        db::update_task(
                            &conn,
                            &verify_task.id,
                            &db::TaskUpdates {
                                status: Some(TaskStatus::Failed),
                                error_message: Some(e.message.clone()),
                                ..Default::default()
                            },
                        )?;
                        db::update_requirement(
                            &conn,
                            requirement_id,
                            &db::RequirementUpdates {
                                status: Some(RequirementStatus::Failed),
                                current_step: Some("verify".into()),
                                ..Default::default()
                            },
                        )?;
                        return Ok(());
                    }
                };
            let passed = verify
                .get("passed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let review_after = get_mode_steps(requirement.mode)
                .steps
                .contains(&StepType::Review);
            {
                let conn = state.conn.lock().unwrap();
                db::update_task(
                    &conn,
                    &verify_task.id,
                    &db::TaskUpdates {
                        status: Some(if passed {
                            TaskStatus::Completed
                        } else {
                            TaskStatus::Failed
                        }),
                        result: Some(verify.clone()),
                        tokens: Some(verify_tokens),
                        ..Default::default()
                    },
                )?;
                db::create_artifact(
                    &conn,
                    requirement_id,
                    Some(&verify_task.id),
                    ArtifactType::Markdown,
                    "verify",
                    "验证报告",
                    &render_verify_markdown(&verify),
                )?;
                if passed && review_after {
                    let review_task = ensure_task(&conn, requirement_id, StepType::Review)?;
                    db::update_requirement(
                        &conn,
                        requirement_id,
                        &db::RequirementUpdates {
                            status: Some(RequirementStatus::Verifying),
                            current_step: Some("review".into()),
                            ..Default::default()
                        },
                    )?;
                    db::update_task(
                        &conn,
                        &review_task.id,
                        &db::TaskUpdates {
                            status: Some(TaskStatus::Running),
                            ..Default::default()
                        },
                    )?;
                    log_task(
                        &conn,
                        requirement_id,
                        &verify_task.id,
                        "verify",
                        "info",
                        "验证通过，高风险模式进入审查",
                    );
                } else if passed {
                    db::update_requirement(
                        &conn,
                        requirement_id,
                        &db::RequirementUpdates {
                            status: Some(RequirementStatus::Completed),
                            current_step: None,
                            ..Default::default()
                        },
                    )?;
                    db::refresh_requirement_metrics(&conn, requirement_id)?;
                    log_task(
                        &conn,
                        requirement_id,
                        &verify_task.id,
                        "verify",
                        "info",
                        "验证通过，需求交付完成 ✓",
                    );
                } else {
                    let conclusion = verify
                        .get("conclusion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("验证未通过");
                    db::update_requirement(
                        &conn,
                        requirement_id,
                        &db::RequirementUpdates {
                            status: Some(RequirementStatus::Failed),
                            current_step: Some("verify".into()),
                            ..Default::default()
                        },
                    )?;
                    db::refresh_requirement_metrics(&conn, requirement_id)?;
                    log_task(
                        &conn,
                        requirement_id,
                        &verify_task.id,
                        "verify",
                        "error",
                        &format!("验证未通过：{conclusion}"),
                    );
                }
            }
            if passed && review_after {
                run_review(state, requirement_id).await?;
            }
            return Ok(());
        }

        return Ok(());
    }
}

async fn run_review(state: &AppState, requirement_id: &str) -> ApiResult<()> {
    let review_task = {
        let conn = state.conn.lock().unwrap();
        db::list_tasks_by_requirement(&conn, requirement_id)?
            .into_iter()
            .find(|t| t.step_type == StepType::Review && t.status == TaskStatus::Running)
    };
    let Some(review_task) = review_task else {
        return Ok(());
    };
    let (requirement, deliverable) = {
        let conn = state.conn.lock().unwrap();
        let requirement = db::get_requirement(&conn, requirement_id)?
            .ok_or_else(|| ApiError::not_found("需求不存在"))?;
        let deliverable = db::list_tasks_by_requirement(&conn, requirement_id)?
            .into_iter()
            .filter(|t| t.step_type == StepType::Implement && t.status == TaskStatus::Completed)
            .last()
            .and_then(|t| t.result)
            .unwrap_or(json!({}));
        (requirement, deliverable)
    };
    let (review, review_tokens) = match produce_review(state, &requirement, &deliverable).await {
        Ok((r, tokens)) => (r, tokens),
        Err(e) => {
            let conn = state.conn.lock().unwrap();
            db::update_task(
                &conn,
                &review_task.id,
                &db::TaskUpdates {
                    status: Some(TaskStatus::Failed),
                    error_message: Some(e.message.clone()),
                    ..Default::default()
                },
            )?;
            db::update_requirement(
                &conn,
                requirement_id,
                &db::RequirementUpdates {
                    status: Some(RequirementStatus::Failed),
                    current_step: Some("review".into()),
                    ..Default::default()
                },
            )?;
            return Ok(());
        }
    };
    if review
        .get("findings")
        .and_then(|v| v.as_array())
        .is_some_and(|f| !f.is_empty())
    {
        let conn = state.conn.lock().unwrap();
        db::update_task(
            &conn,
            &review_task.id,
            &db::TaskUpdates {
                status: Some(TaskStatus::Failed),
                result: Some(review.clone()),
                tokens: Some(review_tokens),
                ..Default::default()
            },
        )?;
        db::update_requirement(
            &conn,
            requirement_id,
            &db::RequirementUpdates {
                status: Some(RequirementStatus::Failed),
                current_step: Some("review".into()),
                ..Default::default()
            },
        )?;
        return Ok(());
    }
    let conn = state.conn.lock().unwrap();
    db::update_task(
        &conn,
        &review_task.id,
        &db::TaskUpdates {
            status: Some(TaskStatus::Completed),
            result: Some(review.clone()),
            tokens: Some(review_tokens),
            ..Default::default()
        },
    )?;
    db::create_artifact(
        &conn,
        requirement_id,
        Some(&review_task.id),
        ArtifactType::Markdown,
        "review",
        "审查报告",
        &render_review_markdown(&review),
    )?;
    db::update_requirement(
        &conn,
        requirement_id,
        &db::RequirementUpdates {
            status: Some(RequirementStatus::Completed),
            current_step: None,
            ..Default::default()
        },
    )?;
    db::refresh_requirement_metrics(&conn, requirement_id)?;
    log_task(
        &conn,
        requirement_id,
        &review_task.id,
        "review",
        "info",
        "审查通过，需求交付完成 ✓",
    );
    Ok(())
}

fn ensure_task(
    conn: &rusqlite::Connection,
    requirement_id: &str,
    step: StepType,
) -> ApiResult<Task> {
    if let Some(pending) = db::list_tasks_by_requirement(conn, requirement_id)?
        .into_iter()
        .find(|t| t.step_type == step && t.status == TaskStatus::Pending)
    {
        return Ok(pending);
    }
    db::create_task(
        conn,
        requirement_id,
        step,
        task_engine::step_label(step),
        TaskStatus::Pending,
    )
}

fn ensure_running(
    conn: &rusqlite::Connection,
    requirement_id: &str,
    step: StepType,
) -> ApiResult<Task> {
    let tasks = db::list_tasks_by_requirement(conn, requirement_id)?;
    if let Some(running) = tasks
        .iter()
        .find(|t| t.step_type == step && t.status == TaskStatus::Running)
    {
        return Ok(running.clone());
    }
    if let Some(pending) = tasks
        .iter()
        .find(|t| t.step_type == step && t.status == TaskStatus::Pending)
    {
        db::update_task(
            conn,
            &pending.id,
            &db::TaskUpdates {
                status: Some(TaskStatus::Running),
                ..Default::default()
            },
        )?;
        return Ok(db::get_task(conn, &pending.id)?.expect("task exists"));
    }
    db::create_task(
        conn,
        requirement_id,
        step,
        task_engine::step_label(step),
        TaskStatus::Running,
    )
}

// ---------------------------------------------------------------- 重试

/// 失败需求的重试路由：从最后一个失败任务所在阶段恢复。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryRoute {
    /// 重新执行流水线（方案/实施/验证/审查）
    Pipeline,
    /// 重新执行初始理解
    Understanding,
}

/// 同步部分：校验 + 复位状态 + 记日志，返回恢复路由。流水线由调用方 spawn。
pub fn retry_failed(
    state: &AppState,
    requirement_id: &str,
) -> ApiResult<(Requirement, RetryRoute)> {
    let conn = state.conn.lock().unwrap();
    let requirement = db::get_requirement(&conn, requirement_id)?
        .ok_or_else(|| ApiError::not_found("需求不存在"))?;
    if requirement.status != RequirementStatus::Failed {
        return Err(ApiError::conflict("仅失败状态的需求可以重试"));
    }

    // 最后一个失败任务决定恢复点（迭代后可能有多个同类型任务，取最近的）
    let failed_stage: Option<StepType> = db::list_tasks_by_requirement(&conn, requirement_id)?
        .into_iter()
        .filter(|t| t.status == TaskStatus::Failed)
        .last()
        .map(|t| t.step_type);

    let route = match failed_stage {
        Some(StepType::Plan) => {
            db::create_task(
                &conn,
                requirement_id,
                StepType::Plan,
                task_engine::step_label(StepType::Plan),
                TaskStatus::Running,
            )?;
            db::update_requirement(
                &conn,
                requirement_id,
                &db::RequirementUpdates {
                    status: Some(RequirementStatus::Planning),
                    current_step: Some("plan".into()),
                    ..Default::default()
                },
            )?;
            RetryRoute::Pipeline
        }
        Some(StepType::Implement) => {
            db::update_requirement(
                &conn,
                requirement_id,
                &db::RequirementUpdates {
                    status: Some(RequirementStatus::Implementing),
                    current_step: Some("implement".into()),
                    ..Default::default()
                },
            )?;
            RetryRoute::Pipeline
        }
        Some(StepType::Verify) | Some(StepType::Review) => {
            db::update_requirement(
                &conn,
                requirement_id,
                &db::RequirementUpdates {
                    status: Some(RequirementStatus::Verifying),
                    current_step: Some("verify".into()),
                    ..Default::default()
                },
            )?;
            RetryRoute::Pipeline
        }
        // 理解失败或无失败任务可判定：从头理解
        _ => {
            db::update_requirement(
                &conn,
                requirement_id,
                &db::RequirementUpdates {
                    status: Some(RequirementStatus::Understanding),
                    current_step: Some("understand".into()),
                    ..Default::default()
                },
            )?;
            RetryRoute::Understanding
        }
    };

    let stage_label = failed_stage
        .map(|s| task_engine::step_label(s))
        .unwrap_or("理解需求");
    db::create_log(
        &conn,
        requirement_id,
        None,
        "retry",
        "info",
        &format!("手动重试：从「{stage_label}」阶段恢复执行"),
        None,
    )?;
    let updated = db::get_requirement(&conn, requirement_id)?.expect("requirement exists");
    Ok((updated, route))
}

// ---------------------------------------------------------------- confirm / reunderstand / iterate

pub fn start_planning_sync(
    state: &AppState,
    requirement_id: &str,
) -> ApiResult<Option<(Task, Vec<Task>)>> {
    let conn = state.conn.lock().unwrap();
    let Some(requirement) = db::get_requirement(&conn, requirement_id)? else {
        return Ok(None);
    };
    db::update_requirement(
        &conn,
        requirement_id,
        &db::RequirementUpdates {
            status: Some(RequirementStatus::Planning),
            current_step: Some("plan".into()),
            confirmed_at: Some(now_iso()),
            ..Default::default()
        },
    )?;
    let plan_task = db::create_task(
        &conn,
        requirement_id,
        StepType::Plan,
        task_engine::step_label(StepType::Plan),
        TaskStatus::Running,
    )?;
    let pending_tasks: Vec<Task> = get_mode_steps(requirement.mode)
        .steps
        .into_iter()
        .filter(|s| matches!(s, StepType::Implement | StepType::Verify | StepType::Review))
        .map(|step| {
            db::create_task(
                &conn,
                requirement_id,
                step,
                task_engine::step_label(step),
                TaskStatus::Pending,
            )
            .expect("task insert")
        })
        .collect();
    log(&conn, requirement_id, "plan", "需求已确认，开始制定方案");
    Ok(Some((plan_task, pending_tasks)))
}

pub fn spawn_pipeline(app: tauri::AppHandle, requirement_id: String) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<std::sync::Arc<AppState>>();
        let state: &AppState = &state;
        if let Err(e) = pipeline_inner(state, &requirement_id).await {
            eprintln!("[pipeline:{requirement_id}] {e}");
        }
    });
}

/// 决策解决后恢复流水线：waiting_decision → implementing → 继续执行。
pub async fn resume_pipeline(state: &AppState, requirement_id: &str) {
    let wait_task = {
        let conn = state.conn.lock().unwrap();
        db::list_tasks_by_requirement(&conn, requirement_id)
            .ok()
            .and_then(|tasks| {
                tasks.into_iter().find(|t| {
                    t.step_type == StepType::Implement && t.status == TaskStatus::WaitingDecision
                })
            })
    };
    if let Some(task) = wait_task {
        let conn = state.conn.lock().unwrap();
        let _ = db::update_task(
            &conn,
            &task.id,
            &db::TaskUpdates {
                status: Some(TaskStatus::Running),
                ..Default::default()
            },
        );
        let _ = db::update_requirement(
            &conn,
            requirement_id,
            &db::RequirementUpdates {
                status: Some(RequirementStatus::Implementing),
                current_step: Some("implement".into()),
                ..Default::default()
            },
        );
        log(&conn, requirement_id, "implement", "决策已解决，继续实施");
    }
    if let Err(e) = pipeline_inner(state, requirement_id).await {
        eprintln!("[pipeline:{requirement_id}] {e}");
    }
}

pub async fn start_reunderstand(
    state: &AppState,
    requirement_id: &str,
    input: &ReunderstandInput,
) -> ApiResult<()> {
    let source = if input.manual.unwrap_or(false) {
        UnderstandingSource::Manual
    } else if input.answering.unwrap_or(false) {
        UnderstandingSource::QuestionAnswer
    } else {
        UnderstandingSource::UserFeedback
    };
    let processing = if input.answering.unwrap_or(false) {
        RequirementStatus::Questioning
    } else {
        RequirementStatus::Understanding
    };
    run_understanding(
        state,
        requirement_id,
        source,
        processing,
        RequirementStatus::AwaitingConfirmation,
        RequirementStatus::AwaitingConfirmation,
        input.feedback.clone(),
        crate::understanding::UnderstandingContext {
            feedback: input.feedback.clone(),
            answering_question: input.answering.unwrap_or(false),
            manual: input.manual.unwrap_or(false),
            quote: input.quote.clone(),
            attachments: Vec::new(), // 附件已在 command 层持久化后随 requirement.attachments 变更
        },
    )
    .await?;
    Ok(())
}

pub async fn start_iterate(
    state: &AppState,
    requirement_id: &str,
    input: &IterateInput,
    attachments: Vec<AttachmentItem>,
) -> ApiResult<()> {
    {
        let conn = state.conn.lock().unwrap();
        db::create_log(
            &conn,
            requirement_id,
            None,
            "iterate",
            "info",
            "收到用户反馈，开始迭代",
            Some(&json!({ "feedback": input.feedback })),
        )?;
    }
    let failure_status = {
        let conn = state.conn.lock().unwrap();
        db::get_requirement(&conn, requirement_id)?
            .map(|r| r.status)
            .unwrap_or(RequirementStatus::Failed)
    };
    let result = run_understanding(
        state,
        requirement_id,
        UnderstandingSource::Iteration,
        RequirementStatus::Implementing,
        RequirementStatus::Implementing,
        failure_status,
        Some(input.feedback.clone()),
        crate::understanding::UnderstandingContext {
            feedback: Some(input.feedback.clone()),
            answering_question: false,
            manual: false,
            quote: None,
            attachments,
        },
    )
    .await?;
    if let Some(outcome) = result {
        if outcome.requirement.status == RequirementStatus::Implementing {
            {
                let conn = state.conn.lock().unwrap();
                log(
                    &conn,
                    requirement_id,
                    "iterate",
                    &format!("迭代理解完成（v{}），重新进入实施", outcome.version),
                );
            }
            pipeline_inner(state, requirement_id).await?;
        }
    }
    Ok(())
}

pub fn has_in_flight_understanding(state: &AppState, requirement_id: &str) -> ApiResult<bool> {
    let conn = state.conn.lock().unwrap();
    db::has_running_task(
        &conn,
        requirement_id,
        &[StepType::Understand, StepType::Iterate],
    )
}

// ---------------------------------------------------------------- Markdown 渲染

fn md_section(title: &str, lines: Vec<String>) -> String {
    let mut out = format!("# {title}\n\n");
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn render_plan_markdown(plan: &serde_json::Value) -> String {
    let steps: Vec<String> = plan
        .get("steps")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .enumerate()
                .filter_map(|(i, s)| s.as_str().map(|s| format!("{}. {s}", i + 1)))
                .collect()
        })
        .unwrap_or_default();
    let considerations: Vec<String> = plan
        .get("considerations")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|c| c.as_str())
                .map(|c| format!("- {c}"))
                .collect()
        })
        .unwrap_or_default();
    let mut out = md_section(
        "实施方案",
        vec![
            "## 总体思路".into(),
            plan.get("approach")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            String::new(),
            "## 实施步骤".into(),
        ],
    );
    out.push_str(&steps.join("\n"));
    out.push_str("\n\n## 风险与考量\n");
    out.push_str(&considerations.join("\n"));
    out
}

fn render_deliverable_markdown(d: &serde_json::Value) -> String {
    let changes: Vec<String> = d
        .get("changes")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|c| c.as_str())
                .map(|c| format!("- {c}"))
                .collect()
        })
        .unwrap_or_default();
    let files: Vec<String> = d
        .get("files")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    let path = f.get("path").and_then(|v| v.as_str())?;
                    let desc = f.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    Some(format!("- `{path}` — {desc}"))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut out = md_section(
        "交付产物",
        vec![
            "## 摘要".into(),
            d.get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            String::new(),
            "## 变更清单".into(),
        ],
    );
    out.push_str(&changes.join("\n"));
    out.push_str("\n\n## 涉及文件\n");
    out.push_str(&files.join("\n"));
    out.push_str(&format!(
        "\n\n## 测试计划\n{}",
        d.get("test_plan")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
    ));
    out
}

fn render_verify_markdown(v: &serde_json::Value) -> String {
    let results: Vec<String> = v
        .get("results")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    let criteria = r.get("criteria").and_then(|v| v.as_str())?;
                    let passed = r.get("passed").and_then(|v| v.as_bool()).unwrap_or(false);
                    let note = r.get("note").and_then(|v| v.as_str()).unwrap_or("");
                    Some(format!(
                        "- [{}] {criteria} — {note}",
                        if passed { "x" } else { " " }
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut out = md_section("验证报告", vec!["## 逐条核对".into()]);
    out.push_str(&results.join("\n"));
    out.push_str(&format!(
        "\n\n## 结论\n{}",
        v.get("conclusion")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
    ));
    out
}

fn render_review_markdown(r: &serde_json::Value) -> String {
    let findings: Vec<String> = r
        .get("findings")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|f| f.as_str())
                .map(|f| format!("- {f}"))
                .collect()
        })
        .unwrap_or_default();
    let mut out = md_section("审查报告", vec!["## 审查发现".into()]);
    out.push_str(&findings.join("\n"));
    out.push_str(&format!(
        "\n\n## 结论\n{}",
        r.get("conclusion")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
    ));
    out
}
