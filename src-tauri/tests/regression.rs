use agentup_harness_lib::agent_runtime;
use agentup_harness_lib::db::{self, AppState};
use agentup_harness_lib::error::ApiResult;
use agentup_harness_lib::orchestrator;
use agentup_harness_lib::roles::{self, RoleStage};
use agentup_harness_lib::types::*;

/// 回归：has_running_task 的 SQL 占位符曾用 ?0 起始导致
/// "variable number must be between ?1 and ?32766"。
/// 直接调用该函数 + 走一遍 reunderstand 幂等路径。

#[tokio::test(flavor = "multi_thread")]
async fn has_running_task_sql_regression() {
    let base = tempfile::tempdir().expect("tempdir").keep();
    let state: AppState = db::open_state(&base).expect("open state");

    let project = db::create_project(&state.conn.lock().unwrap(), "回归项目", None).unwrap();
    let requirement = db::create_requirement(
        &state.conn.lock().unwrap(),
        &project.id,
        "为回归测试创建的需求内容，长度足够通过校验",
        None,
        RequirementMode::Standard,
        None,
    )
    .unwrap();
    let req_id = requirement.id.clone();

    // 无 running 任务 → false（此前这里直接抛 SQLite 错误）
    let none_running: ApiResult<bool> = db::has_running_task(
        &state.conn.lock().unwrap(),
        &req_id,
        &[StepType::Understand, StepType::Iterate],
    );
    assert!(!none_running.expect("has_running_task must not error"), "无 running 任务应为 false");
    println!("  ✓ has_running_task SQL 正常（空库返回 false）");

    // 造一个 running 任务 → true
    db::create_task(&state.conn.lock().unwrap(), &req_id, StepType::Understand, "理解需求", TaskStatus::Running).unwrap();
    let one_running = db::has_running_task(
        &state.conn.lock().unwrap(),
        &req_id,
        &[StepType::Understand, StepType::Iterate],
    )
    .expect("has_running_task must not error");
    assert!(one_running, "有 running 任务应为 true");
    println!("  ✓ has_running_task 检出 running 任务");

    // 幂等语义：running 期间 has_in_flight_understanding = true
    assert!(orchestrator::has_in_flight_understanding(&state, &req_id).unwrap());
    println!("  ✓ 幂等检查可识别进行中的理解任务");
}

/// 回归：角色 agentmd 的播种/读写/恢复默认，以及按阶段运行时解析规则。
#[tokio::test(flavor = "multi_thread")]
async fn roles_and_stage_runtime_regression() {
    let base = tempfile::tempdir().expect("tempdir").keep();
    let state: AppState = db::open_state(&base).expect("open state");

    // 1) 播种：5 个角色文件全部落盘，重复播种不覆盖
    roles::seed_roles(&state).unwrap();
    roles::seed_roles(&state).unwrap();
    let list = roles::list_roles(&state).unwrap();
    assert_eq!(list.len(), 5, "播种 5 个角色: {}", list.len());
    println!("  ✓ 播种 5 个角色文件（幂等）");

    // 2) 编辑 → 内容落盘；mtime 出现（“角色已自定义”标记依据）
    let verify_default = roles::get_role(&state, RoleStage::Verify).unwrap();
    let edited = roles::save_role(&state, RoleStage::Verify, "自定义验证角色：只认证据。").unwrap();
    assert!(edited.content.contains("只认证据"));
    assert!(edited.modified_at.is_some(), "保存后应有 mtime");
    assert_ne!(edited.content, verify_default.content);
    println!("  ✓ 角色编辑落盘 + mtime");

    // 3) 恢复默认：删文件重播种，与种子一致
    let reset = roles::reset_role(&state, RoleStage::Verify).unwrap();
    assert_eq!(reset.content, verify_default.content, "恢复默认后与种子一致");
    println!("  ✓ 恢复默认 = 删文件重播种");

    // 4) 按阶段运行时解析：无配置 → 注册表默认
    let settings = OrchestrationSettings::default();
    assert_eq!(
        mirror_resolve(&settings, RoleStage::Understand).id,
        "codex-cli",
        "无配置时落注册表默认"
    );
    println!("  ✓ 无配置 → 注册表默认（codex-cli）");

    // 5) 阶段配置优先于兜底；review 跟随 verify
    let settings = OrchestrationSettings {
        default_runtime: Some("opencode".to_string()),
        stage_runtimes: [("verify".to_string(), "codex-cli".to_string())].into_iter().collect(),
    };
    assert_eq!(mirror_resolve(&settings, RoleStage::Verify).id, "codex-cli", "verify 显式配置优先");
    assert_eq!(mirror_resolve(&settings, RoleStage::Implement).id, "opencode", "implement 落 default_runtime");
    assert_eq!(mirror_resolve(&settings, RoleStage::Review).id, "codex-cli", "review 跟随 verify 配置");
    println!("  ✓ review 跟随 verify；阶段配置优先于兜底");
}

/// orchestrator::resolve_stage_runtime 未 pub；这里按相同规则镜像断言（防规则漂移）。
fn mirror_resolve(settings: &OrchestrationSettings, stage: RoleStage) -> &'static agent_runtime::RuntimeSpec {
    let configured = |key: &str| settings.stage_runtimes.get(key).and_then(|id| agent_runtime::spec_by_id(id));
    let default = || {
        settings
            .default_runtime
            .as_deref()
            .and_then(agent_runtime::spec_by_id)
            .unwrap_or_else(|| agent_runtime::resolve_spec(None))
    };
    match stage {
        RoleStage::Review => configured("review").or_else(|| configured("verify")).unwrap_or_else(default),
        stage => configured(stage.key()).unwrap_or_else(default),
    }
}
