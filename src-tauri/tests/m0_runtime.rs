use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::symlink;
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
    if let Some(details) = result["error"].get("details") {
        let obj = details.as_object().unwrap();
        for key in obj.keys() {
            assert!(matches!(key.as_str(), "relative_path" | "revision" | "field"));
        }
        if let Some(path) = obj.get("relative_path") {
            let path = path.as_str().unwrap();
            assert!(!path.starts_with('/'));
            assert!(!path.contains(".."));
            assert!(!path.contains('\\'));
        }
    }
}

fn initialize(runtime: &mut AppRuntime, root: &Path) -> (String, String) {
    let scan = runtime.scan_project(root.to_str().unwrap());
    let data = assert_ok(&scan, "scan_project");
    let project_id = data["project_id"].as_str().unwrap().to_string();
    let fingerprint = data["root_fingerprint"].as_str().unwrap().to_string();
    assert!(fingerprint.starts_with("fp-v1:"));
    assert_eq!(fingerprint.len(), 22);
    let before = listing(root);
    let preview = runtime.preview_initialize(&project_id, root.to_str().unwrap(), &fingerprint);
    let preview_data = assert_ok(&preview, "preview_initialize");
    assert_eq!(listing(root), before, "preview must be zero-write");
    assert_eq!(preview_data["requires_confirmation"], true);
    let token = preview_data["confirmation_token"].as_str().unwrap();
    let init = runtime.initialize_project(&project_id, root.to_str().unwrap(), token, &fingerprint);
    assert_ok(&init, "initialize_project");
    (project_id, fingerprint)
}

#[test]
fn preview_is_zero_write_and_unconfirmed_initialize_is_rejected() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let scan = runtime.scan_project(root.to_str().unwrap());
    let data = assert_ok(&scan, "scan_project");
    let project_id = data["project_id"].as_str().unwrap();
    let fingerprint = data["root_fingerprint"].as_str().unwrap();
    let before = listing(&root);
    let preview = runtime.preview_initialize(project_id, root.to_str().unwrap(), fingerprint);
    assert_ok(&preview, "preview_initialize");
    assert_eq!(listing(&root), before);
    let missing = runtime.initialize_project(project_id, root.to_str().unwrap(), "", fingerprint);
    assert_err(&missing, "initialize_project", "confirmation_required");
    assert_eq!(listing(&root), before);
    let bogus = runtime.initialize_project(project_id, root.to_str().unwrap(), "tok-not-real", fingerprint);
    assert_err(&bogus, "initialize_project", "confirmation_expired");
    assert_eq!(listing(&root), before);
    fs::write(root.join("extra.txt"), "changed").unwrap();
    let token = preview["data"]["confirmation_token"].as_str().unwrap();
    let changed = runtime.initialize_project(project_id, root.to_str().unwrap(), token, fingerprint);
    assert_err(&changed, "initialize_project", "confirmation_expired");
    let after = listing(&root);
    assert!(!after.iter().any(|name| name == ".agentup" || name.starts_with(".agentup.tmp.")));
}

#[test]
fn initialize_uses_atomic_temp_directory_and_keeps_old_state_on_failure() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let (project_id, _) = initialize(&mut runtime, &root);
    let names = listing(&root);
    assert!(names.contains(".agentup"));
    assert!(names.contains(".agentup/manifest.json"));
    assert!(names.contains(".agentup/events"));
    assert!(!names.iter().any(|name| name.starts_with(".agentup.tmp.")));
    let manifest = fs::read(root.join(".agentup/manifest.json")).unwrap();
    let again = runtime.scan_project(root.to_str().unwrap());
    let data = assert_ok(&again, "scan_project");
    let preview = runtime.preview_initialize(
        data["project_id"].as_str().unwrap(),
        root.to_str().unwrap(),
        data["root_fingerprint"].as_str().unwrap(),
    );
    assert_err(&preview, "preview_initialize", "already_initialized");
    let init = runtime.initialize_project(
        &project_id,
        root.to_str().unwrap(),
        "tok-stale",
        data["root_fingerprint"].as_str().unwrap(),
    );
    assert_err(&init, "initialize_project", "already_initialized");
    assert_eq!(fs::read(root.join(".agentup/manifest.json")).unwrap(), manifest);
    assert!(!listing(&root).iter().any(|name| name.starts_with(".agentup.tmp.")));
}

