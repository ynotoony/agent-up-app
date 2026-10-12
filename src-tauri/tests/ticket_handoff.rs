// Input: Temporary projects, ready/blocked tickets and an injected terminal process boundary.
// Output: Verified interactive argv, exact cwd/context, injection protection and honest launch failures.
// Pos: Agent handoff regression suite; never calls a real Agent or opens the user's terminal.
use agentup_harness_lib::{error::ApiError, ticket_handoff};
use serde_json::{json, Value};
use std::{fs, path::Path};
use tempfile::TempDir;

fn fixture() -> (TempDir, Value) {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("docs/issues")).unwrap();
    fs::write(
        root.path().join("docs/issues/index.json"),
        r#"{"issues":[{"id":"one","status":"ready"}]}"#,
    )
    .unwrap();
    let body = json!({"id":"one","title":"真实任务","goal":"新增交互会话","scope":["src/"],"acceptance":["启动成功"],"verification":"test actual cwd","context":{"nested":"keep complete context"}});
    fs::write(
        root.path().join("docs/issues/one.json"),
        serde_json::to_vec(&body).unwrap(),
    )
    .unwrap();
    let ticket = ticket_handoff::ready_ticket(root.path(), "p", "one").unwrap();
    (root, ticket)
}
#[test]
fn supported_runtime_vectors_are_interactive_and_never_bypass_approval() {
    let root = Path::new("/tmp/project space");
    let codex = ticket_handoff::interactive_command(
        "codex-cli",
        Path::new("/bin/codex"),
        None,
        root,
        "task",
    )
    .unwrap();
    assert_eq!(
        codex.args,
        [
            "--cd",
            "/tmp/project space",
            "--sandbox",
            "workspace-write",
            "--ask-for-approval",
            "on-request",
            "task"
        ]
    );
    let opencode = ticket_handoff::interactive_command(
        "opencode",
        Path::new("/bin/opencode"),
        None,
        root,
        "task",
    )
    .unwrap();
    assert_eq!(opencode.args, ["/tmp/project space", "--prompt", "task"]);
    let zcode = ticket_handoff::interactive_command(
        "zcode-app",
        Path::new("/app/zcode.cjs"),
        Some(Path::new("/bin/node")),
        root,
        "task",
    )
    .unwrap();
    assert_eq!(zcode.program, Path::new("/bin/node"));
    assert_eq!(
        zcode.args,
        [
            "/app/zcode.cjs",
            "tui",
            "--cwd",
            "/tmp/project space",
            "--mode",
            "build"
        ]
    );
    assert!(zcode.needs_paste);
    assert!(!codex.needs_paste && !opencode.needs_paste);
    for command in [codex, opencode, zcode] {
        assert!(!command.args.iter().any(|arg| [
            "exec",
            "run",
            "yolo",
            "--auto",
            "--dangerously-bypass-approvals-and-sandbox"
        ]
        .contains(&arg.as_str())));
    }
}
#[test]
fn blocked_terminal_unknown_or_unreadable_tickets_never_launch() {
    let (root, _) = fixture();
    for status in [
        "done",
        "superseded",
        "blocked",
        "in_progress",
        "review_fail",
        "unknown",
    ] {
        fs::write(
            root.path().join("docs/issues/index.json"),
            serde_json::to_vec(&json!({"issues":[{"id":"one","status":status}]})).unwrap(),
        )
        .unwrap();
        assert_eq!(
            ticket_handoff::ready_ticket(root.path(), "p", "one")
                .unwrap_err()
                .code,
            409
        );
    }
    fs::write(
        root.path().join("docs/issues/index.json"),
        r#"{"issues":[{"id":"one","status":"ready","blocked_by":["missing"]}]}"#,
    )
    .unwrap();
    assert_eq!(
        ticket_handoff::ready_ticket(root.path(), "p", "one")
            .unwrap_err()
            .code,
        409
    );
    assert_eq!(
        ticket_handoff::ready_ticket(root.path(), "p", "../one")
            .unwrap_err()
            .code,
        400
    );
}
#[test]
fn handoff_carries_complete_context_and_returns_actual_terminal_receipt() {
    let (root, ticket) = fixture();
    let app_data = TempDir::new().unwrap();
    let receipt = ticket_handoff::launch_with(
        root.path(),
        &ticket,
        "codex-cli",
        Path::new("/bin/codex"),
        None,
        app_data.path(),
        |command, clipboard| {
            assert!(command.contains("--ask-for-approval"));
            assert!(clipboard.is_none());
            Ok(("/dev/ttys-test".into(), 123))
        },
    )
    .unwrap();
    assert!(receipt.launched);
    assert_eq!(receipt.session_id, "/dev/ttys-test");
    assert_eq!(receipt.launcher_pid, 123);
    assert!(!receipt.needs_paste);
    let context = fs::read_to_string(&receipt.context_path).unwrap();
    assert!(context.contains("keep complete context"));
    assert!(context.contains("test actual cwd"));
    assert!(context.contains("docs/issues/one.json"));
    assert_eq!(
        receipt.project_path,
        root.path().canonicalize().unwrap().to_str().unwrap()
    );
    assert!(!root.path().join(".agentup-app").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(receipt.context_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
#[test]
fn zcode_clarifies_context_copy_and_launch_failure_is_not_success() {
    let (root, ticket) = fixture();
    let app_data = TempDir::new().unwrap();
    let receipt = ticket_handoff::launch_with(
        root.path(),
        &ticket,
        "zcode-app",
        Path::new("/bin/zcode"),
        None,
        app_data.path(),
        |command, clipboard| {
            assert!(command.contains("'tui'"));
            assert!(!command.contains("--prompt"));
            assert!(clipboard.unwrap().contains("keep complete context"));
            Ok(("/dev/ttys-test".into(), 42))
        },
    )
    .unwrap();
    assert!(receipt.needs_paste);
    assert_eq!(receipt.prompt_delivery, "clipboard");
    fs::remove_file(receipt.context_path).unwrap();
    for invalid_receipt in [false, true] {
        let result = ticket_handoff::launch_with(
            root.path(),
            &ticket,
            "codex-cli",
            Path::new("/bin/codex"),
            None,
            app_data.path(),
            |_, _| {
                if invalid_receipt {
                    Ok((String::new(), 0))
                } else {
                    Err(ApiError::internal("Terminal refused"))
                }
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read_dir(app_data.path()).unwrap().count(), 0);
    }
}
#[cfg(unix)]
#[test]
fn shell_boundary_preserves_cwd_and_hostile_arguments_without_execution() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("project 'quote' $(touch ROOT_PWNED)");
    fs::create_dir(&root).unwrap();
    let binary = temp.path().join("fake 'codex' `touch BIN_PWNED`");
    fs::write(&binary, "#!/bin/sh\nprintf '%s\\0' \"$PWD\" \"$@\"\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    let prompt = "single' double\"; $(touch PWNED) `touch PWNED2`\nnew line";
    let command =
        ticket_handoff::interactive_command("codex-cli", &binary, None, &root, prompt).unwrap();
    let shell = ticket_handoff::terminal_command(&root, &command).unwrap();
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(shell)
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let words: Vec<_> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|word| !word.is_empty())
        .map(|word| String::from_utf8(word.to_vec()).unwrap())
        .collect();
    assert_eq!(
        Path::new(&words[0]).canonicalize().unwrap(),
        root.canonicalize().unwrap()
    );
    assert_eq!(&words[1..], command.args.as_slice());
    assert_eq!(words.last().unwrap(), prompt);
    assert!(!root.join("PWNED").exists());
    assert!(!root.join("PWNED2").exists());
    assert!(!temp.path().join("ROOT_PWNED").exists());
    assert!(!temp.path().join("BIN_PWNED").exists());
}
