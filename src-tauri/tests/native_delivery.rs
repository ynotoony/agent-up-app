// Input: Temporary Git projects, approved native goals and explicit delivery evidence fixtures.
// Output: Guard, interruption, acceptance and evidence identity regression coverage without real Agent cost.
// Pos: Native delivery contract tests; fixture review evidence is test-only, never a production fallback.
use agentup_harness_lib::{
    delivery_git::{self, CheckResult, VerificationCommand},
    native_delivery::{self, NativeDelivery, ReviewEvidence},
    native_goals::{GoalStore, NativeGoal, NativeTask},
    project_discovery::{confirm, discover, ImportProjectInput},
};
use std::{fs, process::Command};
use tempfile::TempDir;

fn git(root: &std::path::Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
fn setup() -> (TempDir, NativeGoal) {
    let tmp = TempDir::new().unwrap();
    git(tmp.path(), &["init", "-q"]);
    git(tmp.path(), &["config", "user.name", "Delivery test"]);
    git(
        tmp.path(),
        &["config", "user.email", "delivery@example.invalid"],
    );
    fs::write(tmp.path().join("README.md"), "before\n").unwrap();
    fs::write(tmp.path().join(".gitignore"), ".agentup-app/\n").unwrap();
    git(tmp.path(), &["add", "README.md", ".gitignore"]);
    git(tmp.path(), &["commit", "-qm", "baseline"]);
    let preview = discover(tmp.path().to_str().unwrap()).unwrap();
    confirm(&ImportProjectInput {
        path: preview.path,
        fingerprint: preview.fingerprint,
        selected_sources: vec![],
    })
    .unwrap();
    let store = GoalStore::open(tmp.path().to_str().unwrap()).unwrap();
    let mut goal = store.create("更新项目说明文档").unwrap();
    goal.status = "awaiting_confirmation".into();
    let task = NativeTask {
        id: "task-1".into(),
        title: "完善项目说明".into(),
        description: "更新 README 中的说明".into(),
        acceptance: vec!["包含 after".into()],
        depends_on: vec![],
        capabilities: vec!["文档".into()],
    };
    goal.tasks = vec![task.clone()];
    fs::write(
        tmp.path()
            .join(format!(".agentup-app/goals/{}.json", goal.id)),
        serde_json::to_vec_pretty(&goal).unwrap(),
    )
    .unwrap();
    let goal = store.confirm(&goal.id, goal.revision, vec![task]).unwrap();
    (tmp, goal)
}
fn evidence(tmp: &TempDir, goal: &NativeGoal) -> NativeDelivery {
    let id = uuid::Uuid::new_v4().to_string();
    let w = delivery_git::create(tmp.path(), &id).unwrap();
    fs::write(std::path::Path::new(&w.path).join("README.md"), "after\n").unwrap();
    let diff = delivery_git::diff(&w).unwrap();
    let command = VerificationCommand {
        program: "git".into(),
        args: vec!["diff".into(), "--check".into()],
    };
    NativeDelivery {
        schema_version: 1,
        id,
        project_id: goal.project_id.clone(),
        goal_id: goal.id.clone(),
        goal_revision: goal.revision,
        task_id: goal.tasks[0].id.clone(),
        task_title: goal.tasks[0].title.clone(),
        task_snapshot: goal.tasks[0].clone(),
        goal_content: goal.content.clone(),
        status: "awaiting_acceptance".into(),
        started_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        runtime_id: "codex-cli".into(),
        worktree: Some(w),
        verified_fingerprint: Some(diff.fingerprint.clone()),
        diff: Some(diff),
        checks: vec![CheckResult {
            program: command.program.clone(),
            args: command.args.clone(),
            exit_code: Some(0),
            passed: true,
            output: "test fixture".into(),
        }],
        commands: vec![command],
        review: Some(ReviewEvidence {
            passed: true,
            summary: "Test-only review evidence fixture".into(),
            findings: vec![],
        }),
        implementation_summary: Some("Test fixture".into()),
        error: None,
        commit: None,
        owner_pid: None,
    }
}
fn save(tmp: &TempDir, run: &NativeDelivery) {
    let dir = tmp.path().join(".agentup-app/deliveries");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("{}.json", run.id)),
        serde_json::to_vec_pretty(run).unwrap(),
    )
    .unwrap();
}
fn cleanup(run: &NativeDelivery) {
    if let Some(w) = &run.worktree {
        let _ = delivery_git::cleanup(w);
        let _ = fs::remove_dir(std::path::Path::new(&w.path).parent().unwrap());
    }
}
#[test]
fn strict_review_requires_consistent_complete_result() {
    assert!(
        native_delivery::parse_review(r#"{"passed":true,"summary":"通过","findings":[]}"#)
            .unwrap()
            .passed
    );
    assert!(!native_delivery::parse_review("```json\n{\"passed\":false,\"summary\":\"失败\",\"findings\":[\"missing validation\"]}\n```").unwrap().passed);
    for text in [
        r#"{"passed":true,"summary":"OK","findings":["bug"]}"#,
        r#"{"passed":true,"summary":"","findings":[]}"#,
        r#"{"passed":true,"summary":"OK","findings":[],"unexpected":1}"#,
        "OK",
    ] {
        assert!(native_delivery::parse_review(text).is_err());
    }
}
#[tokio::test]
async fn execution_requires_current_confirmed_plan_and_nonempty_checks() {
    let (tmp, goal) = setup();
    let path = tmp.path().to_str().unwrap();
    assert!(
        native_delivery::execute(path, &goal.id, "task-1", goal.revision, vec![], None)
            .await
            .is_err()
    );
    assert!(native_delivery::execute(
        path,
        &goal.id,
        "task-1",
        goal.revision + 1,
        vec![VerificationCommand {
            program: "git".into(),
            args: vec!["diff".into(), "--check".into()]
        }],
        None
    )
    .await
    .is_err());
    assert!(native_delivery::list(path, &goal.id).unwrap().is_empty());
}

#[tokio::test]
async fn missing_runtime_fails_closed_without_delivery_evidence() {
    // The native path must persist the failed attempt so a restart can explain it,
    // while never claiming that implementation, verification, or review happened.
    let (tmp, goal) = setup();
    let previous_fake_runtime = std::env::var_os("AGENTUP_TEST_FAKE_RUNTIME");
    std::env::set_var("AGENTUP_TEST_FAKE_RUNTIME", "1");
    let result = native_delivery::execute(
        tmp.path().to_str().unwrap(),
        &goal.id,
        "task-1",
        goal.revision,
        vec![VerificationCommand {
            program: "git".into(),
            args: vec!["diff".into(), "--check".into()],
        }],
        None,
    )
    .await
    .expect("failed runtime is represented as a persisted run, not an API error");
    match previous_fake_runtime {
        Some(value) => std::env::set_var("AGENTUP_TEST_FAKE_RUNTIME", value),
        None => std::env::remove_var("AGENTUP_TEST_FAKE_RUNTIME"),
    }

    assert_eq!(result.status, "failed");
    assert!(result
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("不会使用模拟结果"));
    assert!(
        result.worktree.is_none(),
        "runtime gate runs before creating a worktree"
    );
    assert!(result.diff.is_none());
    assert!(result.checks.is_empty());
    assert!(result.review.is_none());
    assert!(result.commit.is_none());
    let persisted = native_delivery::list(tmp.path().to_str().unwrap(), &goal.id).unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].status, "failed");
}

