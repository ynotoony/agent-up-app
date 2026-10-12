// Input: 临时空目录、Git 仓库、真实任务索引与路径冲突。
// Output: 接入预览无写入、确认补缺和来源保留的回归证据。
// Pos: 三类仓库接入集成测试；src-tauri/ 登记豁免。
use agentup_harness_lib::workspace_onboarding::{confirm, preview, WorkspaceConfirmInput};
use std::{fs, process::Command};
use tempfile::TempDir;

fn request(path: &std::path::Path) -> WorkspaceConfirmInput {
    let state = preview(path.to_str().unwrap()).unwrap();
    WorkspaceConfirmInput {
        path: state.path,
        fingerprint: state.fingerprint,
    }
}

fn git(root: &std::path::Path) {
    assert!(Command::new("git")
        .args(["init", "-q", "--template="])
        .arg(root)
        .status()
        .unwrap()
        .success());
}

#[test]
fn empty_directory_preview_is_read_only_then_confirm_creates_empty_real_index() {
    let dir = TempDir::new().unwrap();
    let scan = preview(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(scan.mode, "new");
    assert_eq!(scan.writes.len(), 6);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    let (_, profile) = confirm(&WorkspaceConfirmInput {
        path: scan.path,
        fingerprint: scan.fingerprint,
    })
    .unwrap();
    assert!(dir.path().join(".git").is_dir());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(dir.path().join("facts/requirements/tickets/index.json")).unwrap()
        )
        .unwrap()["issues"],
        serde_json::json!([])
    );
    assert!(dir
        .path()
        .join("facts/requirements/tickets/README.md")
        .is_file());
    assert!(dir.path().join("facts/project/changes.jsonl").is_file());
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/progress-current.md")
        .exists());
    let after = preview(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(after.mode, "governed");
    assert_eq!(after.writes.len(), 1);
    assert_eq!(confirm(&request(dir.path())).unwrap().1.id, profile.id);
}

#[test]
fn existing_git_without_tickets_only_adds_missing_index_and_profile() {
    let dir = TempDir::new().unwrap();
    git(dir.path());
    fs::write(dir.path().join("README.md"), "Do not rewrite me\n").unwrap();
    let scan = preview(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(scan.mode, "git");
    assert!(!scan
        .writes
        .iter()
        .any(|write| write.action == "initialize_git"));
    confirm(&request(dir.path())).unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("README.md")).unwrap(),
        "Do not rewrite me\n"
    );
    assert!(dir
        .path()
        .join("facts/requirements/tickets/index.json")
        .is_file());
}

#[test]
fn git_repo_with_nonstandard_task_directory_stops_before_empty_index_proposal() {
    let dir = TempDir::new().unwrap();
    git(dir.path());
    let task = dir.path().join("tasks/active.md");
    fs::create_dir_all(task.parent().unwrap()).unwrap();
    fs::write(&task, "Existing work item; preserve this record.\n").unwrap();

    let error = preview(dir.path().to_str().unwrap()).unwrap_err();
    assert!(error.message.contains("tasks/active.md"));
    assert!(error.message.contains("唯一索引"));
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/index.json")
        .exists());
    assert_eq!(
        fs::read_to_string(task).unwrap(),
        "Existing work item; preserve this record.\n"
    );
}

#[test]
fn non_git_directory_with_existing_task_material_stops_without_writes() {
    let dir = TempDir::new().unwrap();
    let task = dir.path().join("tasks/active.md");
    fs::create_dir_all(task.parent().unwrap()).unwrap();
    let original = b"Existing task material; preserve every byte.\n\xff\n";
    fs::write(&task, original).unwrap();

    let error = preview(dir.path().to_str().unwrap()).unwrap_err();

    assert!(error.message.contains("tasks/active.md"));
    assert!(error.message.contains("唯一索引"));
    assert!(!dir.path().join(".git").exists());
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/index.json")
        .exists());
    assert_eq!(fs::read(task).unwrap(), original);
}

#[test]
fn git_repo_with_orphan_ticket_body_stops_without_replacing_or_shadowing_it() {
    let dir = TempDir::new().unwrap();
    git(dir.path());
    let body = dir
        .path()
        .join("facts/requirements/tickets/07-existing-task.json");
    fs::create_dir_all(body.parent().unwrap()).unwrap();
    let original = b"{\"id\":\"07-existing-task\",\"title\":\"Existing ticket\"}\n";
    fs::write(&body, original).unwrap();

    let error = preview(dir.path().to_str().unwrap()).unwrap_err();
    assert!(error
        .message
        .contains("facts/requirements/tickets/07-existing-task.json"));
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/index.json")
        .exists());
    assert_eq!(fs::read(body).unwrap(), original);
}

#[test]
fn git_repo_with_governance_ledger_but_no_index_fails_closed() {
    let dir = TempDir::new().unwrap();
    git(dir.path());
    let ledger = dir.path().join("facts/project/changes.jsonl");
    fs::create_dir_all(ledger.parent().unwrap()).unwrap();
    let original = b"{\"date\":\"2026-10-12\",\"kind\":\"record\"}\n";
    fs::write(&ledger, original).unwrap();

    let error = preview(dir.path().to_str().unwrap()).unwrap_err();
    assert!(error.message.contains("facts/project/changes.jsonl"));
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/index.json")
        .exists());
    assert_eq!(fs::read(ledger).unwrap(), original);
}

