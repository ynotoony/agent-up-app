use agentup_harness_lib::project_discovery::{confirm, discover, ImportProjectInput};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn git_repo() -> TempDir {
    let d = TempDir::new().unwrap();
    let status = Command::new("git")
        .args(["init", "-q", d.path().to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    d
}
fn commit(repo: &Path) {
    fs::write(repo.join("README.md"), "sample\n").unwrap();
    Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "add", "README.md"])
        .status()
        .unwrap();
    Command::new("git")
        .args([
            "-C",
            repo.to_str().unwrap(),
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
}
fn input(
    preview: &agentup_harness_lib::project_discovery::ProjectDiscovery,
    selected: Vec<String>,
) -> ImportProjectInput {
    ImportProjectInput {
        path: preview.path.clone(),
        fingerprint: preview.fingerprint.clone(),
        selected_sources: selected,
    }
}

#[test]
fn non_git_and_counterfeit_git_are_rejected() {
    let d = TempDir::new().unwrap();
    assert!(discover(d.path().to_str().unwrap()).is_err());
    fs::create_dir(d.path().join(".git")).unwrap();
    assert!(discover(d.path().to_str().unwrap()).is_err());
}

#[test]
fn subdirectory_requires_repository_root() {
    let d = git_repo();
    fs::create_dir(d.path().join("src")).unwrap();
    let error = discover(d.path().join("src").to_str().unwrap()).unwrap_err();
    assert!(error.message.contains("根目录"));
}

#[test]
fn empty_git_repository_is_supported_without_a_head() {
    let d = git_repo();
    let preview = discover(d.path().to_str().unwrap()).unwrap();
    assert!(preview.git.head.is_none());
    assert!(!preview.git.dirty);
    assert!(!d.path().join(".agentup-app").exists());
}

#[test]
fn linked_worktree_root_is_supported() {
    let d = git_repo();
    commit(d.path());
    let parent = TempDir::new().unwrap();
    let linked = parent.path().join("linked");
    let status = Command::new("git")
        .args([
            "-C",
            d.path().to_str().unwrap(),
            "worktree",
            "add",
            "-q",
            "-b",
            "linked",
            linked.to_str().unwrap(),
            "HEAD",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let preview = discover(linked.to_str().unwrap()).unwrap();
    assert!(preview.git.head.is_some());
}

#[test]
fn worktree_is_supported_but_symlink_sources_are_rejected() {
    let d = git_repo();
    commit(d.path());
    fs::create_dir(d.path().join("facts")).unwrap();
    std::os::unix::fs::symlink("/tmp", d.path().join("facts/requirements")).unwrap();
    assert!(discover(d.path().to_str().unwrap()).is_err());
}

#[test]
fn preview_does_not_write_and_confirm_is_idempotent() {
    let d = git_repo();
    commit(d.path());
    fs::create_dir_all(d.path().join("facts/requirements/tickets")).unwrap();
    fs::write(d.path().join("facts/requirements/tickets/index.json"), r#"{"issues":[{"id":"1","status":"ready"},{"id":"2","status":"done"},{"id":"3","status":"mystery"}]}"#).unwrap();
    let before = fs::read(d.path().join("facts/requirements/tickets/index.json")).unwrap();
    let preview = discover(d.path().to_str().unwrap()).unwrap();
    assert!(!d.path().join(".agentup-app").exists());
    let tickets = preview
        .sources
        .iter()
        .find(|s| s.path.ends_with("tickets/index.json"))
        .unwrap();
    assert_eq!(
        (
            tickets.current_count,
            tickets.history_count,
            tickets.unknown_count
        ),
        (1, 1, 1)
    );
    let first = confirm(&input(&preview, vec![tickets.path.clone()])).unwrap();
    let bytes = fs::read(d.path().join(".agentup-app/project.json")).unwrap();
    let second = confirm(&input(&preview, vec![tickets.path.clone()])).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        bytes,
        fs::read(d.path().join(".agentup-app/project.json")).unwrap()
    );
    assert_eq!(
        before,
        fs::read(d.path().join("facts/requirements/tickets/index.json")).unwrap()
    );
}

#[test]
fn stale_preview_and_tampered_selection_are_rejected_before_write() {
    let d = git_repo();
    commit(d.path());
    let preview = discover(d.path().to_str().unwrap()).unwrap();
    fs::write(d.path().join("new.txt"), "changed").unwrap();
    let mut stale = input(&preview, vec![]);
    assert!(confirm(&stale).is_err());
    assert!(!d.path().join(".agentup-app").exists());
    stale.fingerprint = discover(d.path().to_str().unwrap()).unwrap().fingerprint;
    stale.selected_sources = vec!["outside.json".into()];
    assert!(confirm(&stale).is_err());
    assert!(!d.path().join(".agentup-app").exists());
}

#[test]
fn malformed_index_warns_and_partial_metadata_never_overwrites() {
    let d = git_repo();
    commit(d.path());
    fs::create_dir_all(d.path().join("docs/issues")).unwrap();
    fs::write(d.path().join("docs/issues/index.json"), "not json").unwrap();
    let preview = discover(d.path().to_str().unwrap()).unwrap();
    assert!(preview.warnings.iter().any(|w| w.contains("无法识别")));
    fs::create_dir(d.path().join(".agentup-app")).unwrap();
    fs::write(d.path().join(".agentup-app/project.json"), "foreign").unwrap();
    assert!(discover(d.path().to_str().unwrap()).is_err());
    assert_eq!(
        fs::read_to_string(d.path().join(".agentup-app/project.json")).unwrap(),
        "foreign"
    );
}

#[test]
fn blocked_app_directory_preserves_existing_file() {
    let d = git_repo();
    commit(d.path());
    let preview = discover(d.path().to_str().unwrap()).unwrap();
    let blocker = d.path().join(".agentup-app");
    fs::write(&blocker, b"existing user bytes").unwrap();
    assert!(confirm(&input(&preview, vec![])).is_err());
    assert_eq!(fs::read(&blocker).unwrap(), b"existing user bytes");
    assert!(!blocker.join("project.json").exists());
}

#[test]
fn nested_source_content_change_stales_preview() {
    let d = git_repo();
    commit(d.path());
    let nested = d.path().join("rules/deep/policy.md");
    fs::create_dir_all(nested.parent().unwrap()).unwrap();
    fs::write(&nested, "alpha").unwrap();
    let preview = discover(d.path().to_str().unwrap()).unwrap();
    fs::write(&nested, "omega").unwrap(); // Same byte length; directory names/sizes unchanged.
    let updated = discover(d.path().to_str().unwrap()).unwrap();
    assert_ne!(preview.fingerprint, updated.fingerprint);
    assert!(confirm(&input(&preview, vec!["rules".into()])).is_err());
    assert!(!d.path().join(".agentup-app").exists());
}
