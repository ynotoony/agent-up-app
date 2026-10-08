use agentup_harness_lib::agent_runtime::{
    extract_last_text_zcode, extract_tokens_zcode, RuntimeKind, DEFAULT_RUNTIME_ID,
};

// fixture 取自 2026-10-01 真实 zcode 0.16.9 stream-json 校准输出（节选关键字段）。

const TURN_COMPLETED: &str = r#"{"eventId":"4d3122e8","payload":{"response":"正常","tokenCount":22240,"usage":{"source":"provider","inputTokens":22213,"outputTokens":27,"totalTokens":22240,"reasoningTokens":20},"duration":31780,"resultType":"success"},"seq":14,"sessionId":"sess_x","type":"turn.completed"}"#;

const RESULT_LINE: &str = r#"{"type":"result","sessionId":"sess_x","response":"正常","usage":{"source":"provider","modelRequestCount":1,"inputTokens":22213,"outputTokens":27,"totalTokens":22240},"eventCount":13,"projection":{"status":"idle","totalTokenCount":22240}}"#;

const STREAMING_DELTA: &str = r#"{"eventId":"099bb0ec","payload":{"assistantMessageId":"msg_x","delta":"正常","done":false,"kind":"text_delta"},"seq":9,"type":"model.streaming"}"#;

#[test]
fn zcode_extracts_response_from_turn_completed() {
    assert_eq!(extract_last_text_zcode(TURN_COMPLETED).as_deref(), Some("正常"));
    assert_eq!(extract_tokens_zcode(TURN_COMPLETED), Some(22213 + 27));
}

#[test]
fn zcode_extracts_response_from_result_line() {
    // 末行 result 与 turn.completed 同文本时互相印证；token 不重复计（result 行取 usage 覆盖语义）
    assert_eq!(extract_last_text_zcode(RESULT_LINE).as_deref(), Some("正常"));
    assert_eq!(extract_tokens_zcode(RESULT_LINE), Some(22213 + 27));
}

#[test]
fn zcode_ignores_streaming_and_noise() {
    // 增量事件不作为最终文本（避免拿到半截 delta）；非 JSON 行忽略
    assert_eq!(extract_last_text_zcode(STREAMING_DELTA), None);
    assert_eq!(extract_tokens_zcode(STREAMING_DELTA), None);
    assert_eq!(extract_last_text_zcode("not json"), None);
    assert_eq!(extract_last_text_zcode(""), None);
}

#[test]
fn zcode_result_overrides_turn_completed_as_last_text() {
    // 逐行扫描语义：后出现的 result 行覆盖 turn.completed（双保险，两者不一致时以 result 为准）
    let mut last = String::new();
    for line in [TURN_COMPLETED, RESULT_LINE] {
        if let Some(t) = extract_last_text_zcode(line) {
            last = t;
        }
    }
    assert_eq!(last, "正常");
}

// 注册表：第三条 zcode-app 就位，id 唯一，缺省非只读档。
#[test]
fn registry_contains_zcode_app() {
    let ids: Vec<&str> = agentup_harness_lib::agent_runtime::REGISTRY.iter().map(|s| s.id).collect();
    assert_eq!(ids, ["codex-cli", "opencode", "zcode-app"]);
    assert_eq!(agentup_harness_lib::agent_runtime::spec_by_id("zcode-app").unwrap().kind, RuntimeKind::Zcode);
    assert!(!agentup_harness_lib::agent_runtime::spec_by_id("zcode-app").unwrap().read_only_analysis);
    assert_eq!(DEFAULT_RUNTIME_ID, "codex-cli", "默认运行时不变");
}