#[test]
fn generic_project_documentation_is_not_misclassified_as_task_management() {
    let dir = TempDir::new().unwrap();
    git(dir.path());
    fs::create_dir_all(dir.path().join("docs/architecture")).unwrap();
    fs::write(
        dir.path().join("docs/architecture/overview.md"),
        "System overview.\n",
    )
    .unwrap();

    let scan = preview(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(scan.mode, "git");
    assert!(scan
        .writes
        .iter()
        .any(|write| write.path == "facts/requirements/tickets/index.json"));
}

#[test]
fn non_git_directory_with_real_tickets_preserves_source_and_registers_empty_or_real_board() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("docs/issues")).unwrap();
    let original =
        "{\"issues\":[{\"id\":\"real-task\",\"status\":\"blocked\",\"blocked_by\":[]}]}\n";
    fs::write(dir.path().join("docs/issues/index.json"), original).unwrap();
    let (path, profile) = confirm(&request(dir.path())).unwrap();
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/index.json")
        .exists());
    let data = TempDir::new().unwrap();
    let state = agentup_harness_lib::db::open_state(data.path()).unwrap();
    let connection = state.conn.lock().unwrap();
    let project = agentup_harness_lib::db::register_imported_project(
        &connection,
        &profile.id,
        &profile.name,
        &path,
    )
    .unwrap();
    let board =
        agentup_harness_lib::ticket_source::load_governance_tickets(&connection, &project.id, true)
            .unwrap();
    assert_eq!(board["total"], 1);
    assert_eq!(board["tickets"][0]["id"], "real-task");
    assert_eq!(
        fs::read_to_string(dir.path().join("docs/issues/index.json")).unwrap(),
        original
    );
}

#[test]
fn bare_repository_is_not_reinitialized() {
    let dir = TempDir::new().unwrap();
    assert!(Command::new("git")
        .args(["init", "--bare", "-q", "--template="])
        .arg(dir.path())
        .status()
        .unwrap()
        .success());
    assert!(preview(dir.path().to_str().unwrap()).is_err());
    assert!(!dir.path().join(".git").exists());
    assert!(!dir.path().join("facts").exists());
}

#[test]
fn governed_legacy_source_is_preserved_without_shadow_index() {
    let dir = TempDir::new().unwrap();
    git(dir.path());
    fs::create_dir_all(dir.path().join("docs/issues")).unwrap();
    let original = "{\"issues\":[{\"id\":\"real-task\",\"status\":\"ready\"}]}\n";
    fs::write(dir.path().join("docs/issues/index.json"), original).unwrap();
    let scan = preview(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(scan.mode, "governed");
    assert_eq!(scan.index_path.as_deref(), Some("docs/issues/index.json"));
    confirm(&request(dir.path())).unwrap();
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/index.json")
        .exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("docs/issues/index.json")).unwrap(),
        original
    );
}

#[test]
fn stale_preview_and_broken_existing_index_fail_before_writes() {
    let dir = TempDir::new().unwrap();
    let input = request(dir.path());
    fs::write(dir.path().join("new.txt"), "new").unwrap();
    assert!(confirm(&input).is_err());
    assert!(!dir.path().join(".git").exists());
    fs::create_dir_all(dir.path().join("docs/issues")).unwrap();
    fs::write(dir.path().join("docs/issues/index.json"), "broken").unwrap();
    assert!(preview(dir.path().to_str().unwrap()).is_err());
    assert!(!dir.path().join("facts").exists());
}

#[test]
fn non_git_governance_ledger_without_index_fails_closed_and_preserves_bytes() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("facts/project")).unwrap();
    let ledger = b"{\"date\":\"2026-10-12\",\"kind\":\"record\",\"scope\":\"prior\",\"decision\":\"keep exactly\",\"evidence_ref\":\"existing\"}\n";
    fs::write(dir.path().join("facts/project/changes.jsonl"), ledger).unwrap();

    let error = preview(dir.path().to_str().unwrap()).unwrap_err();

    assert!(error.message.contains("facts/project/changes.jsonl"));
    assert!(!dir.path().join(".git").exists());
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/index.json")
        .exists());
    assert_eq!(
        fs::read(dir.path().join("facts/project/changes.jsonl")).unwrap(),
        ledger
    );
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/progress-current.md")
        .exists());
}

#[test]
fn malformed_existing_ticket_readme_fails_preview_without_writes() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("facts/requirements/tickets")).unwrap();
    fs::write(
        dir.path().join("facts/requirements/tickets/README.md"),
        "user text without a directory table\n",
    )
    .unwrap();

    assert!(preview(dir.path().to_str().unwrap()).is_err());
    assert!(!dir.path().join(".git").exists());
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/index.json")
        .exists());
}

#[test]
fn repository_subdirectory_does_not_create_nested_git() {
    let dir = TempDir::new().unwrap();
    git(dir.path());
    fs::create_dir(dir.path().join("child")).unwrap();
    assert!(preview(dir.path().join("child").to_str().unwrap()).is_err());
    assert!(!dir.path().join("child/.git").exists());
}

#[cfg(unix)]
#[test]
fn symlinked_source_parents_are_rejected_without_writes() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("facts")).unwrap();
    assert!(preview(dir.path().to_str().unwrap()).is_err());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    assert!(!dir.path().join(".git").exists());
}

#[cfg(unix)]
#[test]
fn symlinked_ticket_support_files_are_rejected_without_following_or_overwriting() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("facts/requirements/tickets")).unwrap();
    fs::create_dir_all(outside.path().join("facts/project")).unwrap();
    let sentinel = outside.path().join("facts/project/changes.jsonl");
    fs::write(&sentinel, b"keep\n").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("facts/project"),
        dir.path().join("facts/project"),
    )
    .unwrap();

    assert!(preview(dir.path().to_str().unwrap()).is_err());
    assert_eq!(fs::read(sentinel).unwrap(), b"keep\n");
    assert!(!dir
        .path()
        .join("facts/requirements/tickets/index.json")
        .exists());
    assert!(!dir.path().join(".git").exists());
}
