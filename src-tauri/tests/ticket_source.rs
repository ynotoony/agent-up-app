// 票 #1：治理票只读源测试。索引→卡映射、泳道映射、防御性拒收计数、票体懒加载、fail-visible。
use agentup_harness_lib::db;
use agentup_harness_lib::ticket_source;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn temp_base(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agentup-tickets-{}-{}-{}",
        tag,
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("create temp base");
    dir
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, content).expect("write file");
}

/// 建库＋建一个绑定 project_dir 的项目，返回 (state, project_id)。
fn state_with_project(base: &Path, project_dir: &Path) -> (Arc<db::AppState>, String) {
    let state = Arc::new(db::open_state(base).expect("open state"));
    let conn = state.conn.lock().unwrap();
    let project =
        db::create_project_full(&conn, "demo", None, Some(project_dir.to_str().unwrap())).expect("create project");
    drop(conn);
    (state, project.id)
}

const SAMPLE_INDEX: &str = r#"{
  "issues": [
    {"id": "01-alpha", "status": "done", "title": "第一张", "updated_at": "2026-10-01T10:00"},
    {"id": "02-beta", "status": "in_progress", "title": "第二张", "blocked_by": ["01-alpha"], "updated_at": "2026-10-02T11:00"},
    {"id": "03-gamma", "status": "surfing", "title": "未知状态票"},
    {"id": "", "status": "done"},
    {"id": "../escape", "status": "done"},
    {"status": "done"}
  ]
}"#;

// ① 索引→卡映射：泳道、排序、blocked_by 透传、未知状态 fail-visible、坏条目计数不静默。
#[test]
fn index_maps_to_cards_with_lanes() {
    let base = temp_base("map");
    let project_dir = temp_base("proj-map");
    write_file(&project_dir.join("docs/issues/index.json"), SAMPLE_INDEX);
    let (state, pid) = state_with_project(&base, &project_dir);
    let conn = state.conn.lock().unwrap();
    let out = ticket_source::load_governance_tickets(&conn, &pid, false).expect("load");

    // 6 条索引行：3 有效 + 3 拒收（空 id / 带路径符号 / 缺 id）——对账必须能对上
    assert_eq!(out["total"].as_u64(), Some(3), "有效卡数");
    assert_eq!(out["skipped"].as_u64(), Some(3), "拒收条目必须计数");
    let tickets = out["tickets"].as_array().expect("tickets array");
    assert_eq!(tickets.len(), 3);

    // 按 id 升序
    assert_eq!(tickets[0]["id"].as_str(), Some("01-alpha"));
    assert_eq!(tickets[1]["id"].as_str(), Some("02-beta"));
    assert_eq!(tickets[2]["id"].as_str(), Some("03-gamma"));

    // 泳道映射
    assert_eq!(tickets[0]["lane"].as_str(), Some("done"));
    assert_eq!(tickets[1]["lane"].as_str(), Some("in_progress"));
    assert_eq!(tickets[2]["lane"].as_str(), Some("unknown"));

    // blocked_by 透传；title 回退与透传
    assert_eq!(tickets[1]["blocked_by"], serde_json::json!(["01-alpha"]));
    assert_eq!(tickets[0]["title"].as_str(), Some("第一张"));

    // 未知状态进 unknown_statuses（fail visible，不静默丢）
    assert_eq!(out["unknown_statuses"]["03-gamma"].as_str(), Some("surfing"));

    // load_bodies=false 时无 body 键
    assert!(tickets[0].get("body").is_none(), "首屏路径不读票体");
}

// ② 票体懒加载：load_bodies=true 读 <id>.json 摘字段；缺失票体不报错、无 body 键。
#[test]
fn bodies_load_when_requested() {
    let base = temp_base("bodies");
    let project_dir = temp_base("proj-bodies");
    write_file(&project_dir.join("docs/issues/index.json"), SAMPLE_INDEX);
    write_file(
        &project_dir.join("docs/issues/01-alpha.json"),
        r#"{"goal": "做个东西", "scope": ["a"], "acceptance": ["x"], "estimated_time": "0.5天"}"#,
    );
    let (state, pid) = state_with_project(&base, &project_dir);
    let conn = state.conn.lock().unwrap();

    let out = ticket_source::load_governance_tickets(&conn, &pid, true).expect("load with bodies");
    let tickets = out["tickets"].as_array().expect("tickets array");
    assert_eq!(tickets[0]["body"]["goal"].as_str(), Some("做个东西"));
    assert_eq!(tickets[0]["body"]["estimated_time"].as_str(), Some("0.5天"));
    // 02-beta 无票体文件：静默无 body，不报错
    assert!(tickets[1].get("body").is_none(), "缺失票体不应报错");
}

// ③ fail-visible：索引缺失 → 404；索引坏 JSON / 缺 issues 数组 → 400。
#[test]
fn broken_sources_fail_visible() {
    let base = temp_base("fail");
    let missing = temp_base("proj-no-index");
    let (state, pid) = state_with_project(&base, &missing);
    let conn = state.conn.lock().unwrap();
    let err = ticket_source::load_governance_tickets(&conn, &pid, false).unwrap_err();
    assert_eq!(err.code, 404, "无索引必须让用户看见，不是空看板");

    let bad_json = temp_base("proj-bad-json");
    write_file(&bad_json.join("docs/issues/index.json"), "{not json");
    let (state2, pid2) = state_with_project(&base, &bad_json);
    let conn2 = state2.conn.lock().unwrap();
    let err = ticket_source::load_governance_tickets(&conn2, &pid2, false).unwrap_err();
    assert_eq!(err.code, 400);

    let no_array = temp_base("proj-no-array");
    write_file(&no_array.join("docs/issues/index.json"), r#"{"other": []}"#);
    let (state3, pid3) = state_with_project(&base, &no_array);
    let conn3 = state3.conn.lock().unwrap();
    let err = ticket_source::load_governance_tickets(&conn3, &pid3, false).unwrap_err();
    assert_eq!(err.code, 400);
}

// ④ 未绑定目录的项目 → 400（不是 500 也不是静默空表）。
#[test]
fn unbound_project_rejected() {
    let base = temp_base("unbound");
    let state = Arc::new(db::open_state(&base).expect("open state"));
    let conn = state.conn.lock().unwrap();
    let project = db::create_project_full(&conn, "no-path", None, None).expect("create");
    let err = ticket_source::load_governance_tickets(&conn, &project.id, false).unwrap_err();
    assert_eq!(err.code, 400);
    assert!(err.message.contains("未绑定目录"), "错误信息要指明原因");
}

// ⑤ 目录已消失 → 404 fail-visible。
#[test]
fn missing_directory_fails_visible() {
    let base = temp_base("gone");
    let project_dir = temp_base("proj-gone");
    write_file(&project_dir.join("docs/issues/index.json"), SAMPLE_INDEX);
    let (state, pid) = state_with_project(&base, &project_dir);
    std::fs::remove_dir_all(&project_dir).expect("remove dir");
    let conn = state.conn.lock().unwrap();
    let err = ticket_source::load_governance_tickets(&conn, &pid, false).unwrap_err();
    assert_eq!(err.code, 404);
}
