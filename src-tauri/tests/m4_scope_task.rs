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
    assert!(result.get("error").is_none());
    &result["data"]
}

fn assert_err(result: &Value, command: &str, code: &str) {
    assert_eq!(result["ok"], false, "{result}");
    assert_eq!(result["command"], command);
    assert_eq!(result["error"]["code"], code, "{result}");
    assert!(result["error"]["message"].as_str().unwrap().len() > 0);
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

fn seed_request(runtime: &mut AppRuntime, project_id: &str) {
    assert_ok(
        &runtime.create_request(
            project_id,
            "req-1",
            json!({"title": "Scope work", "body": "Need a board."}),
            json!({}),
            0,
        ),
        "create_request",
    );
}

#[test]
fn two_put_scope_diff_covers_added_removed_changed() {
    let (_dir, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    seed_request(&mut runtime, &project_id);
    assert_ok(
        &runtime.put_task(&project_id, "task-1", "req-1", "Main", None, None, 0, None),
        "put_task",
    );
    let first_entries = json!([
        {"relative_path": "src/a.rs", "reason": "entry", "source": "user", "included": true},
        {"relative_path": "src/b.rs", "reason": "old", "source": "user", "included": true}
    ]);
    assert_ok(
        &runtime.put_scope(&project_id, "scope-1", "req-1", "task-1", first_entries, 0),
        "put_scope",
    );
    let second_entries = json!([
        {"relative_path": "src/a.rs", "reason": "entry", "source": "user", "included": true},
        {"relative_path": "src/b.rs", "reason": "new", "source": "user", "included": true},
        {"relative_path": "src/c.rs", "reason": "added", "source": "user", "included": true}
    ]);
    let second_result =
        runtime.put_scope(&project_id, "scope-1", "req-1", "task-1", second_entries, 1);
    let second = assert_ok(&second_result, "put_scope");
    assert_eq!(second["event"]["event_type"], "scope.updated");
    let diff_result = runtime.diff_scope("scope-1", 1, 2);
    let diff = assert_ok(&diff_result, "diff_scope");
    assert_eq!(diff["added"], json!(["src/c.rs"]));
    assert_eq!(diff["removed"], json!([]));
    assert_eq!(diff["changed"], json!(["src/b.rs"]));

    let third_entries = json!([
        {"relative_path": "src/c.rs", "reason": "added", "source": "user", "included": true}
    ]);
    assert_ok(
        &runtime.put_scope(&project_id, "scope-1", "req-1", "task-1", third_entries, 2),
        "put_scope",
    );
    let diff2_result = runtime.diff_scope("scope-1", 2, 3);
    let diff2 = assert_ok(&diff2_result, "diff_scope");
    let removed = diff2["removed"].as_array().unwrap();
    let removed: Vec<_> = removed.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(removed.contains(&"src/a.rs"));
    assert!(removed.contains(&"src/b.rs"));
    assert_eq!(diff2["added"], json!([]));
}

#[test]
fn put_task_rejects_content_task_state_and_set_task_state_emits() {
    let (_dir, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    seed_request(&mut runtime, &project_id);
    let rejected = runtime.put_task(
        &project_id,
        "task-1",
        "req-1",
        "Main",
        None,
        None,
        0,
        Some(json!({"task_state": "done"})),
    );
    assert_err(&rejected, "put_task", "invalid_input");
    let created_result =
        runtime.put_task(&project_id, "task-1", "req-1", "Main", None, None, 0, None);
    let created = assert_ok(&created_result, "put_task");
    assert_eq!(created["fact"]["content"]["task_state"], "ready");
    let changed_result = runtime.set_task_state("task-1", "in_progress", 1);
    let changed = assert_ok(&changed_result, "set_task_state");
    assert_eq!(changed["event"]["event_type"], "task.state_changed");
    assert_eq!(changed["event"]["payload"]["task_state"], "in_progress");
    let board_result = runtime.load_board(&project_id, "req-1");
    let board = assert_ok(&board_result, "load_board");
    assert_eq!(board["tasks"][0]["task_state"], "in_progress");
    assert_err(
        &runtime.set_task_state("task-1", "not-a-state", 2),
        "set_task_state",
        "invalid_input",
    );
}

#[test]
fn subtask_parent_survives_runtime_restart() {
    let (_dir, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    seed_request(&mut runtime, &project_id);
    assert_ok(
        &runtime.put_task(
            &project_id,
            "task-main",
            "req-1",
            "Main",
            None,
            None,
            0,
            None,
        ),
        "put_task",
    );
    assert_ok(
        &runtime.put_task(
            &project_id,
            "task-child",
            "req-1",
            "Child",
            Some("task-main"),
            Some(vec!["task-main".to_string()]),
            0,
            None,
        ),
        "put_task",
    );
    drop(runtime);
    let mut reopened = AppRuntime::new();
    assert_ok(
        &reopened.load_project(root.to_str().unwrap(), Some(&project_id)),
        "load_project",
    );
    let board_result = reopened.load_board(&project_id, "req-1");
    let board = assert_ok(&board_result, "load_board");
    let tasks = board["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 2);
    let child = tasks.iter().find(|t| t["task_id"] == "task-child").unwrap();
    assert_eq!(child["parent_id"], "task-main");
    assert_eq!(child["depends_on"], json!(["task-main"]));
    assert_eq!(child["task_state"], "ready");
}

#[test]
fn stale_revision_returns_conflict_with_current_revision() {
    let (_dir, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    seed_request(&mut runtime, &project_id);
    assert_ok(
        &runtime.put_task(&project_id, "task-1", "req-1", "Main", None, None, 0, None),
        "put_task",
    );
    let entries = json!([
        {"relative_path": "src/a.rs", "reason": "entry", "source": "user", "included": true}
    ]);
    assert_ok(
        &runtime.put_scope(
            &project_id,
            "scope-1",
            "req-1",
            "task-1",
            entries.clone(),
            0,
        ),
        "put_scope",
    );
    let conflict = runtime.put_scope(&project_id, "scope-1", "req-1", "task-1", entries, 0);
    assert_err(&conflict, "put_scope", "revision_conflict");
    assert_eq!(conflict["error"]["details"]["revision"], 1);
    let task_conflict = runtime.set_task_state("task-1", "done", 0);
    assert_err(&task_conflict, "set_task_state", "revision_conflict");
    assert_eq!(task_conflict["error"]["details"]["revision"], 1);
}
