use std::fs;
use std::path::PathBuf;
use agent_up_lib::AppRuntime;
use serde_json::{json, Value};
use tempfile::TempDir;

fn tmp_project() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("README.md"), "# demo\n").unwrap();
    fs::write(dir.path().join("package.json"), "{\"name\":\"demo\"}\n").unwrap();
    let path = dir.path().canonicalize().unwrap();
    (dir, path)
}
fn assert_ok<'a>(result: &'a Value, command: &str) -> &'a Value {
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["command"], command);
    &result["data"]
}
fn assert_err(result: &Value, command: &str, code: &str) {
    assert_eq!(result["ok"], false, "{result}");
    assert_eq!(result["command"], command);
    assert_eq!(result["error"]["code"], code, "{result}");
}
fn initialize(runtime: &mut AppRuntime, root: &std::path::Path) -> String {
    let scan = runtime.scan_project(root.to_str().unwrap());
    let data = assert_ok(&scan, "scan_project");
    let project_id = data["project_id"].as_str().unwrap().to_string();
    let fp = data["root_fingerprint"].as_str().unwrap().to_string();
    let preview = runtime.preview_initialize(&project_id, root.to_str().unwrap(), &fp);
    let token = assert_ok(&preview, "preview_initialize")["confirmation_token"].as_str().unwrap();
    assert_ok(&runtime.initialize_project(&project_id, root.to_str().unwrap(), token, &fp), "initialize_project");
    project_id
}
fn seed(runtime: &mut AppRuntime, project_id: &str) {
    assert_ok(&runtime.create_request(project_id, "req-1", json!({"title":"R","body":"b"}), json!({}), 0), "create_request");
}

#[test]
fn two_publish_versions_keep_first_content() {
    let (_d, root) = tmp_project();
    let mut rt = AppRuntime::new();
    let pid = initialize(&mut rt, &root);
    seed(&mut rt, &pid);
    let a_raw = rt.publish_result(&pid, "res-1", "req-1", "first", None);
    let a = assert_ok(&a_raw, "publish_result");
    assert_eq!(a["fact"]["content"]["version"], 1);
    let b_raw = rt.publish_result(&pid, "res-2", "req-1", "second", None);
    let b = assert_ok(&b_raw, "publish_result");
    assert_eq!(b["fact"]["content"]["version"], 2);
    let hist_raw = rt.load_result_history(&pid, "req-1");
    let hist = assert_ok(&hist_raw, "load_result_history");
    assert_eq!(hist["results"].as_array().unwrap().len(), 2);
    assert_eq!(hist["results"][0]["content"]["summary"], "first");
    assert_eq!(hist["results"][1]["content"]["summary"], "second");
}

#[test]
fn accept_then_reject_keeps_old_accepted() {
    let (_d, root) = tmp_project();
    let mut rt = AppRuntime::new();
    let pid = initialize(&mut rt, &root);
    seed(&mut rt, &pid);
    assert_ok(&rt.publish_result(&pid, "res-1", "req-1", "first", None), "publish_result");
    assert_ok(&rt.accept_result(&pid, "req-1", "res-1", "res-acc"), "accept_result");
    assert_ok(&rt.reject_result(&pid, "req-1", "res-acc", "res-rej"), "reject_result");
    let hist_raw = rt.load_result_history(&pid, "req-1");
    let hist = assert_ok(&hist_raw, "load_result_history");
    let acc = hist["results"].as_array().unwrap().iter().find(|r| r["id"]=="res-acc").unwrap();
    assert_eq!(acc["content"]["acceptance"], "accepted");
    let first = hist["results"].as_array().unwrap().iter().find(|r| r["id"]=="res-1").unwrap();
    assert_eq!(first["content"]["acceptance"], "pending");
}

#[test]
fn feedback_does_not_change_result() {
    let (_d, root) = tmp_project();
    let mut rt = AppRuntime::new();
    let pid = initialize(&mut rt, &root);
    seed(&mut rt, &pid);
    assert_ok(&rt.publish_result(&pid, "res-1", "req-1", "first", None), "publish_result");
    assert_ok(&rt.accept_result(&pid, "req-1", "res-1", "res-acc"), "accept_result");
    assert_ok(&rt.submit_feedback(&pid, "req-1", "need contrast", Some("Follow-up".into())), "submit_feedback");
    let hist_raw = rt.load_result_history(&pid, "req-1");
    let hist = assert_ok(&hist_raw, "load_result_history");
    let acc = hist["results"].as_array().unwrap().iter().find(|r| r["id"]=="res-acc").unwrap();
    assert_eq!(acc["content"]["summary"], "first");
    assert_eq!(acc["content"]["acceptance"], "accepted");
    let thread_raw = rt.load_request_thread(&pid, "req-1");
    let thread = assert_ok(&thread_raw, "load_request_thread");
    let bodies: Vec<_> = thread["discussions"].as_array().unwrap().iter().filter_map(|i| i["content"]["body"].as_str()).collect();
    assert!(bodies.iter().any(|b| *b == "need contrast"), "{thread}");
}

#[test]
fn history_survives_restart() {
    let (_d, root) = tmp_project();
    let mut rt = AppRuntime::new();
    let pid = initialize(&mut rt, &root);
    seed(&mut rt, &pid);
    assert_ok(&rt.publish_result(&pid, "res-1", "req-1", "first", None), "publish_result");
    drop(rt);
    let mut rt = AppRuntime::new();
    assert_ok(&rt.load_project(root.to_str().unwrap(), Some(&pid)), "load_project");
    let hist_raw = rt.load_result_history(&pid, "req-1");
    let hist = assert_ok(&hist_raw, "load_result_history");
    assert_eq!(hist["results"][0]["content"]["summary"], "first");
}

#[test]
fn absolute_evidence_rejected() {
    let (_d, root) = tmp_project();
    let mut rt = AppRuntime::new();
    let pid = initialize(&mut rt, &root);
    seed(&mut rt, &pid);
    assert_err(&rt.publish_result(&pid, "res-1", "req-1", "first", Some(vec!["/tmp/x".into()])), "publish_result", "invalid_input");
}
