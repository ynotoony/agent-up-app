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

fn seed_project(dir: &Path) {
    fs::write(dir.join("README.md"), "# demo\n").unwrap();
    fs::write(dir.join("package.json"), "{\"name\":\"demo\"}\n").unwrap();
    fs::create_dir_all(dir.join(".git")).unwrap();
    fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
}

fn tmp_project() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    seed_project(dir.path());
    let path = dir.path().canonicalize().unwrap();
    (dir, path)
}

fn runtime_with_index() -> (TempDir, AppRuntime) {
    let app_dir = TempDir::new().unwrap();
    let runtime = AppRuntime::with_app_data_dir(app_dir.path().to_path_buf());
    (app_dir, runtime)
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
    let message = result["error"]["message"].as_str().unwrap();
    assert!(!message.is_empty());
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

fn register(runtime: &mut AppRuntime, project_id: &str, root: &Path) {
    assert_ok(
        &runtime.register_project(project_id, root.to_str().unwrap()),
        "register_project",
    );
}

#[test]
fn two_registered_projects_are_isolated() {
    let parent = TempDir::new().unwrap();
    let a = parent.path().join("alpha");
    let b = parent.path().join("beta");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    seed_project(&a);
    seed_project(&b);
    let a = a.canonicalize().unwrap();
    let b = b.canonicalize().unwrap();

    let (app_dir, mut runtime) = runtime_with_index();
    let id_a = initialize(&mut runtime, &a);
    let id_b = initialize(&mut runtime, &b);
    assert_ne!(id_a, id_b);

    let before_a = listing(&a);
    let before_b = listing(&b);
    register(&mut runtime, &id_a, &a);
    register(&mut runtime, &id_b, &b);
    assert_eq!(listing(&a), before_a, "register must not write the project disk");
    assert_eq!(listing(&b), before_b, "register must not write the project disk");

    let listed_value = runtime.list_projects();
    let listed = assert_ok(&listed_value, "list_projects");
    let projects = listed["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 2);
    let ids: BTreeSet<_> = projects
        .iter()
        .map(|item| item["project_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(id_a.as_str()));
    assert!(ids.contains(id_b.as_str()));
    assert!(projects.iter().all(|item| item["path_state"] == "ok"));

    assert_ok(
        &runtime.create_request(
            &id_a,
            "req-alpha",
            json!({"title": "Alpha", "body": "Only A"}),
            json!({}),
            0,
        ),
        "create_request",
    );
    let loaded_b_value = runtime.load_project(b.to_str().unwrap(), None);
    let loaded_b = assert_ok(&loaded_b_value, "load_project");
    assert_eq!(loaded_b["project_id"], id_b);
    assert!(loaded_b["requests"].as_array().unwrap().is_empty());

    let index = runtime.app_index_path().unwrap();
    assert!(index.starts_with(app_dir.path()));
    assert!(index.exists());
    assert!(!a.join("app-index.sqlite").exists());
    assert!(!a.join(".agentup/app-index.sqlite").exists());
}

#[test]
fn missing_path_is_reported_without_writing_project_disk() {
    let parent = TempDir::new().unwrap();
    let original = parent.path().join("live");
    fs::create_dir(&original).unwrap();
    seed_project(&original);
    let original = original.canonicalize().unwrap();

    let (app_dir, mut runtime) = runtime_with_index();
    let project_id = initialize(&mut runtime, &original);
    register(&mut runtime, &project_id, &original);

    let moved = parent.path().join("gone");
    fs::rename(&original, &moved).unwrap();
    assert!(!original.exists());
    let parent_before = listing(parent.path());
    let app_before = listing(app_dir.path());

    let listed_value = runtime.list_projects();
    let listed = assert_ok(&listed_value, "list_projects");
    let item = listed["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["project_id"] == project_id)
        .unwrap();
    assert_eq!(item["path_state"], "missing");

    assert_err(
        &runtime.scan_project(original.to_str().unwrap()),
        "scan_project",
        "path_not_found",
    );
    assert_err(
        &runtime.load_project(original.to_str().unwrap(), Some(&project_id)),
        "load_project",
        "path_not_found",
    );

    assert!(!original.exists(), "missing path must not be recreated");
    assert_eq!(listing(parent.path()), parent_before);
    assert_eq!(listing(app_dir.path()), app_before);
}

#[test]
fn rebind_restores_the_same_project_id() {
    let parent = TempDir::new().unwrap();
    let original = parent.path().join("from");
    fs::create_dir(&original).unwrap();
    seed_project(&original);
    let original = original.canonicalize().unwrap();

    let (_app_dir, mut runtime) = runtime_with_index();
    let project_id = initialize(&mut runtime, &original);
    register(&mut runtime, &project_id, &original);

    let moved = parent.path().join("to");
    fs::rename(&original, &moved).unwrap();
    let moved = moved.canonicalize().unwrap();

    let listed_value = runtime.list_projects();
    let listed = assert_ok(&listed_value, "list_projects");
    assert_eq!(listed["projects"][0]["path_state"], "missing");

    let rebound_value = runtime.rebind_project(&project_id, moved.to_str().unwrap());
    let rebound = assert_ok(&rebound_value, "rebind_project");
    assert_eq!(rebound["project_id"], project_id);
    assert_eq!(rebound["path_state"], "ok");

    let loaded_value = runtime.load_project(moved.to_str().unwrap(), None);
    let loaded = assert_ok(&loaded_value, "load_project");
    assert_eq!(loaded["project_id"], project_id);

    let listed_value = runtime.list_projects();
    let listed = assert_ok(&listed_value, "list_projects");
    assert_eq!(listed["projects"][0]["project_id"], project_id);
    assert_eq!(listed["projects"][0]["path_state"], "ok");
    assert_eq!(
        listed["projects"][0]["project_path"].as_str().unwrap(),
        moved.to_str().unwrap()
    );
}

#[test]
fn remove_agentup_backs_up_then_deletes_only_agentup() {
    let (_tmp, root) = tmp_project();
    let (_app_dir, mut runtime) = runtime_with_index();
    let project_id = initialize(&mut runtime, &root);
    register(&mut runtime, &project_id, &root);

    let before = listing(&root);
    let rejected = runtime.remove_agentup(&project_id, root.to_str().unwrap(), "");
    assert_err(&rejected, "remove_agentup", "confirmation_required");
    assert_eq!(listing(&root), before);
    assert!(root.join(".agentup/manifest.json").exists());
    assert!(root.join("README.md").exists());
    assert!(root.join(".git/HEAD").exists());

    let token = runtime
        .mint_remove_confirmation(&project_id, root.to_str().unwrap())
        .expect("remove token");
    let removed = runtime.remove_agentup(&project_id, root.to_str().unwrap(), &token);
    let data = assert_ok(&removed, "remove_agentup");
    let backup = data["backup_relative_path"].as_str().unwrap();
    assert!(backup.starts_with(".agentup.backup."));

    assert!(root.join("README.md").exists());
    assert!(root.join("package.json").exists());
    assert!(root.join(".git/HEAD").exists());
    assert!(!root.join(".agentup").exists());
    assert!(root.join(backup).join("manifest.json").exists());
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join(backup).join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["project_id"], project_id);
}
