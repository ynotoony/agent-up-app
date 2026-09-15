use std::fs;
use std::path::PathBuf;

use agent_up_lib::AppRuntime;
use serde_json::{json, Value};
use tempfile::TempDir;

fn tmp_project() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("README.md"), "# demo\n").unwrap();
    fs::write(dir.path().join("package.json"), "{\"name\":\"demo\"}\n").unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/out.rs"), "OUT\n").unwrap();
    let path = dir.path().canonicalize().unwrap();
    (dir, path)
}

fn assert_ok<'a>(result: &'a Value, command: &str) -> &'a Value {
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["command"], command);
    assert!(result.get("error").is_none());
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
    let fingerprint = data["root_fingerprint"].as_str().unwrap().to_string();
    let preview = runtime.preview_initialize(&project_id, root.to_str().unwrap(), &fingerprint);
    let token = assert_ok(&preview, "preview_initialize")["confirmation_token"]
        .as_str()
        .unwrap();
    assert_ok(
        &runtime.initialize_project(&project_id, root.to_str().unwrap(), token, &fingerprint),
        "initialize_project",
    );
    project_id
}

fn seed_board(runtime: &mut AppRuntime, project_id: &str) {
    assert_ok(
        &runtime.create_request(
            project_id,
            "req-1",
            json!({"title": "Reliability", "body": "Gate"}),
            json!({}),
            0,
        ),
        "create_request",
    );
    assert_ok(&runtime.put_task(project_id, "task-1", "req-1", "Main", None, None, 0, None), "put_task");
    assert_ok(
        &runtime.put_scope(
            project_id,
            "scope-1",
            "req-1",
            "task-1",
            json!([{
                "relative_path": "src/in.rs",
                "reason": "allowed",
                "source": "user",
                "included": true
            }]),
            0,
        ),
        "put_scope",
    );
}

fn start_impl(runtime: &mut AppRuntime, project_id: &str, run_id: &str) -> (Value, i64) {
    let raw = runtime.start_run(project_id, run_id, "req-1", "task-1", "implement", "fake", "pv-1", 0);
    let data = assert_ok(&raw, "start_run").clone();
    let rev = data["fact"]["revision"].as_i64().unwrap();
    (data, rev)
}

#[test]
fn cancel_run_blocks_later_writes() {
    let (_dir, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    seed_board(&mut runtime, &project_id);
    let (_data, rev) = start_impl(&mut runtime, &project_id, "run-1");
    let written = runtime.apply_fake_script(
        &project_id,
        "run-1",
        json!([{"tool": "write", "relative_path": "src/in.rs", "content": "IN\n"}]),
        rev,
    );
    let after_write = assert_ok(&written, "apply_fake_script")["fact"]["revision"].as_i64().unwrap();
    assert_ok(&runtime.cancel_run(&project_id, "run-1", after_write), "cancel_run");
    let denied = runtime.apply_fake_script(
        &project_id,
        "run-1",
        json!([{"tool": "write", "relative_path": "src/in.rs", "content": "NOPE\n"}]),
        after_write + 1,
    );
    assert_err(&denied, "apply_fake_script", "invalid_input");
    assert_eq!(fs::read_to_string(root.join("src/in.rs")).unwrap(), "IN\n");
}

#[test]
fn clock_timeout_persists_after_restart() {
    let (_dir, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    seed_board(&mut runtime, &project_id);
    let (_data, rev) = start_impl(&mut runtime, &project_id, "run-1");
    let timed = runtime.advance_run_clock(&project_id, "run-1", 900_000, rev);
    let data = assert_ok(&timed, "advance_run_clock");
    assert_eq!(data["fact"]["content"]["run_state"], "interrupted");
    assert_eq!(data["fact"]["content"]["error_code"], "timeout");
    drop(runtime);
    let mut runtime = AppRuntime::new();
    assert_ok(&runtime.load_project(root.to_str().unwrap(), Some(&project_id)), "load_project");
    let loaded_raw = runtime.load_run(&project_id, "run-1");
    let loaded = assert_ok(&loaded_raw, "load_run");
    assert_eq!(loaded["fact"]["content"]["run_state"], "interrupted");
    assert_eq!(loaded["fact"]["content"]["error_code"], "timeout");
}

#[test]
fn provider_timeout_retries_once_then_interrupts() {
    let (_dir, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    seed_board(&mut runtime, &project_id);
    let (_data, rev) = start_impl(&mut runtime, &project_id, "run-1");
    let first = runtime.apply_fake_script(&project_id, "run-1", json!([{"tool": "timeout"}]), rev);
    let first_data = assert_ok(&first, "apply_fake_script");
    assert_eq!(first_data["fact"]["content"]["run_state"], "active");
    assert_eq!(first_data["retried"], true);
    let rev2 = first_data["fact"]["revision"].as_i64().unwrap();
    let second = runtime.apply_fake_script(&project_id, "run-1", json!([{"tool": "timeout"}]), rev2);
    let second_data = assert_ok(&second, "apply_fake_script");
    assert_eq!(second_data["fact"]["content"]["run_state"], "interrupted");
    assert_eq!(second_data["fact"]["content"]["error_code"], "timeout");
}

#[test]
fn out_of_scope_write_does_not_retry() {
    let (_dir, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    seed_board(&mut runtime, &project_id);
    let (_data, rev) = start_impl(&mut runtime, &project_id, "run-1");
    let denied = runtime.apply_fake_script(
        &project_id,
        "run-1",
        json!([{"tool": "write", "relative_path": "src/out.rs", "content": "HACK\n"}]),
        rev,
    );
    assert_err(&denied, "apply_fake_script", "invalid_input");
    let loaded_raw = runtime.load_run(&project_id, "run-1");
    let loaded = assert_ok(&loaded_raw, "load_run");
    assert_eq!(loaded["fact"]["content"]["run_state"], "active");
    assert!(loaded["fact"]["content"].get("provider_timeout_count").is_none() || loaded["fact"]["content"]["provider_timeout_count"] == 0);
    assert_eq!(fs::read_to_string(root.join("src/out.rs")).unwrap(), "OUT\n");
}

#[test]
fn agent_run_does_not_spawn_processes() {
    let src = include_str!("../src/agent_run.rs");
    assert!(!src.contains("std::process"));
    assert!(!src.contains("Command::"));
    assert!(!src.contains("std::os::unix::process"));
}
