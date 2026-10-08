use agentup_harness_lib::agent_runtime;

/// 真实链路冒烟（#[ignore]：会真实调 ZCode，消耗 token，约 30-90s）。
/// 运行：AGENTUP_TEST_FAST=1 cargo test --test zcode_runtime real_zcode_turn -- --ignored
/// 前置：/Applications/ZCode.app 已安装且桌面端已登录。
#[tokio::test(flavor = "multi_thread")]
#[ignore = "真实调用 ZCode 桌面 App（消耗 token），手动执行"]
async fn real_zcode_turn() {
    let spec = agent_runtime::spec_by_id("zcode-app").expect("zcode-app in registry");
    assert!(agent_runtime::resolve_binary(spec.kind).is_some(), "内嵌 CLI 应可解析");

    let (reply, latency_ms) = agent_runtime::check_connectivity(spec).await.expect("真实 turn 应成功");
    println!("回复：{reply}");
    println!("耗时：{latency_ms}ms");
    assert!(!reply.trim().is_empty(), "回复不能为空");
}
