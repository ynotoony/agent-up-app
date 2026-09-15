use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use agent_up_lib::AppRuntime;
use serde_json::{json, Value};
use tempfile::TempDir;

fn tmp_project() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("README.md"), "# demo\n").unwrap();
    fs::write(dir.path().join("package.json"), "{\"name\":\"demo\"}\n").unwrap();
    fs::create_dir_all(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
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
    let message = result["error"]["message"].as_str().unwrap();
    assert!(!message.contains("/Users/"), "{message}");
    assert!(!message.contains("/tmp/"), "{message}");
}

fn scan_id_fp(runtime: &mut AppRuntime, root: &Path) -> (String, String) {
    let scan = runtime.scan_project(root.to_str().unwrap());
    let data = assert_ok(&scan, "scan_project");
    (
        data["project_id"].as_str().unwrap().to_string(),
        data["root_fingerprint"].as_str().unwrap().to_string(),
    )
}

#[test]
fn external_file_change_expires_initialize_token() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let (project_id, fingerprint) = scan_id_fp(&mut runtime, &root);
    let preview = runtime.preview_initialize(&project_id, root.to_str().unwrap(), &fingerprint);
    let token = assert_ok(&preview, "preview_initialize")["confirmation_token"]
        .as_str()
        .unwrap()
        .to_string();
    fs::write(root.join("README.md"), "# changed outside\n").unwrap();
    let init = runtime.initialize_project(&project_id, root.to_str().unwrap(), &token, &fingerprint);
    assert_err(&init, "initialize_project", "confirmation_expired");
    assert!(!root.join(".agentup").exists());
}

#[test]
fn uncommitted_workdir_file_changes_fingerprint() {
    let (_tmp, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let (_id, fp1) = scan_id_fp(&mut runtime, &root);
    fs::write(root.join(".git/HEAD"), "ref: refs/heads/other\n").unwrap();
    let (_id, fp_git_only) = scan_id_fp(&mut runtime, &root);
    assert_eq!(fp1, fp_git_only, ".git contents must not change fp-v1");
    fs::write(root.join("src.txt"), "uncommitted worktree\n").unwrap();
    let (_id, fp2) = scan_id_fp(&mut runtime, &root);
    assert_ne!(fp1, fp2);
}

#[test]
fn symlink_escape_does_not_write() {
    let (_tmp, root) = tmp_project();
    let outside = TempDir::new().unwrap();
    fs::write(outside.path().join("secret"), "nope").unwrap();
    symlink(outside.path(), root.join("link-out")).unwrap();
    let mut runtime = AppRuntime::new();
    let scan = runtime.scan_project(root.to_str().unwrap());
    assert_err(&scan, "scan_project", "path_outside_project");
    assert!(!root.join(".agentup").exists());
}

#[test]
fn concurrent_stale_revision_does_not_drop_new_fact() {
    let (_tmp, root) = tmp_project();
    let mut a = AppRuntime::new();
    let (project_id, fingerprint) = scan_id_fp(&mut a, &root);
    let preview = a.preview_initialize(&project_id, root.to_str().unwrap(), &fingerprint);
    let token = assert_ok(&preview, "preview_initialize")["confirmation_token"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ok(
        &a.initialize_project(&project_id, root.to_str().unwrap(), &token, &fingerprint),
        "initialize_project",
    );
    let created = a.create_request(
        &project_id,
        "req-1",
        json!({"title": "one", "body": "body-one", "lifecycle": "draft"}),
        json!({}),
        0,
    );
    assert_ok(&created, "create_request");

    let mut b = AppRuntime::new();
    assert_ok(
        &b.load_project(root.to_str().unwrap(), Some(&project_id)),
        "load_project",
    );
    let stale = b.create_request(
        &project_id,
        "req-1",
        json!({"title": "two", "body": "body-two", "lifecycle": "draft"}),
        json!({}),
        0,
    );
    assert_err(&stale, "create_request", "already_exists");
    let conflict = a.create_request(
        &project_id,
        "req-1",
        json!({"title": "three", "body": "body-three", "lifecycle": "draft"}),
        json!({}),
        0,
    );
    assert_err(&conflict, "create_request", "already_exists");
}

#[test]
fn missing_directory_is_not_initialized() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("README.md"), "x").unwrap();
    let root = dir.path().canonicalize().unwrap();
    let gone = root.to_string_lossy().to_string();
    drop(dir);
    let mut runtime = AppRuntime::new();
    let scan = runtime.scan_project(&gone);
    assert_err(&scan, "scan_project", "path_not_found");
}
