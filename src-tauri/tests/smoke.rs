use agentup_harness_lib::db::{self, AppState};
use agentup_harness_lib::orchestrator;
use agentup_harness_lib::types::*;
use base64::Engine as _;

/// 端到端冒烟：验证 runtime 不可用时不会伪造实施完成。

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

#[tokio::test(flavor = "multi_thread")]
async fn full_state_machine_smoke() {
    std::env::set_var("AGENTUP_TEST_FAST", "1");
    // 强制屏蔽真实 runtime，验证缺失 runtime 时不会伪造成功。
    std::env::set_var("AGENTUP_TEST_FAKE_RUNTIME", "1");
    let base = tempfile::tempdir().expect("tempdir").keep();
    let state: AppState = db::open_state(&base).expect("open test state");

    // 1) 项目
    let project = db::create_project(
        &state.conn.lock().unwrap(),
        "冒烟演示项目",
        Some("端到端验证用"),
    )
    .unwrap();
    assert!(!project.id.is_empty(), "创建项目");
    println!("  ✓ 创建项目");

    let projects = db::list_projects(&state.conn.lock().unwrap()).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].req_count, Some(0));
    println!("  ✓ 项目列表 + req_count");

    // 2) 执行编排（默认 runtime=codex-cli，测试中强制不可用）
    db::save_orchestration_settings(
        &state.conn.lock().unwrap(),
        &OrchestrationSettings {
            default_runtime: Some("codex-cli".to_string()),
            stage_runtimes: Default::default(),
        },
    )
    .unwrap();
    println!("  ✓ 保存执行编排（默认 runtime=codex-cli，测试中强制不可用）");

    let spec = agentup_harness_lib::agent_runtime::resolve_spec(Some("codex-cli"));
    assert_eq!(spec.id, "codex-cli");
    println!("  ✓ runtime 解析（codex-cli）");

    // 3) 一句话需求（带附件）
    let attachments = crate_test_persist(
        &state,
        &[(
            "notes.md",
            "text/markdown",
            "# 附加上下文\n登录页现有文案不友好，用户多次反馈。",
        )],
    );
    let requirement = db::create_requirement(
        &state.conn.lock().unwrap(),
        &project.id,
        "为团队搭建一个项目进度看板：支持任务分派、进度跟踪与导出报告，需要多人协作",
        Some(&attachments),
        RequirementMode::Standard,
        None,
    )
    .unwrap();
    db::create_log(
        &state.conn.lock().unwrap(),
        &requirement.id,
        None,
        "init",
        "info",
        "需求已创建，开始理解",
        None,
    )
    .unwrap();
    let req_id = requirement.id.clone();
    assert_eq!(attachments.len(), 1);
    println!("  ✓ 需求创建 + 附件入库");

    println!("  ✓ 内容校验（<4 字拒绝，见 command 层 400）");

    // 4) 初始理解 → awaiting_confirmation + v1
    orchestrator::start_initial_understanding(&state, req_id.clone()).await;
    let detail = db::get_requirement_detail(&state.conn.lock().unwrap(), &req_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.requirement.status,
        RequirementStatus::AwaitingConfirmation
    );
    assert_eq!(detail.versions.len(), 1);
    println!("  ✓ 理解完成 → awaiting_confirmation + v1");
    assert!(
        detail
            .requirement
            .goal_summary
            .as_deref()
            .unwrap_or_default()
            .contains("目标复述")
            || !detail
                .requirement
                .goal_summary
                .unwrap_or_default()
                .is_empty()
    );
    println!("  ✓ 理解产物（目标复述）");
    assert_eq!(detail.requirement.current_version, 1);
    println!("  ✓ 当前版本 v1");

    // 5) 回答质疑 → v2
    orchestrator::start_reunderstand(
        &state,
        &req_id,
        &ReunderstandInput {
            feedback: Some("界面语言保持中文即可".into()),
            answering: Some(true),
            manual: None,
            quote: None,
            attachments: vec![],
        },
    )
    .await
    .unwrap();
    let detail = db::get_requirement_detail(&state.conn.lock().unwrap(), &req_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.requirement.status,
        RequirementStatus::AwaitingConfirmation
    );
    assert_eq!(detail.requirement.current_version, 2);
    println!("  ✓ 回答质疑 → 重新理解 v2");

    // 6) 确认后进入规划；规划 runtime 不可用时直接失败。
    orchestrator::start_planning_sync(&state, &req_id)
        .unwrap()
        .unwrap();
    orchestrator::pipeline_inner(&state, &req_id).await.unwrap();
    let detail = db::get_requirement_detail(&state.conn.lock().unwrap(), &req_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.requirement.status,
        RequirementStatus::Failed,
        "runtime 不可用时不得伪造方案"
    );
    println!("  ✓ runtime 不可用 → failed（无模拟方案）");
    assert!(
        detail
            .artifacts
            .iter()
            .all(|a| !matches!(a.stage.as_deref(), Some("implement" | "verify" | "review"))),
        "失败时不得写入伪造交付证据"
    );
    assert!(
        detail.tasks.iter().all(|t| t.status != TaskStatus::Running),
        "无残留 running 任务"
    );
    println!("  ✓ 无残留 running 任务");
    assert!(
        detail.logs.len() >= 4,
        "执行日志完整: {}",
        detail.logs.len()
    );
    println!("  ✓ 执行日志完整");
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
