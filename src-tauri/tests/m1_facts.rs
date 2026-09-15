use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use agent_up_lib::AppRuntime;
use serde_json::{json, Value};
use tempfile::TempDir;

fn listing(root: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    fn walk(dir: &Path, rel: &str, names: &mut BTreeSet<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let child_rel = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            names.insert(child_rel.clone());
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                walk(&entry.path(), &child_rel, names);
            }
        }
    }
    walk(root, "", &mut names);
    names
}

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
    let message = result["error"]["message"].as_str().unwrap();
    assert!(!message.contains("/Users/"), "{message}");
    assert!(!message.contains("/tmp/"), "{message}");
    assert!(!message.contains(".."));
}

fn initialize(runtime: &mut AppRuntime, root: &Path) -> String {
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

fn sha256_hex(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

fn write(
    runtime: &mut AppRuntime,
    project_id: &str,
    fact_id: &str,
    fact_type: &str,
    content: Value,
    expected_revision: i64,
    event_id: Option<&str>,
) -> Value {
    runtime.write_fact(
        project_id,
        fact_id,
        fact_type,
        content,
        json!({}),
        expected_revision,
        event_id,
    )
}

fn eight_contents(posted_at: &str) -> [(&'static str, &'static str, Value); 8] {
    [
        (
            "discussion",
            "disc-1",
            json!({
                "request_id": "req-1",
                "author_source": "user",
                "body": "Let's keep this fact.",
                "posted_at": posted_at,
                "attachment_ids": ["att-1"]
            }),
        ),
        (
            "plan",
            "plan-1",
            json!({
                "request_id": "req-1",
                "summary": "Ship typed facts",
                "success_criteria": ["Eight types round-trip"],
                "constraints": ["No SQLite"],
                "status": "active"
            }),
        ),
        (
            "task",
            "task-1",
            json!({
                "request_id": "req-1",
                "title": "Write facts",
                "task_state": "in_progress",
                "parent_id": null,
                "depends_on": ["task-0"]
            }),
        ),
        (
            "scope",
            "scope-1",
            json!({
                "request_id": "req-1",
                "task_id": "task-1",
                "entries": [{
                    "relative_path": "src-tauri/src/runtime.rs",
                    "reason": "writer lives here",
                    "source": "user",
                    "included": true
                }]
            }),
        ),
        (
            "run",
            "run-1",
            json!({
                "request_id": "req-1",
                "task_id": "task-1",
                "kind": "implement",
                "run_state": "closed",
                "started_at": posted_at,
                "ended_at": posted_at
            }),
        ),
        (
            "decision",
            "dec-1",
            json!({
                "request_id": "req-1",
                "question": "Persist events?",
                "options": ["yes", "no"],
                "status": "chosen",
                "chosen": "yes",
                "rationale": "SPEC-003 requires it"
            }),
        ),
        (
            "result",
            "res-1",
            json!({
                "request_id": "req-1",
                "version": 1,
                "summary": "Facts persisted",
                "acceptance": "pending",
                "evidence_paths": ["docs/issues/04-m1-fact-types.md"]
            }),
        ),
        (
            "attachment",
            "att-1",
            json!({
                "request_id": "req-1",
                "relative_path": "attachments/note.txt",
                "media_type": "text/plain",
                "byte_length": 4,
                "sha256": sha256_hex(0xab)
            }),
        ),
    ]
}

#[test]
fn eight_typed_facts_persist_and_reload_after_restart() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    let posted_at = "2026-09-15T05:00:00.000Z";
    let mut written = Vec::new();
    for (fact_type, fact_id, content) in eight_contents(posted_at) {
        let result = write(
            &mut runtime,
            &project_id,
            fact_id,
            fact_type,
            content.clone(),
            0,
            None,
        );
        let data = assert_ok(&result, "write_fact");
        assert_eq!(data["fact"]["type"], fact_type);
        assert_eq!(data["fact"]["id"], fact_id);
        assert_eq!(data["fact"]["revision"], 1);
        assert_eq!(data["fact"]["content"], content);
        assert!(data["fact"]["content"].get("bytes").is_none());
        written.push((fact_type, fact_id, content, data["event"].clone()));
    }
    let names = listing(&root);
    assert!(names.contains(".agentup/facts/discussions/disc-1.r1.json"));
    assert!(names.contains(".agentup/facts/plans/plan-1.r1.json"));
    assert!(names.contains(".agentup/facts/tasks/task-1.r1.json"));
    assert!(names.contains(".agentup/facts/scopes/scope-1.r1.json"));
    assert!(names.contains(".agentup/facts/runs/run-1.r1.json"));
    assert!(names.contains(".agentup/facts/decisions/dec-1.r1.json"));
    assert!(names.contains(".agentup/facts/results/res-1.r1.json"));
    assert!(names.contains(".agentup/facts/attachments/att-1.r1.json"));
    drop(runtime);

    let mut reopened = AppRuntime::new();
    let loaded = reopened.load_project(root.to_str().unwrap(), None);
    let load_data = assert_ok(&loaded, "load_project");
    let events = load_data["events"].as_array().unwrap();
    for required in [
        "discussion.posted",
        "task.state_changed",
        "scope.updated",
        "decision.recorded",
        "result.published",
    ] {
        assert!(
            events.iter().any(|event| event["event_type"] == required),
            "missing {required} in {events:?}"
        );
    }
    for (fact_type, fact_id, content, _event) in written {
        let read = reopened.read_fact(&project_id, fact_type, fact_id);
        let data = assert_ok(&read, "read_fact");
        assert_eq!(data["fact"]["content"], content);
        assert_eq!(data["fact"]["revision"], 1);
        assert!(data["fact"]["content"].get("bytes").is_none());
        assert!(data["fact"]["content"].get("data").is_none());
    }
}

#[test]
fn stale_revision_is_rejected_and_does_not_overwrite() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    let content = json!({
        "request_id": "req-1",
        "author_source": "user",
        "body": "first",
        "posted_at": "2026-09-15T05:00:00.000Z"
    });
    assert_ok(
        &write(
            &mut runtime,
            &project_id,
            "disc-cas",
            "discussion",
            content.clone(),
            0,
            None,
        ),
        "write_fact",
    );
    let updated = json!({
        "request_id": "req-1",
        "author_source": "user",
        "body": "second",
        "posted_at": "2026-09-15T05:01:00.000Z"
    });
    let second = write(
        &mut runtime,
        &project_id,
        "disc-cas",
        "discussion",
        updated.clone(),
        1,
        None,
    );
    assert_ok(&second, "write_fact");
    assert_eq!(second["data"]["fact"]["revision"], 2);
    let before = listing(&root);
    let stale = write(
        &mut runtime,
        &project_id,
        "disc-cas",
        "discussion",
        content,
        1,
        None,
    );
    assert_err(&stale, "write_fact", "revision_conflict");
    assert_eq!(stale["error"]["details"]["revision"], 2);
    assert_eq!(listing(&root), before);
    let read = runtime.read_fact(&project_id, "discussion", "disc-cas");
    assert_eq!(
        assert_ok(&read, "read_fact")["fact"]["content"]["body"],
        "second"
    );
}

