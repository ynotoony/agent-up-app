use std::fs;
use std::path::PathBuf;
use agent_up_lib::AppRuntime;
use serde_json::{json, Value};
use tempfile::TempDir;

fn tmp_frontend() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("package.json"), "{\"name\":\"ui\"}\n").unwrap();
    fs::write(dir.path().join("src/button.tsx"), "export function Button(){return <button>Go</button>}\n").unwrap();
    fs::write(dir.path().join("src/save.ts"), "export function save(){}\n").unwrap();
    let path = dir.path().canonicalize().unwrap();
    (dir, path)
}
fn assert_ok<'a>(r: &'a Value, cmd: &str) -> &'a Value {
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["command"], cmd);
    &r["data"]
}
fn assert_err(r: &Value, cmd: &str, code: &str) {
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["command"], cmd);
    assert_eq!(r["error"]["code"], code, "{r}");
}
fn initialize(rt: &mut AppRuntime, root: &std::path::Path) -> String {
    let scan = rt.scan_project(root.to_str().unwrap());
    let data = assert_ok(&scan, "scan_project");
    let pid = data["project_id"].as_str().unwrap().to_string();
    let fp = data["root_fingerprint"].as_str().unwrap().to_string();
    let preview = rt.preview_initialize(&pid, root.to_str().unwrap(), &fp);
    let token = assert_ok(&preview, "preview_initialize")["confirmation_token"].as_str().unwrap();
    assert_ok(&rt.initialize_project(&pid, root.to_str().unwrap(), token, &fp), "initialize_project");
    pid
}

#[test]
fn s1_ui_change_full_loop() {
    let (_d, root) = tmp_frontend();
    let mut rt = AppRuntime::new();
    let pid = initialize(&mut rt, &root);
    assert_ok(&rt.create_request(&pid, "req-1", json!({"title":"Primary button","body":"Make main button primary and keyboard focusable"}), json!({}), 0), "create_request");
    let att = rt.add_attachment(&pid, "req-1", "image/png", b"\x89PNG");
    let att_id = assert_ok(&att, "add_attachment")["fact"]["id"].as_str().unwrap().to_string();
    assert_ok(&rt.post_discussion(&pid, "req-1", "Do not change routing", Some(vec![att_id])), "post_discussion");
    assert_ok(&rt.put_task(&pid, "task-1", "req-1", "Restyle button", None, None, 0, None), "put_task");
    assert_ok(&rt.put_scope(&pid, "scope-1", "req-1", "task-1", json!([{"relative_path":"src/button.tsx","reason":"button","source":"user","included":true}]), 0), "put_scope");
    let impl_raw = rt.start_run(&pid, "run-impl", "req-1", "task-1", "implement", "fake", "pv-1", 0);
    let impl_rev = assert_ok(&impl_raw, "start_run")["fact"]["revision"].as_i64().unwrap();
    let wrote = rt.apply_fake_script(&pid, "run-impl", json!([{"tool":"write","relative_path":"src/button.tsx","content":"export function Button(){return <button autoFocus>Go</button>}\n"}]), impl_rev);
    let impl_rev2 = assert_ok(&wrote, "apply_fake_script")["fact"]["revision"].as_i64().unwrap();
    assert_ok(&rt.finish_run(&pid, "run-impl", impl_rev2, 1, 1, None), "finish_run");
    let rev_fail = rt.start_run(&pid, "run-rev-fail", "req-1", "task-1", "review", "fake", "pv-1", 0);
    let fail_rev = assert_ok(&rev_fail, "start_run")["fact"]["revision"].as_i64().unwrap();
    assert_ok(&rt.finish_run(&pid, "run-rev-fail", fail_rev, 1, 1, Some("fail".into())), "finish_run");
    assert_err(&rt.commit_changes(&pid, "task-1", "req-1"), "commit_changes", "review_required");
    let rev_pass = rt.start_run(&pid, "run-rev-pass", "req-1", "task-1", "review", "fake", "pv-1", 0);
    let pass_rev = assert_ok(&rev_pass, "start_run")["fact"]["revision"].as_i64().unwrap();
    assert_ok(&rt.finish_run(&pid, "run-rev-pass", pass_rev, 1, 1, Some("pass".into())), "finish_run");
    assert_ok(&rt.commit_changes(&pid, "task-1", "req-1"), "commit_changes");
    assert_ok(&rt.publish_result(&pid, "res-1", "req-1", "Button restyled", Some(vec!["src/button.tsx".into()])), "publish_result");
    assert_ok(&rt.accept_result(&pid, "req-1", "res-1", "res-acc"), "accept_result");
    assert_ok(&rt.submit_feedback(&pid, "req-1", "contrast is not enough", Some("Fix contrast".into())), "submit_feedback");
    drop(rt);
    let mut rt = AppRuntime::new();
    assert_ok(&rt.load_project(root.to_str().unwrap(), Some(&pid)), "load_project");
    let hist_raw = rt.load_result_history(&pid, "req-1");
    let hist = assert_ok(&hist_raw, "load_result_history");
    assert!(hist["results"].as_array().unwrap().len() >= 2);
    assert!(fs::read_to_string(root.join("src/button.tsx")).unwrap().contains("autoFocus"));
}