#[test]
fn imported_goal_can_be_rejected_then_a_separate_run_accepted_and_archived() {
    // setup() performs discover -> confirm -> create goal -> confirm plan. Keep the
    // rest of the test at the public delivery boundary, where user acceptance occurs.
    let (tmp, goal) = setup();
    assert!(tmp.path().join(".agentup-app/project.json").is_file());
    assert_eq!(goal.status, "ready");
    assert_eq!(goal.tasks.len(), 1);

    let rejected = evidence(&tmp, &goal);
    save(&tmp, &rejected);
    let rejected = native_delivery::reject(tmp.path().to_str().unwrap(), &rejected.id).unwrap();
    assert_eq!(rejected.status, "failed");
    assert!(rejected.commit.is_none());
    assert!(rejected.worktree.is_some());

    // A rejection leaves the isolated result recoverable, but releases the project
    // lock so a new execution can be reviewed and accepted independently.
    let accepted_run = evidence(&tmp, &goal);
    let fingerprint = accepted_run.diff.as_ref().unwrap().fingerprint.clone();
    save(&tmp, &accepted_run);
    let accepted =
        native_delivery::accept(tmp.path().to_str().unwrap(), &accepted_run.id, &fingerprint)
            .unwrap();
    assert_eq!(accepted.status, "accepted");
    assert!(accepted.commit.as_ref().is_some_and(|sha| sha.len() == 40));
    let completed = GoalStore::open(tmp.path().to_str().unwrap())
        .unwrap()
        .get(&goal.id)
        .unwrap();
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.revision, goal.revision + 1);
    let mut stale = completed.clone();
    stale.status = "ready".into();
    stale.revision -= 1;
    fs::write(
        tmp.path()
            .join(format!(".agentup-app/goals/{}.json", stale.id)),
        serde_json::to_vec_pretty(&stale).unwrap(),
    )
    .unwrap();
    native_delivery::reconcile_goal_completion(tmp.path().to_str().unwrap()).unwrap();
    let reconciled = GoalStore::open(tmp.path().to_str().unwrap())
        .unwrap()
        .get(&goal.id)
        .unwrap();
    assert_eq!(reconciled.status, "completed");
    assert_eq!(reconciled.revision, completed.revision);
    assert_eq!(
        fs::read_to_string(tmp.path().join("README.md")).unwrap(),
        "after\n"
    );
    assert!(tmp
        .path()
        .join(format!(".agentup-app/deliveries/{}.json", accepted.id))
        .is_file());
    assert!(!std::path::Path::new(&accepted_run.worktree.as_ref().unwrap().path).exists());

    cleanup(&rejected);
}
#[test]
fn pending_execution_recovers_after_restart_and_retains_worktree() {
    let (tmp, goal) = setup();
    let mut run = evidence(&tmp, &goal);
    run.status = "verifying".into();
    save(&tmp, &run);
    let loaded = native_delivery::list(tmp.path().to_str().unwrap(), &goal.id).unwrap();
    assert_eq!(loaded[0].status, "failed");
    assert!(loaded[0].error.as_ref().unwrap().contains("中断"));
    assert_eq!(
        loaded[0].worktree.as_ref().unwrap().path,
        run.worktree.as_ref().unwrap().path
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("README.md")).unwrap(),
        "before\n"
    );
    cleanup(&run);
}

