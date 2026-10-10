// Input: 临时真实 Git 仓库、隔离任务与用户已有改动。
// Output: 隔离、完整差异、验证、接收与失败恢复的回归证据。
// Pos: 原生交付 Git 集成测试；变更同步根 README。
use agentup_harness_lib::delivery_git::{self, VerificationCommand, Worktree};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap()
}
fn ok(repo: &Path, args: &[&str]) -> String {
    let out = git(repo, args);
    assert!(
        out.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}
fn repo() -> (TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    ok(&root, &["init", "-q", "-b", "main"]);
    ok(&root, &["config", "user.name", "Test"]);
    ok(&root, &["config", "user.email", "test@example.invalid"]);
    fs::write(root.join("README.md"), "original\n").unwrap();
    fs::write(root.join("other.txt"), "other\n").unwrap();
    fs::write(root.join(".gitignore"), "node_modules/\ntarget/\n").unwrap();
    ok(&root, &["add", "."]);
    ok(&root, &["commit", "-qm", "initial"]);
    (temp, root)
}
fn worktree(root: &Path) -> Worktree {
    delivery_git::create(root, &uuid::Uuid::new_v4().to_string()).unwrap()
}
fn edit(w: &Worktree) {
    fs::write(Path::new(&w.path).join("README.md"), "implemented\n").unwrap();
}

#[test]
fn isolated_from_dirty_source_and_no_changes_rejected() {
    let (_temp, root) = repo();
    fs::write(root.join("README.md"), "user draft\n").unwrap();
    let w = worktree(&root);
    assert_eq!(
        fs::read_to_string(Path::new(&w.path).join("README.md")).unwrap(),
        "original\n"
    );
    assert!(delivery_git::diff(&w).unwrap_err().message.contains("没有"));
    assert!(delivery_git::create(&root, "../bad").is_err());
}
#[test]
fn untracked_contents_and_deletions_are_reviewed() {
    let (_temp, root) = repo();
    let w = worktree(&root);
    let p = Path::new(&w.path);
    fs::write(p.join("new.txt"), "new contents\n").unwrap();
    fs::remove_file(p.join("other.txt")).unwrap();
    let diff = delivery_git::diff(&w).unwrap();
    assert_eq!(diff.files, ["new.txt", "other.txt"]);
    assert!(diff.patch.contains("+new contents"));
    assert!(diff.patch.contains("-other"));
    fs::write(p.join("new.txt"), "changed contents\n").unwrap();
    assert!(delivery_git::accept(&w, &diff.fingerprint)
        .unwrap_err()
        .message
        .contains("变化"));
    assert_eq!(ok(&root, &["rev-parse", "HEAD"]), w.base_head);
}
#[test]
fn accept_preserves_unrelated_user_changes_and_is_idempotent() {
    let (_temp, root) = repo();
    let w = worktree(&root);
    edit(&w);
    fs::write(Path::new(&w.path).join("new.txt"), "new\n").unwrap();
    fs::write(root.join("other.txt"), "user work\n").unwrap();
    fs::create_dir(root.join(".agentup-app")).unwrap();
    fs::write(root.join(".agentup-app/project.json"), "user records").unwrap();
    fs::write(root.join("notes.txt"), "untracked user work").unwrap();
    let diff = delivery_git::diff(&w).unwrap();
    let commit = delivery_git::accept(&w, &diff.fingerprint).unwrap();
    assert_eq!(ok(&root, &["rev-parse", "HEAD"]), commit);
    assert_eq!(
        fs::read_to_string(root.join("README.md")).unwrap(),
        "implemented\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("other.txt")).unwrap(),
        "user work\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "untracked user work"
    );
    assert_eq!(
        fs::read_to_string(root.join(".agentup-app/project.json")).unwrap(),
        "user records"
    );
    assert_eq!(delivery_git::accept(&w, &diff.fingerprint).unwrap(), commit);
    delivery_git::cleanup(&w).unwrap();
    assert!(!Path::new(&w.path).exists());
}
#[test]
fn overlap_and_any_main_staging_are_refused_without_touching_user() {
    let (_temp, root) = repo();
    let w = worktree(&root);
    edit(&w);
    let diff = delivery_git::diff(&w).unwrap();
    fs::write(root.join("README.md"), "user draft").unwrap();
    assert!(delivery_git::accept(&w, &diff.fingerprint)
        .unwrap_err()
        .message
        .contains("重叠"));
    assert_eq!(ok(Path::new(&w.path), &["rev-parse", "HEAD"]), w.base_head);
    fs::write(root.join("README.md"), "original\n").unwrap();
    fs::write(root.join("other.txt"), "staged user work").unwrap();
    ok(&root, &["add", "other.txt"]);
    assert!(delivery_git::accept(&w, &diff.fingerprint)
        .unwrap_err()
        .message
        .contains("暂存"));
    assert_eq!(ok(&root, &["diff", "--cached", "--name-only"]), "other.txt");
}
#[test]
fn changing_main_head_branch_or_worktree_identity_invalidates_acceptance() {
    let (_temp, root) = repo();
    let w = worktree(&root);
    edit(&w);
    let d = delivery_git::diff(&w).unwrap();
    let mut foreign = w.clone();
    foreign.path = root.to_str().unwrap().to_string();
    assert!(delivery_git::diff(&foreign).is_err());
    ok(&root, &["commit", "--allow-empty", "-qm", "concurrent"]);
    assert!(delivery_git::accept(&w, &d.fingerprint)
        .unwrap_err()
        .message
        .contains("基线"));
    ok(Path::new(&w.path), &["add", "README.md"]);
    ok(Path::new(&w.path), &["commit", "-qm", "agent commit"]);
    assert!(delivery_git::diff(&w).is_err());
    assert!(delivery_git::accept(&w, &d.fingerprint).is_err());
}
#[test]
fn prepared_commit_is_recoverable_after_merge_interruption() {
    let (_temp, root) = repo();
    let w = worktree(&root);
    edit(&w);
    fs::write(Path::new(&w.path).join("new.txt"), "new file\n").unwrap();
    let diff = delivery_git::diff(&w).unwrap();
    let run = w.branch.strip_prefix("codex/run-").unwrap();
    let message = format!(
        "feat(agentup): complete isolated task\n\nAgentUp-Run: {run}\nAgentUp-Fingerprint: {}",
        diff.fingerprint
    );
    ok(Path::new(&w.path), &["add", "README.md", "new.txt"]);
    ok(Path::new(&w.path), &["commit", "-m", &message]);
    fs::write(root.join("README.md"), "user work").unwrap();
    assert!(delivery_git::accept(&w, &diff.fingerprint).is_err());
    fs::write(root.join("README.md"), "original\n").unwrap();
    let commit = delivery_git::accept(&w, &diff.fingerprint).unwrap();
    assert_eq!(ok(&root, &["rev-parse", "HEAD"]), commit);
}
#[test]
fn metadata_binary_large_patch_and_symlinks_are_rejected() {
    let (_temp, root) = repo();
    let w = worktree(&root);
    let p = Path::new(&w.path);
    fs::create_dir(p.join(".agentup-app")).unwrap();
    fs::write(p.join(".agentup-app/project.json"), "{}").unwrap();
    assert!(delivery_git::diff(&w)
        .unwrap_err()
        .message
        .contains("元数据"));
    fs::remove_file(p.join(".agentup-app/project.json")).unwrap();
    fs::write(p.join("binary.bin"), [0u8, 3, 7, 2]).unwrap();
    assert!(delivery_git::diff(&w)
        .unwrap_err()
        .message
        .contains("二进制"));
    fs::remove_file(p.join("binary.bin")).unwrap();
    fs::write(p.join("huge.txt"), "a\n".repeat(100_000)).unwrap();
    assert!(delivery_git::diff(&w)
        .unwrap_err()
        .message
        .contains("200KB"));
    fs::remove_file(p.join("huge.txt")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("README.md"), p.join("linked.txt")).unwrap();
        assert!(delivery_git::diff(&w)
            .unwrap_err()
            .message
            .contains("符号链接"));
    }
}
#[test]
fn local_hooks_are_not_run_and_cleanup_never_discards_work() {
    let (_temp, root) = repo();
    let w = worktree(&root);
    edit(&w);
    let hook = root.join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\nexit 99\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let d = delivery_git::diff(&w).unwrap();
    delivery_git::accept(&w, &d.fingerprint).unwrap();
    fs::write(Path::new(&w.path).join("leftover.txt"), "keep me").unwrap();
    assert!(delivery_git::cleanup(&w).is_err());
    assert!(Path::new(&w.path).join("leftover.txt").exists());
}
#[tokio::test]
async fn verification_runs_backend_allowed_command() {
    let (_temp, root) = repo();
    let w = worktree(&root);
    let results = delivery_git::run_checks(
        &w,
        &[VerificationCommand {
            program: "git".into(),
            args: vec!["diff".into(), "--check".into()],
        }],
    )
    .await;
    assert_eq!(results[0].exit_code, Some(0));
    assert!(results[0].passed);
}

