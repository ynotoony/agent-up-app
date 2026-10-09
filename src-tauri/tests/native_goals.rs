// Input: Temporary Git repositories and native goal records.
// Output: Storage, plan validation and isolation regression evidence.
// Pos: Native goal integration tests; no real agents or production substitutes.
use agentup_harness_lib::{
    native_goals::{parse_plan, validate_tasks, GoalStore, NativeTask},
    project_discovery::{confirm, discover, ImportProjectInput},
};
use std::{fs, process::Command};
use tempfile::TempDir;

fn project() -> TempDir {
    let temp = TempDir::new().unwrap();
    assert!(Command::new("git")
        .args(["init", "-q"])
        .arg(temp.path())
        .status()
        .unwrap()
        .success());
    fs::write(temp.path().join("README.md"), "Original project\n").unwrap();
    let preview = discover(temp.path().to_str().unwrap()).unwrap();
    confirm(&ImportProjectInput {
        path: preview.path,
        fingerprint: preview.fingerprint,
        selected_sources: vec!["README.md".into()],
    })
    .unwrap();
    temp
}
fn task(id: &str, deps: &[&str]) -> NativeTask {
    NativeTask {
        id: id.into(),
        title: "可用入口".into(),
        description: "用户保存一句话目标".into(),
        acceptance: vec!["重启后能看到目标".into()],
        depends_on: deps.iter().map(|s| s.to_string()).collect(),
        capabilities: vec!["编码".into()],
    }
}
#[test]
fn goal_survives_reload_without_modifying_sources_or_profile() {
    let project = project();
    let before = fs::read(project.path().join(".agentup-app/project.json")).unwrap();
    let store = GoalStore::open(project.path().to_str().unwrap()).unwrap();
    assert!(store.list().unwrap().is_empty());
    assert!(!project.path().join(".agentup-app/goals").exists());
    assert!(store.create("三个字").is_err());
    assert!(store.create(&"好".repeat(12001)).is_err());
    let goal = store.create("创建目标入口").unwrap();
    drop(store);
    let store = GoalStore::open(project.path().to_str().unwrap()).unwrap();
    let loaded = store.get(&goal.id).unwrap();
    assert_eq!(loaded.content, "创建目标入口");
    assert_eq!(loaded.status, "draft");
    assert_eq!(loaded.revision, 1);
    assert!(loaded.run.is_none());
    assert!(loaded.tasks.is_empty());
    assert_eq!(store.list().unwrap().len(), 1);
    assert_eq!(
        fs::read_to_string(project.path().join("README.md")).unwrap(),
        "Original project\n"
    );
    assert_eq!(
        fs::read(project.path().join(".agentup-app/project.json")).unwrap(),
        before
    );
}
#[test]
fn invalid_dependency_graphs_and_ids_are_rejected() {
    assert!(validate_tasks(&[task("first", &[]), task("second", &["first"])]).is_ok());
    for tasks in [
        vec![task("same", &[]), task("same", &[])],
        vec![task("first", &["missing"])],
        vec![task("self", &["self"])],
        vec![task("one", &["two"]), task("two", &["one"])],
        vec![task("../../outside", &[])],
        vec![task("first", &[]), task("second", &["first", "first"])],
    ] {
        assert!(validate_tasks(&tasks).is_err());
    }
    let mut empty_acceptance = task("one", &[]);
    empty_acceptance.acceptance.clear();
    assert!(validate_tasks(&[empty_acceptance]).is_err());
}
#[test]
fn plan_json_is_strict_and_supports_questions_without_inventing_tasks() {
    let json = r#"{"summary":"需要确认范围","questions":["是否需要离线使用？"],"tasks":[]}"#;
    assert_eq!(parse_plan(json).unwrap().questions.len(), 1);
    assert!(parse_plan(&format!("```json\n{json}\n```")).is_ok());
    assert!(parse_plan("result: {} trailing text").is_err());
    assert!(parse_plan(r#"{"summary":"empty","questions":[],"tasks":[]}"#).is_err());
    assert!(parse_plan(r#"{"summary":"x","questions":[],"tasks":[],"success":true}"#).is_err());
    assert!(parse_plan("```json\n{}\n").is_err());
}
#[test]
fn path_traversal_and_malformed_or_large_records_are_rejected_without_overwrite() {
    let project = project();
    let store = GoalStore::open(project.path().to_str().unwrap()).unwrap();
    assert!(store.get("../../../README").is_err());
    let goal = store.create("创建目标入口").unwrap();
    let path = project
        .path()
        .join(format!(".agentup-app/goals/{}.json", goal.id));
    fs::write(&path, b"broken record").unwrap();
    assert!(store.get(&goal.id).is_err());
    assert!(store.list().is_err());
    assert_eq!(fs::read(&path).unwrap(), b"broken record");
    fs::write(&path, vec![b'x'; 512 * 1024 + 1]).unwrap();
    assert!(store.get(&goal.id).is_err());
}
#[cfg(unix)]
#[test]
fn symlinked_owned_directories_and_goal_files_are_rejected() {
    use std::os::unix::fs::symlink;
    let project = project();
    let outside = TempDir::new().unwrap();
    let store = GoalStore::open(project.path().to_str().unwrap()).unwrap();
    let directory = project.path().join(".agentup-app/goals");
    symlink(outside.path(), &directory).unwrap();
    assert!(store.create("创建目标入口").is_err());
    assert!(store.list().is_err());
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
    fs::remove_file(&directory).unwrap();
    let goal = store.create("创建目标入口").unwrap();
    let path = directory.join(format!("{}.json", goal.id));
    fs::rename(&path, outside.path().join("goal.json")).unwrap();
    symlink(outside.path().join("goal.json"), &path).unwrap();
    assert!(store.get(&goal.id).is_err());
    assert!(store
        .confirm(&goal.id, goal.revision, vec![task("one", &[])])
        .is_err());
}
