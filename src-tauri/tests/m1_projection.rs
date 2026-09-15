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

fn fact_listing(root: &Path) -> BTreeSet<String> {
    listing(root)
        .into_iter()
        .filter(|name| !name.contains("cache.sqlite"))
        .collect()
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
                "constraints": ["No SQLite authority"],
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

fn seed_facts(runtime: &mut AppRuntime, project_id: &str) {
    assert_ok(
        &runtime.create_request(
            project_id,
            "req-1",
            json!({
                "title": "Typed facts",
                "body": "Need a projection that is not the source of truth."
            }),
            json!({}),
            0,
        ),
        "create_request",
    );
    for (fact_type, fact_id, content) in eight_contents("2026-09-15T05:00:00.000Z") {
        assert_ok(
            &write(runtime, project_id, fact_id, fact_type, content, 0, None),
            "write_fact",
        );
    }
    let second_discussion = json!({
        "request_id": "req-1",
        "author_source": "user",
        "body": "Second revision stays on disk.",
        "posted_at": "2026-09-15T05:01:00.000Z",
        "attachment_ids": ["att-1"]
    });
    assert_ok(
        &write(
            runtime,
            project_id,
            "disc-1",
            "discussion",
            second_discussion,
            1,
            None,
        ),
        "write_fact",
    );
}

fn index_revisions(facts: &Value, fact_type: &str, fact_id: &str) -> Vec<i64> {
    facts
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| fact["type"] == fact_type && fact["id"] == fact_id)
        .map(|fact| fact["revision"].as_i64().unwrap())
        .collect()
}

#[test]
fn rebuild_indexes_eight_facts_and_matches_after_sqlite_delete() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    seed_facts(&mut runtime, &project_id);
    let before_facts = fact_listing(&root);
    let first = runtime.rebuild_projection(&project_id);
    let first_data = assert_ok(&first, "rebuild_projection");
    assert_eq!(first_data["sqlite_path"], ".agentup/cache.sqlite");
    assert!(root.join(".agentup/cache.sqlite").is_file());
    assert_eq!(fact_listing(&root), before_facts);

    let facts = &first_data["facts"];
    for fact_type in [
        "manifest",
        "request",
        "discussion",
        "plan",
        "task",
        "scope",
        "run",
        "decision",
        "result",
        "attachment",
    ] {
        assert!(
            facts
                .as_array()
                .unwrap()
                .iter()
                .any(|fact| fact["type"] == fact_type),
            "missing {fact_type} in {facts}"
        );
    }
    assert_eq!(index_revisions(facts, "discussion", "disc-1"), vec![1, 2]);
    assert_eq!(index_revisions(facts, "task", "task-1"), vec![1]);
    assert_eq!(index_revisions(facts, "request", "fact-req-1"), vec![1]);
    let events = first_data["events"].as_array().unwrap();
    for required in [
        "project.initialized",
        "request.created",
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

    fs::remove_file(root.join(".agentup/cache.sqlite")).unwrap();
    assert!(!root.join(".agentup/cache.sqlite").exists());
    let loaded = runtime.load_project(root.to_str().unwrap(), Some(&project_id));
    assert_ok(&loaded, "load_project");
    let read = runtime.read_fact(&project_id, "discussion", "disc-1");
    assert_eq!(assert_ok(&read, "read_fact")["fact"]["revision"], 2);

    let second = runtime.rebuild_projection(&project_id);
    let second_data = assert_ok(&second, "rebuild_projection");
    assert_eq!(second_data["facts"], first_data["facts"]);
    assert_eq!(second_data["events"], first_data["events"]);
    assert_eq!(fact_listing(&root), before_facts);
}

#[test]
fn missing_or_corrupt_sqlite_does_not_block_load_from_facts() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    seed_facts(&mut runtime, &project_id);
    assert_ok(
        &runtime.rebuild_projection(&project_id),
        "rebuild_projection",
    );
    fs::write(
        root.join(".agentup/cache.sqlite"),
        b"this is not a sqlite database",
    )
    .unwrap();

    drop(runtime);
    let mut reopened = AppRuntime::new();
    let loaded = reopened.load_project(root.to_str().unwrap(), None);
    let load_data = assert_ok(&loaded, "load_project");
    assert_eq!(load_data["project_id"], project_id);
    assert_eq!(load_data["requests"][0]["request_id"], "req-1");
    let read = reopened.read_fact(&project_id, "plan", "plan-1");
    assert_eq!(
        assert_ok(&read, "read_fact")["fact"]["content"]["summary"],
        "Ship typed facts"
    );
    assert_eq!(
        fs::read(root.join(".agentup/cache.sqlite")).unwrap(),
        b"this is not a sqlite database"
    );

    fs::remove_file(root.join(".agentup/cache.sqlite")).unwrap();
    let loaded_missing = reopened.load_project(root.to_str().unwrap(), Some(&project_id));
    assert_ok(&loaded_missing, "load_project");
    assert!(!root.join(".agentup/cache.sqlite").exists());
}

#[test]
fn rebuild_does_not_repair_malformed_facts() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    let malformed_path = root.join(".agentup/facts/discussions/disc-bad.r1.json");
    fs::create_dir_all(malformed_path.parent().unwrap()).unwrap();
    let garbage = "{not-json and extra fields that a projection must not repair";
    fs::write(&malformed_path, garbage).unwrap();
    let before = listing(&root);

    let rebuilt = runtime.rebuild_projection(&project_id);
    assert_err(&rebuilt, "rebuild_projection", "malformed_fact");
    assert_eq!(listing(&root), before);
    assert_eq!(fs::read_to_string(&malformed_path).unwrap(), garbage);
    assert!(!root.join(".agentup/cache.sqlite").exists());

    fs::remove_file(&malformed_path).unwrap();
    seed_facts(&mut runtime, &project_id);
    assert_ok(
        &runtime.rebuild_projection(&project_id),
        "rebuild_projection",
    );
    let valid = fs::read_to_string(root.join(".agentup/facts/discussions/disc-1.r1.json")).unwrap();
    let broken = valid.replace("\"Let's keep this fact.\"", "\"\"");
    fs::write(
        root.join(".agentup/facts/discussions/disc-1.r1.json"),
        &broken,
    )
    .unwrap();
    let sqlite_before = fs::read(root.join(".agentup/cache.sqlite")).unwrap();
    let broken_before =
        fs::read_to_string(root.join(".agentup/facts/discussions/disc-1.r1.json")).unwrap();
    let failed = runtime.rebuild_projection(&project_id);
    assert_err(&failed, "rebuild_projection", "malformed_fact");
    assert_eq!(
        fs::read_to_string(root.join(".agentup/facts/discussions/disc-1.r1.json")).unwrap(),
        broken_before
    );
    assert_eq!(
        fs::read(root.join(".agentup/cache.sqlite")).unwrap(),
        sqlite_before
    );
    assert!(!broken_before.contains("Let's keep this fact."));
}
