// Input: 临时 Git 仓库、App 原生档案与隔离的 SQLite 索引。
// Output: 项目接入登记、重入、身份冲突和文件保护的回归证据。
// Pos: 接入项目集成测试；src-tauri/ 按目录登记豁免规则维护。
use agentup_harness_lib::{
    db,
    project_discovery::{confirm, confirm_and_register, discover, ImportProjectInput},
};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn git_repo() -> TempDir {
    let dir = TempDir::new().expect("temp repo");
    let status = Command::new("git")
        .args(["init", "-q", dir.path().to_str().unwrap()])
        .status()
        .expect("git init");
    assert!(status.success());
    fs::write(dir.path().join("README.md"), "sample\n").unwrap();
    let status = Command::new("git")
        .args(["-C", dir.path().to_str().unwrap(), "add", "README.md"])
        .status()
        .unwrap();
    assert!(status.success());
    let status = Command::new("git")
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "init",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    dir
}

fn input(preview: &agentup_harness_lib::project_discovery::ProjectDiscovery) -> ImportProjectInput {
    ImportProjectInput {
        path: preview.path.clone(),
        fingerprint: preview.fingerprint.clone(),
        selected_sources: vec![],
    }
}

#[test]
fn imported_project_registration_is_idempotent_by_path() {
    let repo = git_repo();
    let base = TempDir::new().unwrap();
    let state = db::open_state(base.path()).unwrap();
    let preview = discover(repo.path().to_str().unwrap()).unwrap();
    let first = confirm_and_register(&state, &input(&preview)).unwrap();
    let second = confirm_and_register(&state, &input(&preview)).unwrap();
    let conn = state.conn.lock().unwrap();
    assert_eq!(first.project.id, first.profile.id);
    assert_eq!(first.project.id, second.project.id);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let tasks: i64 = conn
        .query_row("SELECT COUNT(*) FROM requirements", [], |row| row.get(0))
        .unwrap();
    assert_eq!(tasks, 0);
}

#[test]
fn existing_native_profile_can_be_registered_without_rewriting_bytes() {
    let repo = git_repo();
    let base = TempDir::new().unwrap();
    let state = db::open_state(base.path()).unwrap();
    let preview = discover(repo.path().to_str().unwrap()).unwrap();
    let profile = confirm(&input(&preview)).unwrap();
    let metadata = repo.path().join(".agentup-app/project.json");
    let before = fs::read(&metadata).unwrap();

    // A second confirmation reads the existing native profile, then indexing it
    // must only add the SQLite row and leave the App-owned bytes untouched.
    let again = confirm_and_register(
        &state,
        &input(&discover(repo.path().to_str().unwrap()).unwrap()),
    )
    .unwrap();
    assert_eq!(profile, again.profile);
    assert_eq!(again.project.id, again.profile.id);
    assert_eq!(before, fs::read(metadata).unwrap());
}

#[test]
fn profile_id_collision_on_another_path_is_visible() {
    let first = git_repo();
    let second = git_repo();
    let base = TempDir::new().unwrap();
    let state = db::open_state(base.path()).unwrap();
    let first_path = first.path().canonicalize().unwrap();
    let second_path = second.path().canonicalize().unwrap();
    let conn = state.conn.lock().unwrap();

    db::register_imported_project(
        &conn,
        "stable-profile-id",
        "first",
        first_path.to_str().unwrap(),
    )
    .unwrap();
    let error = db::register_imported_project(
        &conn,
        "stable-profile-id",
        "second",
        second_path.to_str().unwrap(),
    )
    .unwrap_err();
    assert!(error.message.contains("其他目录"));
}

#[test]
fn existing_path_wins_over_a_different_profile_id() {
    let repo = git_repo();
    let base = TempDir::new().unwrap();
    let state = db::open_state(base.path()).unwrap();
    let path = repo.path().canonicalize().unwrap();
    let conn = state.conn.lock().unwrap();
    let first =
        db::register_imported_project(&conn, "first-profile-id", "first", path.to_str().unwrap())
            .unwrap();
    let second =
        db::register_imported_project(&conn, "second-profile-id", "second", path.to_str().unwrap())
            .unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(second.name, "first");
}

#[test]
fn confirmation_returns_existing_database_id_for_navigation() {
    let repo = git_repo();
    let base = TempDir::new().unwrap();
    let state = db::open_state(base.path()).unwrap();
    let preview = discover(repo.path().to_str().unwrap()).unwrap();
    let profile = confirm(&input(&preview)).unwrap();
    let metadata = repo.path().join(".agentup-app/project.json");
    let before = fs::read(&metadata).unwrap();
    let legacy = db::create_project_full(
        &state.conn.lock().unwrap(),
        "existing",
        None,
        Some(&preview.path),
    )
    .unwrap();
    let result = confirm_and_register(&state, &input(&preview)).unwrap();
    assert_eq!(result.project.id, legacy.id);
    assert_eq!(result.profile.id, profile.id);
    assert_ne!(result.project.id, result.profile.id);
    assert_eq!(before, fs::read(metadata).unwrap());
}

#[test]
fn profile_collision_rejected_even_when_target_path_is_registered() {
    let base = TempDir::new().unwrap();
    let state = db::open_state(base.path()).unwrap();
    let conn = state.conn.lock().unwrap();
    db::register_imported_project(&conn, "profile", "first", "/first").unwrap();
    db::register_imported_project(&conn, "second", "second", "/second").unwrap();
    let error = db::register_imported_project(&conn, "profile", "second", "/second").unwrap_err();
    assert_eq!(error.code, 409);
}