#[test]
fn duplicate_event_id_is_event_replay_and_does_not_apply_again() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    let content = json!({
        "request_id": "req-1",
        "author_source": "user",
        "body": "posted",
        "posted_at": "2026-09-15T05:00:00.000Z"
    });
    assert_ok(
        &write(
            &mut runtime,
            &project_id,
            "disc-evt",
            "discussion",
            content.clone(),
            0,
            Some("evt-disc-dup"),
        ),
        "write_fact",
    );
    let before = listing(&root);
    let replay = write(
        &mut runtime,
        &project_id,
        "disc-other",
        "discussion",
        content,
        0,
        Some("evt-disc-dup"),
    );
    assert_err(&replay, "write_fact", "event_replay");
    assert_eq!(listing(&root), before);
    let missing = runtime.read_fact(&project_id, "discussion", "disc-other");
    assert_err(&missing, "read_fact", "invalid_input");
}

#[test]
fn attachment_rejects_bytes_and_oversize() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    let with_bytes = write(
        &mut runtime,
        &project_id,
        "att-bytes",
        "attachment",
        json!({
            "request_id": "req-1",
            "relative_path": "attachments/note.txt",
            "media_type": "text/plain",
            "byte_length": 4,
            "sha256": sha256_hex(1),
            "bytes": "bm90ZQ=="
        }),
        0,
        None,
    );
    assert_err(&with_bytes, "write_fact", "malformed_fact");
    let oversize = write(
        &mut runtime,
        &project_id,
        "att-big",
        "attachment",
        json!({
            "request_id": "req-1",
            "relative_path": "attachments/big.bin",
            "media_type": "application/octet-stream",
            "byte_length": 10 * 1024 * 1024 + 1,
            "sha256": sha256_hex(2)
        }),
        0,
        None,
    );
    assert_err(&oversize, "write_fact", "invalid_input");
    for index in 0..10 {
        let ok_write = write(
            &mut runtime,
            &project_id,
            &format!("att-fill-{index}"),
            "attachment",
            json!({
                "request_id": "req-1",
                "relative_path": format!("attachments/fill-{index}.bin"),
                "media_type": "application/octet-stream",
                "byte_length": 10 * 1024 * 1024,
                "sha256": sha256_hex(index as u8)
            }),
            0,
            None,
        );
        assert_ok(&ok_write, "write_fact");
    }
    let overflow = write(
        &mut runtime,
        &project_id,
        "att-overflow",
        "attachment",
        json!({
            "request_id": "req-1",
            "relative_path": "attachments/overflow.bin",
            "media_type": "application/octet-stream",
            "byte_length": 1,
            "sha256": sha256_hex(99)
        }),
        0,
        None,
    );
    assert_err(&overflow, "write_fact", "invalid_input");
    let stored =
        fs::read_to_string(root.join(".agentup/facts/attachments/att-fill-0.r1.json")).unwrap();
    assert!(!stored.contains("bytes"));
    let names = listing(&root);
    assert!(!names.iter().any(|name| name.contains("overflow")));
}
