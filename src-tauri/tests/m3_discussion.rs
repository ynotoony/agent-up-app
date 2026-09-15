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

#[test]
fn discussion_and_two_attachments_survive_runtime_restart() {
    let (_dir, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    assert_ok(
        &runtime.create_request(
            &project_id,
            "req-1",
            json!({"title": "Need shots", "body": "Please review the screenshots."}),
            json!({}),
            0,
        ),
        "create_request",
    );

    let first_bytes = b"tiny-a";
    let second_bytes = b"tiny-b";
    let first_result = runtime.add_attachment(&project_id, "req-1", "image/png", first_bytes);
    let first = assert_ok(&first_result, "add_attachment");
    let second_result = runtime.add_attachment(&project_id, "req-1", "image/jpeg", second_bytes);
    let second = assert_ok(&second_result, "add_attachment");
    let first_id = first["fact"]["id"].as_str().unwrap().to_string();
    let second_id = second["fact"]["id"].as_str().unwrap().to_string();
    assert_eq!(first["relative_path"], format!("attachments/{first_id}"));
    assert_eq!(second["relative_path"], format!("attachments/{second_id}"));
    assert_eq!(
        first["sha256"],
        "d450127b6e7b4d70e88642c49ffde18c553902880f011dce2c51e9b4e910ba36"
    );
    assert_eq!(
        second["sha256"],
        "73b8f56c359efeb22b93053671697b74c0445f6779a4b69f07f5a71ccc379b99"
    );
    assert!(first["fact"]["content"].get("bytes").is_none());
    assert!(second["fact"]["content"].get("bytes").is_none());

    let posted_result = runtime.post_discussion(
        &project_id,
        "req-1",
        "Here are two screenshots.",
        Some(vec![first_id.clone(), second_id.clone()]),
    );
    let posted = assert_ok(&posted_result, "post_discussion");
    assert_eq!(posted["event"]["event_type"], "discussion.posted");
    assert_eq!(posted["fact"]["type"], "discussion");
    assert_eq!(
        posted["fact"]["content"]["body"],
        "Here are two screenshots."
    );
    assert_eq!(posted["fact"]["source"], "user");

    let first_path = root.join(".agentup/attachments").join(&first_id);
    let second_path = root.join(".agentup/attachments").join(&second_id);
    assert_eq!(fs::read(&first_path).unwrap(), first_bytes);
    assert_eq!(fs::read(&second_path).unwrap(), second_bytes);

    drop(runtime);
    let mut reopened = AppRuntime::new();
    assert_ok(
        &reopened.load_project(root.to_str().unwrap(), Some(&project_id)),
        "load_project",
    );
    let thread_result = reopened.load_request_thread(&project_id, "req-1");
    let thread = assert_ok(&thread_result, "load_request_thread");
    assert_eq!(thread["discussions"].as_array().unwrap().len(), 1);
    assert_eq!(
        thread["discussions"][0]["content"]["body"],
        "Here are two screenshots."
    );
    let attachments = thread["attachments"].as_array().unwrap();
    assert_eq!(attachments.len(), 2);
    let restored: BTreeSet<_> = attachments
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        restored,
        BTreeSet::from([first_id.clone(), second_id.clone()])
    );
    for item in attachments {
        assert!(item.get("bytes").is_none());
        let id = item["id"].as_str().unwrap();
        let on_disk = fs::read(root.join(".agentup/attachments").join(id)).unwrap();
        assert_eq!(item["byte_length"], on_disk.len());
        if id == first_id {
            assert_eq!(on_disk, first_bytes);
            assert_eq!(item["sha256"], first["sha256"]);
            assert_eq!(item["relative_path"], format!("attachments/{first_id}"));
        } else {
            assert_eq!(on_disk, second_bytes);
            assert_eq!(item["sha256"], second["sha256"]);
            assert_eq!(item["relative_path"], format!("attachments/{second_id}"));
        }
    }

    let names = listing(&root);
    assert!(names.contains(&format!(".agentup/attachments/{first_id}")));
    assert!(names.contains(&format!(".agentup/attachments/{second_id}")));
    assert!(names.contains(".agentup/facts/discussions"));
}

#[test]
fn oversize_attachment_is_rejected_without_writing_files() {
    let (_dir, root) = tmp_project();
    let mut runtime = AppRuntime::new();
    let project_id = initialize(&mut runtime, &root);
    assert_ok(
        &runtime.create_request(
            &project_id,
            "req-1",
            json!({"title": "Need shots", "body": "Please review the screenshots."}),
            json!({}),
            0,
        ),
        "create_request",
    );
    let before = listing(&root);
    let too_big = vec![0u8; 10 * 1024 * 1024 + 1];
    let rejected = runtime.add_attachment(&project_id, "req-1", "image/png", &too_big);
    assert_err(&rejected, "add_attachment", "invalid_input");
    assert_eq!(listing(&root), before);
    assert!(!root.join(".agentup/attachments").exists());
}