#[test]
fn forged_accepted_receipt_is_downgraded_to_failed_on_reload() {
    let (tmp, goal) = setup();
    let mut run = evidence(&tmp, &goal);
    run.status = "accepted".into();
    run.commit = Some("a".repeat(40));
    save(&tmp, &run);
    let loaded = native_delivery::list(tmp.path().to_str().unwrap(), &goal.id).unwrap();
    assert_eq!(loaded[0].status, "failed");
    assert!(loaded[0].error.as_deref().unwrap().contains("可验证提交"));
    cleanup(&run);
}

#[test]
fn delivery_snapshot_mismatch_is_rejected_before_acceptance() {
    let (tmp, goal) = setup();
    let run = evidence(&tmp, &goal);
    save(&tmp, &run);
    let path = tmp
        .path()
        .join(format!(".agentup-app/deliveries/{}.json", run.id));
    let mut tampered = run.clone();
    tampered.task_title = "伪造任务".into();
    fs::write(&path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
    assert!(native_delivery::list(tmp.path().to_str().unwrap(), &goal.id).is_err());
    cleanup(&run);
}

#[test]
fn acceptance_rejects_stale_review_content_then_reject_releases_pending() {
    let (tmp, goal) = setup();
    let run = evidence(&tmp, &goal);
    save(&tmp, &run);
    let w = run.worktree.as_ref().unwrap();
    fs::write(
        std::path::Path::new(&w.path).join("README.md"),
        "unexpected drift\n",
    )
    .unwrap();
    let accepted = native_delivery::accept(
        tmp.path().to_str().unwrap(),
        &run.id,
        &run.diff.as_ref().unwrap().fingerprint,
    )
    .unwrap();
    assert_eq!(accepted.status, "awaiting_acceptance");
    assert!(accepted.commit.is_none());
    assert!(accepted.error.is_some());
    assert_eq!(
        fs::read_to_string(tmp.path().join("README.md")).unwrap(),
        "before\n"
    );
    let rejected = native_delivery::reject(tmp.path().to_str().unwrap(), &run.id).unwrap();
    assert_eq!(rejected.status, "failed");
    assert!(std::path::Path::new(&w.path).exists());
    cleanup(&run);
}
#[test]
fn acceptance_creates_real_local_commit_and_preserves_unrelated_edits() {
    let (tmp, goal) = setup();
    let run = evidence(&tmp, &goal);
    save(&tmp, &run);
    fs::write(tmp.path().join("personal.txt"), "do not touch").unwrap();
    let accepted = native_delivery::accept(
        tmp.path().to_str().unwrap(),
        &run.id,
        &run.diff.as_ref().unwrap().fingerprint,
    )
    .unwrap();
    assert_eq!(accepted.status, "accepted", "{:?}", accepted.error);
    assert_eq!(
        accepted.commit.as_ref().unwrap(),
        &git(tmp.path(), &["rev-parse", "HEAD^"])
    );
    assert_eq!(
        git(tmp.path(), &["log", "-1", "--format=%s"]),
        "chore(agentup): record accepted delivery"
    );
    assert!(!git(tmp.path(), &["ls-files", ".agentup-app"]).is_empty());
    assert_eq!(
        fs::read_to_string(tmp.path().join("README.md")).unwrap(),
        "after\n"
    );
    assert!(!std::path::Path::new(&run.worktree.as_ref().unwrap().path).exists());
    assert_eq!(
        fs::read_to_string(tmp.path().join("personal.txt")).unwrap(),
        "do not touch"
    );
    assert!(native_delivery::reject(tmp.path().to_str().unwrap(), &run.id).is_err());
    let again = native_delivery::accept(
        tmp.path().to_str().unwrap(),
        &run.id,
        &run.diff.as_ref().unwrap().fingerprint,
    )
    .unwrap();
    assert_eq!(accepted.commit, again.commit);
    cleanup(&run);
}
#[test]
fn confirmation_change_invalidates_old_evidence() {
    let (tmp, mut goal) = setup();
    let run = evidence(&tmp, &goal);
    save(&tmp, &run);
    goal.revision += 1;
    fs::write(
        tmp.path()
            .join(format!(".agentup-app/goals/{}.json", goal.id)),
        serde_json::to_vec_pretty(&goal).unwrap(),
    )
    .unwrap();
    assert!(native_delivery::accept(
        tmp.path().to_str().unwrap(),
        &run.id,
        &run.diff.as_ref().unwrap().fingerprint
    )
    .is_err());
    assert_eq!(
        fs::read_to_string(tmp.path().join("README.md")).unwrap(),
        "before\n"
    );
    cleanup(&run);
}
#[test]
fn missing_check_evidence_and_symlink_records_are_rejected() {
    let (tmp, goal) = setup();
    let mut run = evidence(&tmp, &goal);
    run.checks.clear();
    save(&tmp, &run);
    assert!(native_delivery::list(tmp.path().to_str().unwrap(), &goal.id).is_err());
    #[cfg(unix)]
    {
        let path = tmp
            .path()
            .join(format!(".agentup-app/deliveries/{}.json", run.id));
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(tmp.path().join("README.md"), &path).unwrap();
        assert!(native_delivery::list(tmp.path().to_str().unwrap(), &goal.id).is_err());
    }
    cleanup(&run);
}
#[tokio::test]
async fn pending_acceptance_blocks_other_execution_and_confirmation() {
    let (tmp, goal) = setup();
    let run = evidence(&tmp, &goal);
    save(&tmp, &run);
    let path = tmp.path().to_str().unwrap();
    let error = native_delivery::execute(
        path,
        &goal.id,
        "task-1",
        goal.revision,
        run.commands.clone(),
        None,
    )
    .await
    .unwrap_err();
    assert!(error.message.contains("已有"));
    assert!(GoalStore::open(path)
        .unwrap()
        .confirm(&goal.id, goal.revision, goal.tasks.clone())
        .unwrap_err()
        .message
        .contains("待验收"));
    cleanup(&run);
}
#[tokio::test]
async fn old_revision_or_missing_commit_cannot_satisfy_dependencies() {
    let (tmp, mut goal) = setup();
    let mut run = evidence(&tmp, &goal);
    run.status = "accepted".into();
    run.commit = Some("a".repeat(40));
    save(&tmp, &run);
    let mut next = goal.tasks[0].clone();
    next.id = "task-2".into();
    next.depends_on = vec!["task-1".into()];
    goal.tasks.push(next);
    let goal_path = tmp
        .path()
        .join(format!(".agentup-app/goals/{}.json", goal.id));
    fs::write(&goal_path, serde_json::to_vec_pretty(&goal).unwrap()).unwrap();
    let path = tmp.path().to_str().unwrap();
    assert!(native_delivery::execute(
        path,
        &goal.id,
        "task-2",
        goal.revision,
        run.commands.clone(),
        None
    )
    .await
    .unwrap_err()
    .message
    .contains("前置任务"));
    goal.revision += 1;
    fs::write(&goal_path, serde_json::to_vec_pretty(&goal).unwrap()).unwrap();
    assert!(native_delivery::execute(
        path,
        &goal.id,
        "task-2",
        goal.revision,
        run.commands.clone(),
        None
    )
    .await
    .unwrap_err()
    .message
    .contains("前置任务"));
    assert!(
        native_delivery::accept(path, &run.id, &run.diff.as_ref().unwrap().fingerprint).is_err()
    );
    cleanup(&run);
}
#[test]
fn generic_project_has_no_invented_checks_and_corruption_is_not_rewritten() {
    let (tmp, goal) = setup();
    assert!(native_delivery::options(tmp.path().to_str().unwrap())
        .unwrap()
        .commands
        .is_empty());
    let dir = tmp.path().join(".agentup-app/deliveries");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}.json", uuid::Uuid::new_v4()));
    fs::write(&path, "broken").unwrap();
    assert!(native_delivery::list(tmp.path().to_str().unwrap(), &goal.id).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "broken");
}
#[test]
fn metadata_archive_failure_preserves_accepted_code_and_retry_is_idempotent() {
    let (tmp, goal) = setup();
    let run = evidence(&tmp, &goal);
    save(&tmp, &run);
    let path = tmp.path().to_str().unwrap();
    let fingerprint = &run.diff.as_ref().unwrap().fingerprint;
    let accepted = native_delivery::accept(path, &run.id, fingerprint).unwrap();
    assert_eq!(accepted.status, "accepted");
    assert!(accepted.error.is_none());
    let accepted_head = git(tmp.path(), &["rev-parse", "HEAD"]);
    fs::write(tmp.path().join("personal.txt"), "owned by user").unwrap();
    git(tmp.path(), &["add", "personal.txt"]);
    let blocked = native_delivery::accept(path, &run.id, fingerprint).unwrap();
    assert_eq!(blocked.status, "accepted");
    assert_eq!(blocked.commit, accepted.commit);
    assert!(blocked.error.unwrap().contains("记录尚未进入 Git"));
    assert_eq!(
        git(tmp.path(), &["diff", "--cached", "--name-only"]),
        "personal.txt"
    );
    assert_eq!(git(tmp.path(), &["rev-parse", "HEAD"]), accepted_head);
    git(tmp.path(), &["reset", "-q", "--", "personal.txt"]);
    let retried = native_delivery::accept(path, &run.id, fingerprint).unwrap();
    assert!(retried.error.is_none());
    assert_eq!(retried.commit, accepted.commit);
    assert_eq!(git(tmp.path(), &["rev-parse", "HEAD"]), accepted_head);
    cleanup(&run);
}
#[test]
fn accepted_delivery_retains_dirty_worktree_then_retries_cleanup_without_new_code_commit() {
    let (tmp, goal) = setup();
    let mut run = evidence(&tmp, &goal);
    let w = run.worktree.as_ref().unwrap().clone();
    let fingerprint = run.diff.as_ref().unwrap().fingerprint.clone();
    // Simulate a crash between code acceptance and receipt/archive/cleanup, then an external edit.
    run.commit = Some(delivery_git::accept(&w, &fingerprint).unwrap());
    run.status = "accepted".into();
    save(&tmp, &run);
    let user_file = std::path::Path::new(&w.path).join("still-working.txt");
    fs::write(&user_file, "keep my work").unwrap();
    let accepted =
        native_delivery::accept(tmp.path().to_str().unwrap(), &run.id, &fingerprint).unwrap();
    assert_eq!(accepted.status, "accepted");
    assert_eq!(accepted.commit, run.commit);
    assert!(accepted
        .error
        .as_ref()
        .unwrap()
        .contains("隔离目录尚未清理"));
    assert_eq!(fs::read_to_string(&user_file).unwrap(), "keep my work");
    assert!(git(
        tmp.path(),
        &[
            "ls-files",
            &format!(".agentup-app/deliveries/{}.json", run.id)
        ]
    )
    .contains(&run.id));
    fs::remove_file(user_file).unwrap();
    let retried =
        native_delivery::accept(tmp.path().to_str().unwrap(), &run.id, &fingerprint).unwrap();
    assert_eq!(retried.status, "accepted");
    assert_eq!(retried.commit, run.commit);
    assert!(retried.error.is_none());
    assert!(!std::path::Path::new(&w.path).exists());
    cleanup(&run);
}
#[cfg(unix)]
#[tokio::test]
async fn live_external_delivery_owner_survives_polling_and_blocks_second_execution() {
    let (tmp, goal) = setup();
    let mut run = evidence(&tmp, &goal);
    let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
    run.status = "running".into();
    run.owner_pid = Some(child.id());
    save(&tmp, &run);
    let path = tmp.path().to_str().unwrap();
    assert_eq!(
        native_delivery::list(path, &goal.id).unwrap()[0].status,
        "running"
    );
    assert!(native_delivery::execute(
        path,
        &goal.id,
        "task-1",
        goal.revision,
        run.commands.clone(),
        None
    )
    .await
    .unwrap_err()
    .message
    .contains("已有"));
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(
        native_delivery::list(path, &goal.id).unwrap()[0].status,
        "failed"
    );
    cleanup(&run);
}
