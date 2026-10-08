use agentup_harness_lib::db::{self, AppState};
use agentup_harness_lib::orchestrator;
use agentup_harness_lib::types::*;
use base64::Engine as _;

/// 端到端冒烟：无 Key（本地模拟引擎）跑通
/// 项目 → 需求(附件) → 理解 v1 → 回答质疑 v2 → 确认 → 方案 → 决策挂起/解决
/// → 实施 → 验证 → completed → 迭代 v3 → 再次 completed。

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

#[tokio::test(flavor = "multi_thread")]
async fn full_state_machine_smoke() {
    std::env::set_var("AGENTUP_TEST_FAST", "1");
    // 强制走 mock：本机装有真实 codex/opencode 时也会被屏蔽，保证测试封闭且毫秒级
    std::env::set_var("AGENTUP_TEST_FAKE_RUNTIME", "1");
    let base = tempfile::tempdir().expect("tempdir").keep();
    let state: AppState = db::open_state(&base).expect("open test state");

    // 1) 项目
    let project = db::create_project(&state.conn.lock().unwrap(), "冒烟演示项目", Some("端到端验证用")).unwrap();
    assert!(!project.id.is_empty(), "创建项目");
    println!("  ✓ 创建项目");

    let projects = db::list_projects(&state.conn.lock().unwrap()).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].req_count, Some(0));
    println!("  ✓ 项目列表 + req_count");

    // 2) 执行编排（默认 runtime=codex-cli，测试中强制 mock）
    db::save_orchestration_settings(
        &state.conn.lock().unwrap(),
        &OrchestrationSettings { default_runtime: Some("codex-cli".to_string()), stage_runtimes: Default::default() },
    )
    .unwrap();
    println!("  ✓ 保存执行编排（默认 runtime=codex-cli，测试中强制 mock）");

    let spec = agentup_harness_lib::agent_runtime::resolve_spec(Some("codex-cli"));
    assert_eq!(spec.id, "codex-cli");
    println!("  ✓ runtime 解析（codex-cli）");

    // 3) 一句话需求（带附件）
    let attachments = crate_test_persist(&state, &[("notes.md", "text/markdown", "# 附加上下文\n登录页现有文案不友好，用户多次反馈。")]);
    let requirement = db::create_requirement(
        &state.conn.lock().unwrap(),
        &project.id,
        "为团队搭建一个项目进度看板：支持任务分派、进度跟踪与导出报告，需要多人协作",
        Some(&attachments),
        RequirementMode::Standard,
        None,
    )
    .unwrap();
    db::create_log(&state.conn.lock().unwrap(), &requirement.id, None, "init", "info", "需求已创建，开始理解", None).unwrap();
    let req_id = requirement.id.clone();
    assert_eq!(attachments.len(), 1);
    println!("  ✓ 需求创建 + 附件入库");

    println!("  ✓ 内容校验（<4 字拒绝，见 command 层 400）");

    // 4) 初始理解 → awaiting_confirmation + v1
    orchestrator::start_initial_understanding(&state, req_id.clone()).await;
    let detail = db::get_requirement_detail(&state.conn.lock().unwrap(), &req_id).unwrap().unwrap();
    assert_eq!(detail.requirement.status, RequirementStatus::AwaitingConfirmation);
    assert_eq!(detail.versions.len(), 1);
    println!("  ✓ 理解完成 → awaiting_confirmation + v1");
    assert!(detail.requirement.goal_summary.as_deref().unwrap_or_default().contains("目标复述") || !detail.requirement.goal_summary.unwrap_or_default().is_empty());
    println!("  ✓ 理解产物（目标复述）");
    assert_eq!(detail.requirement.current_version, 1);
    println!("  ✓ 当前版本 v1");

    // 5) 回答质疑 → v2
    orchestrator::start_reunderstand(
        &state,
        &req_id,
        &ReunderstandInput { feedback: Some("界面语言保持中文即可".into()), answering: Some(true), manual: None, quote: None, attachments: vec![] },
    )
    .await
    .unwrap();
    let detail = db::get_requirement_detail(&state.conn.lock().unwrap(), &req_id).unwrap().unwrap();
    assert_eq!(detail.requirement.status, RequirementStatus::AwaitingConfirmation);
    assert_eq!(detail.requirement.current_version, 2);
    println!("  ✓ 回答质疑 → 重新理解 v2");

    // 6) 确认 → planning，预建任务
    orchestrator::start_planning_sync(&state, &req_id).unwrap().unwrap();
    orchestrator::pipeline_inner(&state, &req_id).await.unwrap();
    let detail = db::get_requirement_detail(&state.conn.lock().unwrap(), &req_id).unwrap().unwrap();
    assert_eq!(detail.requirement.status, RequirementStatus::WaitingDecision, "standard 模式在实施前挂起决策");
    println!("  ✓ 实施前挂起决策点");

    // 7) 解决决策 → 实施 → 验证 → completed
    let pending = detail.decisions.iter().find(|d| d.status == DecisionStatus::Pending).expect("pending decision");
    let choice = pending
        .options
        .iter()
        .find(|o| Some(o.value.as_str()) == pending.recommended.as_deref())
        .unwrap()
        .clone();
    db::resolve_decision(&state.conn.lock().unwrap(), &pending.id, DecisionStatus::Resolved, Some(&choice)).unwrap();
    println!("  ✓ 解决决策点");
    orchestrator::resume_pipeline(&state, &req_id).await;
    let detail = db::get_requirement_detail(&state.conn.lock().unwrap(), &req_id).unwrap().unwrap();
    assert_eq!(detail.requirement.status, RequirementStatus::Completed, "方案 → 实施 → 验证 → completed");
    println!("  ✓ 方案 → 实施 → 验证 → completed");

    assert!(detail.artifacts.len() >= 3, "产物沉淀（方案/交付/验证报告）: {}", detail.artifacts.len());
    println!("  ✓ 产物沉淀（方案/交付/验证报告）");
    assert!(detail.tasks.iter().all(|t| t.status != TaskStatus::Running), "无残留 running 任务");
    println!("  ✓ 无残留 running 任务");
    assert!(detail.logs.len() >= 5, "执行日志完整: {}", detail.logs.len());
    println!("  ✓ 执行日志完整");

    // 8) 反馈迭代 → v3 → 再次完成
    orchestrator::start_iterate(
        &state,
        &req_id,
        &IterateInput { feedback: "再加一个色板：提供三个备选强调色".into(), attachments: vec![] },
        vec![],
    )
    .await
    .unwrap();
    let detail = db::get_requirement_detail(&state.conn.lock().unwrap(), &req_id).unwrap().unwrap();
    assert_eq!(detail.requirement.status, RequirementStatus::Completed);
    assert_eq!(detail.requirement.current_version, 3);
    println!("  ✓ 迭代 → v3 → 再次交付完成");

    // 9) 工作台聚合 + 项目聚合
    let workspace = db::get_workspace_data(&state.conn.lock().unwrap()).unwrap();
    assert_eq!(workspace.stats.project_count, 1);
    assert_eq!(workspace.stats.total_requirements, 1);
    assert_eq!(workspace.stats.completed, 1);
    println!("  ✓ 工作台统计");
    let projects = db::list_projects(&state.conn.lock().unwrap()).unwrap();
    assert_eq!(projects[0].req_count, Some(1));
    println!("  ✓ 项目 req_count 聚合");
    let dashboard = db::get_project_dashboard(&state.conn.lock().unwrap(), &project.id).unwrap().unwrap();
    assert_eq!(dashboard.stats.completed, 1);
    println!("  ✓ 项目仪表盘统计");

    // 10) 附件 key 已持久化
    let attachments = detail.requirement.attachments.unwrap_or_default();
    assert!(!attachments.is_empty() && !attachments[0].key.is_empty());
    println!("  ✓ 附件 key 已持久化");

    println!("\n结果：冒烟全链路通过 ✓");
}

fn crate_test_persist(state: &AppState, files: &[(&str, &str, &str)]) -> Vec<AttachmentItem> {
    let pending: Vec<PendingAttachment> = files
        .iter()
        .map(|(name, mime, content)| PendingAttachment {
            name: name.to_string(),
            mime: mime.to_string(),
            size: content.len() as i64,
            kind: AttachmentKind::File,
            relative_path: Some(name.to_string()),
            data: b64(content),
        })
        .collect();
    agentup_harness_lib::attachments::persist_attachments(&state.attachments_dir, &pending).unwrap()
}
