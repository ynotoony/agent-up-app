use agentup_harness_lib::agent_runtime::{detect_model_for_test, RuntimeKind};

// 三个场景必须串行执行：HOME 是进程级环境变量，并行测试会互相覆盖。
#[test]
fn model_detection_scenarios() {
    // ① codex config.toml：model_provider 等同前缀键不得干扰；顶层 model 正确读取且不落入 profile 段
    let dir = tempfile::tempdir().unwrap();
    let codex_dir = dir.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join("config.toml"),
        "model_provider = \"custom\"\nmodel = \"deepseek-flash\"\n\n[profiles.fast]\nmodel = \"fast-model\"\n",
    )
    .unwrap();
    unsafe { std::env::set_var("HOME", dir.path()) };
    assert_eq!(
        detect_model_for_test(RuntimeKind::Codex).as_deref(),
        Some("deepseek-flash"),
        "顶层 model 必须命中，且不落入 profile 段"
    );

    // ② opencode：多候选路径 + jsonc 行注释容错
    let dir = tempfile::tempdir().unwrap();
    let oc_dir = dir.path().join(".config/opencode");
    std::fs::create_dir_all(&oc_dir).unwrap();
    std::fs::write(
        oc_dir.join("opencode.jsonc"),
        "{\n  // 注释\n  \"model\": \"claude-sonnet-4\",\n}\n",
    )
    .unwrap();
    unsafe { std::env::set_var("HOME", dir.path()) };
    assert_eq!(detect_model_for_test(RuntimeKind::Opencode).as_deref(), Some("claude-sonnet-4"));

    // ③ 无配置文件 → None（不报错）
    let dir = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("HOME", dir.path()) };
    assert_eq!(detect_model_for_test(RuntimeKind::Codex), None);
    assert_eq!(detect_model_for_test(RuntimeKind::Opencode), None);
}