#[test]
fn duplicate_request_id_and_stale_revision_do_not_write_again() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let (project_id, _) = initialize(&mut runtime, &root);
    let created = runtime.create_request(
        &project_id,
        "req-1",
        json!({"lifecycle": "draft", "title": "One", "body": "Body"}),
        json!({}),
        0,
    );
    let data = assert_ok(&created, "create_request");
    assert_eq!(data["fact"]["revision"], 1);
    assert_eq!(data["fact"]["source"], "user");
    assert_eq!(data["event"]["event_type"], "request.created");
    let before = listing(&root);
    let duplicate = runtime.create_request(
        &project_id,
        "req-1",
        json!({"lifecycle": "draft", "title": "Two", "body": "Other"}),
        json!({}),
        0,
    );
    assert_err(&duplicate, "create_request", "already_exists");
    let stale = runtime.create_request(
        &project_id,
        "req-1",
        json!({"lifecycle": "draft", "title": "Two", "body": "Other"}),
        json!({}),
        1,
    );
    assert_err(&stale, "create_request", "revision_conflict");
    assert_eq!(stale["error"]["details"]["revision"], 1);
    assert_eq!(listing(&root), before);
}

#[test]
fn close_and_reopen_load_project_restores_from_facts() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let (project_id, _) = initialize(&mut runtime, &root);
    assert_ok(
        &runtime.create_request(
            &project_id,
            "req-keep",
            json!({"title": "Keep", "body": "Persisted draft"}),
            json!({}),
            0,
        ),
        "create_request",
    );
    drop(runtime);
    let mut reopened = AppRuntime::new();
    let loaded = reopened.load_project(root.to_str().unwrap(), None);
    let data = assert_ok(&loaded, "load_project");
    assert_eq!(data["project_id"], project_id);
    assert_eq!(data["manifest"]["type"], "manifest");
    assert_eq!(data["requests"][0]["request_id"], "req-keep");
    assert_eq!(data["requests"][0]["lifecycle"], "draft");
    assert!(data["facts"].as_array().unwrap().iter().any(|fact| fact["type"] == "request"));
    assert!(data["events"].as_array().unwrap().iter().any(|event| event["event_type"] == "request.created"));
}

#[test]
fn path_outside_project_is_rejected_without_leaking_paths() {
    let (_tmp, root) = tmp_project();
    let outside = TempDir::new().unwrap();
    let mut runtime = AppRuntime::new();
    let scan = runtime.scan_project(root.to_str().unwrap());
    let data = assert_ok(&scan, "scan_project");
    let project_id = data["project_id"].as_str().unwrap();
    let fingerprint = data["root_fingerprint"].as_str().unwrap();
    let escaped = runtime.preview_initialize(
        project_id,
        outside.path().to_str().unwrap(),
        fingerprint,
    );
    assert_err(&escaped, "preview_initialize", "path_outside_project");
    let message = escaped["error"]["message"].as_str().unwrap();
    assert!(!message.contains(outside.path().to_str().unwrap()));
    assert!(!message.contains(root.to_str().unwrap()));
}

#[test]
fn symlink_traversal_is_rejected() {
    let (_tmp, root) = tmp_project();
    let outside = TempDir::new().unwrap();
    fs::write(outside.path().join("secret.txt"), "nope").unwrap();
    symlink(outside.path(), root.join("escape")).unwrap();
    let mut runtime = AppRuntime::new();
    let scan = runtime.scan_project(root.to_str().unwrap());
    assert_err(&scan, "scan_project", "path_outside_project");
    let message = scan["error"]["message"].as_str().unwrap();
    assert!(!message.contains(outside.path().to_str().unwrap()));
    assert!(!message.contains("secret.txt"));
}

#[test]
fn load_detects_event_replay() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let (project_id, _) = initialize(&mut runtime, &root);
    assert_ok(
        &runtime.create_request(
            &project_id,
            "req-keep",
            json!({"title": "Keep", "body": "Persisted draft"}),
            json!({}),
            0,
        ),
        "create_request",
    );
    let events_dir = root.join(".agentup/events");
    let original = fs::read_dir(&events_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .unwrap();
    let copied = events_dir.join("dup.json");
    fs::copy(&original, &copied).unwrap();
    let mut reopened = AppRuntime::new();
    let loaded = reopened.load_project(root.to_str().unwrap(), None);
    assert_err(&loaded, "load_project", "event_replay");
}
