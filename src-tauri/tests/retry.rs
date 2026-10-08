use agentup_harness_lib::db::{self, AppState};
use agentup_harness_lib::orchestrator::{self, RetryRoute};
use agentup_harness_lib::types::*;

/// 重试路由：按最后一个失败任务所在阶段恢复。
#[tokio::test]
async fn retry_routes_to_failed_stage() {
    let base = tempfile::tempdir().expect("tempdir").keep();
    let state: AppState = db::open_state(&base).expect("open state");

    let project = db::create_project(&state.conn.lock().unwrap(), "重试项目", None).unwrap();
    let requirement = db::create_requirement(
        &state.conn.lock().unwrap(),
        &project.id,
        "为重试测试创建的需求内容，长度足够通过校验",
        None,
        RequirementMode::Standard,
        None,
    )
    .unwrap();
    let req_id = requirement.id.clone();

    // 非失败状态不可重试
    let err = orchestrator::retry_failed(&state, &req_id).unwrap_err();
    assert_eq!(err.code, 409, "非失败状态应 409");
    println!("  ✓ 非失败状态重试 → 409");

    // 方案阶段失败 → Pipeline 恢复，状态回到 planning
    db::create_task(&state.conn.lock().unwrap(), &req_id, StepType::Plan, "制定方案", TaskStatus::Failed).unwrap();
    db::update_requirement(&state.conn.lock().unwrap(), &req_id, &db::RequirementUpdates { status: Some(RequirementStatus::Failed), ..Default::default() }).unwrap();
    let (updated, route) = orchestrator::retry_failed(&state, &req_id).unwrap();
    assert_eq!(route, RetryRoute::Pipeline);
    assert_eq!(updated.status, RequirementStatus::Planning);
    println!("  ✓ 方案失败 → 从 pipeline 恢复（planning）");

    // 理解阶段失败 → Understanding 重新理解
    let req2 = db::create_requirement(
        &state.conn.lock().unwrap(),
        &project.id,
        "第二条用于理解失败重试的需求内容",
        None,
        RequirementMode::Standard,
        None,
    )
    .unwrap();
    db::create_task(&state.conn.lock().unwrap(), &req2.id, StepType::Understand, "理解需求", TaskStatus::Failed).unwrap();
    db::update_requirement(&state.conn.lock().unwrap(), &req2.id, &db::RequirementUpdates { status: Some(RequirementStatus::Failed), ..Default::default() }).unwrap();
    let (updated, route) = orchestrator::retry_failed(&state, &req2.id).unwrap();
    assert_eq!(route, RetryRoute::Understanding);
    assert_eq!(updated.status, RequirementStatus::Understanding);
    println!("  ✓ 理解失败 → 重新理解");

    // 验证失败 → Pipeline（verifying）
    let req3 = db::create_requirement(
        &state.conn.lock().unwrap(),
        &project.id,
        "第三条用于验证失败重试的需求内容",
        None,
        RequirementMode::Standard,
        None,
    )
    .unwrap();
    db::create_task(&state.conn.lock().unwrap(), &req3.id, StepType::Verify, "验证", TaskStatus::Failed).unwrap();
    db::update_requirement(&state.conn.lock().unwrap(), &req3.id, &db::RequirementUpdates { status: Some(RequirementStatus::Failed), ..Default::default() }).unwrap();
    let (updated, route) = orchestrator::retry_failed(&state, &req3.id).unwrap();
    assert_eq!(route, RetryRoute::Pipeline);
    assert_eq!(updated.status, RequirementStatus::Verifying);
    println!("  ✓ 验证失败 → 从 verifying 恢复");
}