#[tokio::test]
async fn verification_rejects_shell_and_arbitrary_runtime_commands() {
    let (_temp, root) = repo();
    let w = worktree(&root);
    for command in [
        VerificationCommand {
            program: "sh".into(),
            args: vec!["-c".into(), "touch pwned".into()],
        },
        VerificationCommand {
            program: "node".into(),
            args: vec!["-p".into(), "process.env".into()],
        },
        VerificationCommand {
            program: "npm".into(),
            args: vec!["run".into(), "arbitrary".into()],
        },
    ] {
        let result = delivery_git::run_checks(&w, &[command]).await;
        assert_eq!(result.len(), 1);
        assert!(!result[0].passed);
        assert!(result[0].exit_code.is_none());
    }
}

#[test]
fn receipt_requires_matching_commit_in_current_branch_history() {
    let (_temp, root) = repo();
    let w = worktree(&root);
    edit(&w);
    let diff = delivery_git::diff(&w).unwrap();
    let commit = delivery_git::accept(&w, &diff.fingerprint).unwrap();
    delivery_git::verify_receipt(&w, &commit, &diff.fingerprint).unwrap();
    assert!(delivery_git::verify_receipt(&w, &w.base_head, &diff.fingerprint).is_err());
    assert!(delivery_git::verify_receipt(&w, &commit, "different").is_err());
    ok(&root, &["checkout", "-qb", "pre-task", &w.base_head]);
    assert!(delivery_git::verify_receipt(&w, &commit, &diff.fingerprint).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn cargo_reuses_owned_sibling_cache_and_rejects_symlink_cache() {
    use std::os::unix::fs::symlink;
    let (_temp, root) = repo();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn ok() {}\n").unwrap();
    ok(&root, &["add", "Cargo.toml", "src/lib.rs"]);
    ok(&root, &["commit", "-qm", "fixture manifest"]);
    let commands = [VerificationCommand {
        program: "cargo".into(),
        args: vec!["test".into(), "--manifest-path".into(), "Cargo.toml".into()],
    }];
    let w = worktree(&root);
    let result = delivery_git::run_checks(&w, &commands).await;
    let cache = Path::new(&w.path).parent().unwrap().join(".cargo-target");
    assert!(result[0].passed, "{:?}", result[0]);
    assert!(cache.is_dir());
    assert!(!cache.starts_with(&w.root));
    assert!(!cache.starts_with(&w.path));
    let w2 = worktree(&root);
    assert!(delivery_git::run_checks(&w2, &commands).await[0].passed);
    fs::remove_dir_all(&cache).unwrap();
    symlink(&root, &cache).unwrap();
    let denied = delivery_git::run_checks(&w, &commands).await;
    assert!(!denied[0].passed);
    assert!(denied[0].output.contains("缓存"));
    assert!(denied[0].exit_code.is_none());
}