#[test]
fn s2_bugfix_scope_enforced() {
    let (_d, root) = tmp_frontend();
    let mut rt = AppRuntime::new();
    let pid = initialize(&mut rt, &root);
    assert_ok(&rt.create_request(&pid, "req-1", json!({"title":"Save does nothing","body":"click save no reaction"}), json!({}), 0), "create_request");
    assert_ok(&rt.put_task(&pid, "task-1", "req-1", "Fix save", None, None, 0, None), "put_task");
    assert_ok(&rt.put_scope(&pid, "scope-1", "req-1", "task-1", json!([{"relative_path":"src/save.ts","reason":"handler","source":"user","included":true}]), 0), "put_scope");
    let impl_raw = rt.start_run(&pid, "run-impl", "req-1", "task-1", "implement", "fake", "pv-1", 0);
    let rev = assert_ok(&impl_raw, "start_run")["fact"]["revision"].as_i64().unwrap();
    let denied = rt.apply_fake_script(&pid, "run-impl", json!([{"tool":"write","relative_path":"src/button.tsx","content":"HACK"}]), rev);
    assert_err(&denied, "apply_fake_script", "invalid_input");
    let ok_write = rt.apply_fake_script(&pid, "run-impl", json!([{"tool":"write","relative_path":"src/save.ts","content":"export function save(){return true}\n"}]), rev);
    let rev2 = assert_ok(&ok_write, "apply_fake_script")["fact"]["revision"].as_i64().unwrap();
    assert_ok(&rt.finish_run(&pid, "run-impl", rev2, 1, 1, None), "finish_run");
    let review = rt.start_run(&pid, "run-rev", "req-1", "task-1", "review", "fake", "pv-1", 0);
    let rrev = assert_ok(&review, "start_run")["fact"]["revision"].as_i64().unwrap();
    assert_ok(&rt.finish_run(&pid, "run-rev", rrev, 1, 1, Some("pass".into())), "finish_run");
    assert_ok(&rt.commit_changes(&pid, "task-1", "req-1"), "commit_changes");
    assert_ok(&rt.publish_result(&pid, "res-1", "req-1", "save fixed", None), "publish_result");
    assert!(fs::read_to_string(root.join("src/save.ts")).unwrap().contains("return true"));
}

#[test]
fn s3_eval_without_scope_cannot_write_source() {
    let (_d, root) = tmp_frontend();
    let mut rt = AppRuntime::new();
    let pid = initialize(&mut rt, &root);
    assert_ok(&rt.create_request(&pid, "req-1", json!({"title":"Worth shipping?","body":"evaluate only"}), json!({}), 0), "create_request");
    assert_ok(&rt.put_task(&pid, "task-1", "req-1", "Evaluate", None, None, 0, None), "put_task");
    let impl_raw = rt.start_run(&pid, "run-impl", "req-1", "task-1", "implement", "fake", "pv-1", 0);
    let rev = assert_ok(&impl_raw, "start_run")["fact"]["revision"].as_i64().unwrap();
    let denied = rt.apply_fake_script(&pid, "run-impl", json!([{"tool":"write","relative_path":"src/button.tsx","content":"NO"}]), rev);
    assert_err(&denied, "apply_fake_script", "invalid_input");
    assert_ok(&rt.publish_result(&pid, "res-1", "req-1", "Not worth coding this week", None), "publish_result");
    assert_ok(&rt.accept_result(&pid, "req-1", "res-1", "res-acc"), "accept_result");
    let original = fs::read_to_string(root.join("src/button.tsx")).unwrap();
    assert!(!original.contains("NO"));
}

#[test]
fn diagnostics_redact_paths_and_secrets() {
    let (_d, root) = tmp_frontend();
    let mut rt = AppRuntime::new();
    let pid = initialize(&mut rt, &root);
    assert_ok(&rt.create_request(&pid, "req-1", json!({"title":"t","body":"b"}), json!({"api_key":"sk-live-secret"}), 0), "create_request");
    let raw = rt.export_diagnostics(&pid);
    let data = assert_ok(&raw, "export_diagnostics");
    let rel = data["relative_path"].as_str().unwrap();
    let text = fs::read_to_string(root.join(".agentup").join(rel)).unwrap();
    assert!(!text.contains(root.to_str().unwrap()), "{text}");
    assert!(!text.contains("sk-live-secret"), "{text}");
    assert!(!text.contains("\\x89PNG") && !text.contains("bytes"), "{text}");
}
