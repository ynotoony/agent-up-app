// Input: Isolated project roots containing governance indexes, ticket bodies and ticket drafts.
// Output: Read, readiness, atomic creation, conflict, malformed record and symlink regressions.
// Pos: Governance ticket filesystem integration tests; each case owns its filesystem data.
use agentup_harness_lib::{db, ticket_source};
use serde_json::{json, Value};
use std::{fs, path::Path};
use tempfile::TempDir;

fn write(root: &Path, relative: &str, value: &Value) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn body(id: &str) -> Value {
    json!({"id":id,"title":"来自真实票体的标题","goal":"实现独立验收任务","scope":["src/"],"acceptance":["验证通过"],"context":"完整上下文必须保留","verification":"cargo test"})
}
fn load(root: &Path, bodies: bool) -> Value {
    ticket_source::load_from_root(root, "test-project", bodies).unwrap()
}

fn draft() -> ticket_source::TicketDraft {
    ticket_source::TicketDraft {
        title: "New implementation task".into(),
        complexity: "C1".into(),
        profile: "D:Required,B:Advisory,I:Required,U:Required,S:N/A,M:N/A,O:N/A".into(),
        complexity_reason: "One bounded vertical slice.".into(),
        priority_reason: "The user explicitly requested this task.".into(),
        goal: "Users can create a task safely".into(),
        scope: vec!["src/".into()],
        acceptance: vec!["The task appears on the board".into()],
        verification: "cargo test --manifest-path src-tauri/Cargo.toml".into(),
        blocked_by: vec![],
    }
}

#[cfg(unix)]
fn fake_ticket_ops(root: &Path) -> std::path::PathBuf {
    let resource = root.join("agent-up");
    let scripts = resource.join("scripts");
    let schemas = resource.join("references/schemas");
    fs::create_dir_all(&scripts).unwrap();
    fs::create_dir_all(&schemas).unwrap();
    fs::write(scripts.join("generate-progress.sh"), "#!/bin/sh\nexit 0\n").unwrap();
    fs::write(schemas.join("ticket-record.schema.json"), "{}\n").unwrap();
    fs::write(schemas.join("issue-index.schema.json"), "{}\n").unwrap();
    let script = scripts.join("ticket-ops.sh");
    fs::write(
        &script,
        r#"#!/bin/sh
root=$1
shift
[ "$1" = open ] || exit 2
[ ! -f "$root/.fail-ticket-ops-before" ] || exit 9
shift
while [ "$#" -gt 0 ]; do
  case "$1" in
    --id) id=$2; shift 2 ;;
    --complexity) complexity=$2; shift 2 ;;
    --blocked-by) blocked_by=$2; shift 2 ;;
    --title|--ledger-line) shift 2 ;;
    *) exit 3 ;;
  esac
done
python3 - "$root" "$id" "$complexity" "${blocked_by:-}" <<'PY'
import json
import pathlib
import sys
root, ticket_id, complexity, blocked = sys.argv[1:]
path = pathlib.Path(root) / 'facts/requirements/tickets/index.json'
index = json.loads(path.read_text())
index['issues'].append({
    'id': ticket_id,
    'status': 'ready',
    'complexity': complexity,
    'blocked_by': [item for item in blocked.split(',') if item],
    'updated_at': '2026-10-12T00:00:00Z',
})
with path.open('w') as target:
    target.write('{"issues":[\n')
    for position, issue in enumerate(index['issues']):
        if position:
            target.write(',\n')
        target.write(json.dumps(issue, ensure_ascii=False, separators=(',', ':')))
    target.write('\n]}\n')
PY
[ ! -f "$root/.fail-ticket-ops-after" ] || exit 8
"#,
    )
    .unwrap();
    script
}

#[cfg(unix)]
fn bundled_ticket_ops(root: &Path) -> std::path::PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/agent-up-runtime");
    let resource = root.join("agent-up");
    let scripts = resource.join("scripts");
    let schemas = resource.join("references/schemas");
    fs::create_dir_all(&scripts).unwrap();
    fs::create_dir_all(&schemas).unwrap();
    for (from, to) in [
        ("ticket-ops.sh", scripts.join("ticket-ops.sh")),
        ("generate-progress.sh", scripts.join("generate-progress.sh")),
        (
            "ticket-record.schema.json",
            schemas.join("ticket-record.schema.json"),
        ),
        (
            "issue-index.schema.json",
            schemas.join("issue-index.schema.json"),
        ),
    ] {
        fs::copy(source.join(from), to).unwrap();
    }
    scripts.join("ticket-ops.sh")
}

fn create(root: &Path) -> Result<Value, agentup_harness_lib::error::ApiError> {
    #[cfg(unix)]
    let script = fake_ticket_ops(root);
    #[cfg(not(unix))]
    let script = root.join("ticket-ops.sh");
    ticket_source::create_ticket_from_root(root, "test-project", &draft(), &script)
}
#[test]
fn maps_lanes_and_reports_invalid_rows_and_unknown_statuses() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "docs/issues/index.json",
        &json!({"issues":[
            {"id":"one","status":"done","title":"已完成"},
            {"id":"two","status":"in_progress","blocked_by":["one"]},
            {"id":"three","status":"future_status"},
            {"id":"","status":"done"},{"id":"../escape","status":"done"},{"status":"done"}
        ]}),
    );
    let result = load(root.path(), false);
    assert_eq!(result["total"], 3);
    assert_eq!(result["skipped"], 3);
    assert_eq!(result["unknown_statuses"]["three"], "future_status");
    assert_eq!(result["tickets"][0]["lane"], "done");
    assert_eq!(result["tickets"][1]["lane"], "unknown");
    assert_eq!(result["tickets"][2]["lane"], "in_progress");
    assert!(result["tickets"][0].get("body").is_none());
    assert_eq!(result["tickets"][0]["body_loaded"], false);
    assert!(result["tickets"][0]["body_error"].is_string());
}
#[test]
fn all_three_sources_enrich_title_and_return_complete_body_only_when_requested() {
    for dir in [
        "facts/requirements/tickets",
        "docs/issues",
        "docs/project-management-mvp/issues",
    ] {
        let root = TempDir::new().unwrap();
        write(
            root.path(),
            &format!("{dir}/index.json"),
            &json!({"issues":[{"id":"one","status":"ready"}]}),
        );
        write(root.path(), &format!("{dir}/one.json"), &body("one"));
        let result = load(root.path(), false);
        let ticket = &result["tickets"][0];
        assert_eq!(ticket["title"], "来自真实票体的标题");
        assert_eq!(ticket["source_path"], format!("{dir}/one.json"));
        assert_eq!(result["index_path"], format!("{dir}/index.json"));
        assert_eq!(ticket["ready"], true);
        assert_eq!(ticket["body_loaded"], false);
        assert_eq!(ticket["goal_summary"], "实现独立验收任务");
        assert!(ticket.get("body").is_none());
        let detail =
            ticket_source::load_ticket_detail_from_root(root.path(), "test-project", "one")
                .unwrap();
        assert_eq!(detail["ticket"]["title"], "来自真实票体的标题");
        assert_eq!(detail["ticket"]["body"], body("one"));
    }
}

#[test]
fn selected_ticket_detail_reads_only_the_requested_body() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "docs/issues/index.json",
        &json!({"issues":[{"id":"one","status":"ready"},{"id":"two","status":"ready"}]}),
    );
    write(root.path(), "docs/issues/one.json", &body("one"));
    fs::write(root.path().join("docs/issues/two.json"), b"malformed").unwrap();

    let board = load(root.path(), false);
    assert_eq!(board["tickets"][0]["body_loaded"], false);
    assert_eq!(board["tickets"][0]["ready"], true);
    assert_eq!(board["tickets"][0]["title"], "来自真实票体的标题");
    assert_eq!(board["tickets"][0]["goal_summary"], "实现独立验收任务");
    assert!(board["tickets"][0].get("body").is_none());

    let detail = ticket_source::load_ticket_detail_from_root(root.path(), "p", "one").unwrap();
    assert_eq!(detail["project_id"], "p");
    assert_eq!(detail["index_path"], "docs/issues/index.json");
    assert_eq!(detail["ticket"]["body_loaded"], true);
    assert_eq!(detail["ticket"]["body"], body("one"));
    assert_eq!(detail["ticket"]["ready"], true);
    assert_eq!(
        ticket_source::load_ticket_detail_from_root(root.path(), "p", "missing")
            .unwrap_err()
            .code,
        404
    );
}

#[test]
fn board_summary_cache_refreshes_when_body_changes_and_keeps_body_out_of_cards() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "facts/requirements/tickets/index.json",
        &json!({"issues":[{"id":"01-summary","status":"ready","complexity":"C1","blocked_by":[],"updated_at":"2026-10-12T09:00"}]}),
    );
    let body_path = root
        .path()
        .join("facts/requirements/tickets/01-summary.json");
    write(
        root.path(),
        "facts/requirements/tickets/01-summary.json",
        &json!({"id":"01-summary","title":"First title","goal":"First goal","acceptance":["First criterion"]}),
    );

    let first = load(root.path(), false);
    assert_eq!(first["tickets"][0]["title"], "First title");
    assert_eq!(first["tickets"][0]["goal_summary"], "First goal");
    assert!(first["tickets"][0].get("body").is_none());
    assert_eq!(first["tickets"][0]["body_loaded"], false);

    fs::write(
        &body_path,
        serde_json::to_vec(&json!({"id":"01-summary","title":"Fresh title","goal":"Fresh goal","acceptance":["Fresh criterion"]})).unwrap(),
    )
    .unwrap();
    let updated = load(root.path(), false);
    assert_eq!(updated["tickets"][0]["title"], "Fresh title");
    assert_eq!(updated["tickets"][0]["goal_summary"], "Fresh goal");
    assert_eq!(updated["tickets"][0]["ready"], true);
    assert!(updated["tickets"][0].get("body").is_none());
}

#[cfg(unix)]
#[test]
fn ticket_creation_capability_allows_missing_first_projection_and_rejects_projection_symlink() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "facts/requirements/tickets/index.json",
        &json!({"issues":[]}),
    );
    fs::create_dir_all(root.path().join("facts/requirements/tickets")).unwrap();
    fs::write(
        root.path().join("facts/requirements/tickets/README.md"),
        "| `index.json` | source | tickets |\n",
    )
    .unwrap();
    fs::create_dir_all(root.path().join("facts/project")).unwrap();
    fs::write(root.path().join("facts/project/changes.jsonl"), "").unwrap();
    let script = fake_ticket_ops(root.path());

    assert!(ticket_source::ticket_creation_capability(root.path(), &script).is_ok());
    let outside = TempDir::new().unwrap();
    symlink(
        outside.path(),
        root.path()
            .join("facts/requirements/tickets/progress-current.md"),
    )
    .unwrap();
    assert!(ticket_source::ticket_creation_capability(root.path(), &script).is_err());
}
#[test]
fn readiness_requires_done_dependencies_and_exact_ready_status() {
    let root = TempDir::new().unwrap();
    let rows = json!({"issues":[
        {"id":"done","status":"done"}, {"id":"okay","status":"ready","blocked_by":["done"]},
        {"id":"blocked","status":"ready","blocked_by":["missing"]}, {"id":"self","status":"ready","blocked_by":["self"]},
        {"id":"bad","status":"ready","blocked_by":"done"}, {"id":"review","status":"review_fail"},
        {"id":"superseded","status":"superseded"}, {"id":"unknown","status":"surprise"}
    ]});
    write(root.path(), "docs/issues/index.json", &rows);
    for row in rows["issues"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        write(root.path(), &format!("docs/issues/{id}.json"), &body(id));
    }
    let result = load(root.path(), true);
    for ticket in result["tickets"].as_array().unwrap() {
        assert_eq!(ticket["ready"], ticket["id"] == "okay", "{ticket}");
        if ticket["id"] != "okay" {
            assert!(ticket["readiness_reason"].is_string());
        }
    }
}
#[test]
fn body_failures_are_visible_and_block_launch_without_hiding_index_card() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "docs/issues/index.json",
        &json!({"issues":[{"id":"one","status":"ready"}]}),
    );
    let path = root.path().join("docs/issues/one.json");
    for bytes in [
        b"invalid".to_vec(),
        serde_json::to_vec(&body("different")).unwrap(),
        vec![b'x'; 512 * 1024 + 1],
    ] {
        fs::write(&path, bytes).unwrap();
        let before = fs::read(&path).unwrap();
        let result = load(root.path(), true);
        assert_eq!(result["total"], 1);
        assert_eq!(result["tickets"][0]["ready"], false);
        assert!(result["tickets"][0]["body_error"].is_string());
        assert_eq!(fs::read(&path).unwrap(), before);
        let board = load(root.path(), false);
        assert_eq!(board["tickets"][0]["ready"], false);
        assert!(board["tickets"][0]["body_error"].is_string());
        assert!(board["tickets"][0].get("body").is_none());
    }
}
#[test]
fn missing_corrupt_oversized_duplicate_indexes_fail_visible() {
    let root = TempDir::new().unwrap();
    assert_eq!(
        ticket_source::load_from_root(root.path(), "p", false)
            .unwrap_err()
            .code,
        404
    );
    let path = root.path().join("docs/issues/index.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    for bytes in [
        b"{bad".to_vec(),
        b"{}".to_vec(),
        vec![b'x'; 2 * 1024 * 1024 + 1],
        serde_json::to_vec(
            &json!({"issues":[{"id":"one","status":"ready"},{"id":"one","status":"done"}]}),
        )
        .unwrap(),
    ] {
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            ticket_source::load_from_root(root.path(), "p", false)
                .unwrap_err()
                .code,
            400
        );
    }
}
#[cfg(unix)]
#[test]
fn symlinked_index_parent_and_ticket_body_never_escape_root() {
    use std::os::unix::fs::symlink;
    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write(
        outside.path(),
        "issues/index.json",
        &json!({"issues":[{"id":"one","status":"ready"}]}),
    );
    symlink(outside.path(), root.path().join("docs")).unwrap();
    assert!(ticket_source::load_from_root(root.path(), "p", true)
        .unwrap_err()
        .message
        .contains("符号链接"));
    fs::remove_file(root.path().join("docs")).unwrap();
    write(
        root.path(),
        "docs/issues/index.json",
        &json!({"issues":[{"id":"one","status":"ready"}]}),
    );
    write(outside.path(), "secret.json", &body("one"));
    symlink(
        outside.path().join("secret.json"),
        root.path().join("docs/issues/one.json"),
    )
    .unwrap();
    let result = load(root.path(), true);
    assert_eq!(result["tickets"][0]["ready"], false);
    assert!(result["tickets"][0]["body_error"]
        .as_str()
        .unwrap()
        .contains("符号链接"));
    assert!(result["tickets"][0].get("body").is_none());
}
#[test]
fn database_binding_is_required_and_missing_directories_fail_visible() {
    let data = TempDir::new().unwrap();
    let state = db::open_state(data.path()).unwrap();
    let conn = state.conn.lock().unwrap();
    let project = db::create_project_full(&conn, "unbound", None, None).unwrap();
    assert_eq!(
        ticket_source::load_governance_tickets(&conn, &project.id, false)
            .unwrap_err()
            .code,
        400
    );
    let missing = data.path().join("gone");
    let project = db::create_project_full(&conn, "gone", None, missing.to_str()).unwrap();
    assert_eq!(
        ticket_source::load_governance_tickets(&conn, &project.id, false)
            .unwrap_err()
            .code,
        404
    );
}

#[test]
fn ticket_creation_appends_index_and_publishes_matching_body() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("facts/requirements/tickets")).unwrap();
    fs::write(
        root.path().join("facts/requirements/tickets/index.json"),
        br#"{
  "issues": [
    {"id": "01-existing", "status": "done", "complexity": "C1", "blocked_by": [], "updated_at": "2026-10-10T09:00"}
  ]
}
"#,
    )
    .unwrap();

    let detail = create(root.path()).unwrap();

    assert_eq!(detail["ticket"]["id"], "02-new-implementation-task");
    assert_eq!(detail["ticket"]["title"], "New implementation task");
    assert_eq!(detail["ticket"]["body"]["id"], detail["ticket"]["id"]);
    assert_eq!(
        detail["ticket"]["body"]["goal"],
        "Users can create a task safely"
    );
    assert_eq!(detail["ticket"]["body"]["scope"], json!(["src/"]));
    assert_eq!(
        detail["ticket"]["body"]["acceptance"],
        json!(["The task appears on the board"])
    );
    assert_eq!(detail["ticket"]["ready"], true);
    assert_eq!(detail["ticket"]["body"]["complexity"], "C1");
    assert_eq!(detail["ticket"]["body"]["profile"], draft().profile);
    assert_eq!(detail["ticket"]["creation_warning"], Value::Null);
    assert_eq!(
        detail["ticket"]["body"]["verification"],
        draft().verification
    );
    assert_eq!(
        detail["index_path"],
        "facts/requirements/tickets/index.json"
    );
    let index_text =
        fs::read_to_string(root.path().join("facts/requirements/tickets/index.json")).unwrap();
    assert_eq!(
        index_text
            .lines()
            .filter(|line| line.contains("\"id\":"))
            .count(),
        2
    );
    assert_eq!(
        serde_json::from_str::<Value>(&index_text).unwrap(),
        json!({"issues":[
            {"id":"01-existing","status":"done","complexity":"C1","blocked_by":[],"updated_at":"2026-10-10T09:00"},
            {"id":"02-new-implementation-task","status":"ready","complexity":"C1","blocked_by":[],"updated_at":detail["ticket"]["updated_at"]}
        ]})
    );
}

#[cfg(unix)]
#[test]
fn bundled_ticket_ops_creates_ticket_and_updates_all_governance_records() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("facts/requirements/tickets")).unwrap();
    fs::write(
        root.path().join("facts/requirements/tickets/index.json"),
        br#"{
  "issues": [
    {"id": "01-existing", "status": "done", "complexity": "C1", "blocked_by": [], "updated_at": "2026-10-10T09:00"}
  ]
}
"#,
    )
    .unwrap();
    fs::write(
        root.path().join("facts/requirements/tickets/README.md"),
        "| `01-existing.json` | Task 01; `done` | Existing task |\n",
    )
    .unwrap();
    fs::create_dir_all(root.path().join("facts/project")).unwrap();
    fs::write(root.path().join("facts/project/changes.jsonl"), "").unwrap();

    let detail = ticket_source::create_ticket_from_root(
        root.path(),
        "p",
        &draft(),
        &bundled_ticket_ops(root.path()),
    )
    .unwrap();

    assert_eq!(detail["ticket"]["id"], "02-new-implementation-task");
    assert!(root
        .path()
        .join("facts/requirements/tickets/progress-current.md")
        .is_file());
    let readme =
        fs::read_to_string(root.path().join("facts/requirements/tickets/README.md")).unwrap();
    assert!(readme.contains("02-new-implementation-task.json"));
    let ledger = fs::read_to_string(root.path().join("facts/project/changes.jsonl")).unwrap();
    assert!(ledger.contains("facts/requirements/tickets/02-new-implementation-task.json"));
    let progress = fs::read_to_string(
        root.path()
            .join("facts/requirements/tickets/progress-current.md"),
    )
    .unwrap();
    assert!(progress.contains("| 02-new-implementation-task | ready |"));
}

#[test]
fn ticket_creation_rejects_existing_orphan_id_without_changing_index() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "facts/requirements/tickets/index.json",
        &json!({"issues":[]}),
    );
    write(
        root.path(),
        "facts/requirements/tickets/01-new-implementation-task.json",
        &body("01-new-implementation-task"),
    );
    let before = fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap();

    let error = create(root.path()).unwrap_err();

    assert_eq!(error.code, 409);
    assert_eq!(
        fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap(),
        before
    );
    let names: Vec<_> = fs::read_dir(root.path().join("facts/requirements/tickets"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names
        .iter()
        .all(|name| !name.to_string_lossy().starts_with(".agentup-ticket-")));
}

#[cfg(unix)]
#[test]
fn ticket_ops_failure_before_registration_removes_only_the_staged_body() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "facts/requirements/tickets/index.json",
        &json!({"issues":[]}),
    );
    fs::write(root.path().join(".fail-ticket-ops-before"), "fail").unwrap();
    let before = fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap();

    let error = create(root.path()).unwrap_err();

    assert_eq!(error.code, 409);
    assert_eq!(
        fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap(),
        before
    );
    assert!(!root
        .path()
        .join("facts/requirements/tickets/01-new-implementation-task.json")
        .exists());
    assert!(fs::read_dir(root.path().join("facts/requirements/tickets"))
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".agentup-ticket-")));
}

#[cfg(unix)]
#[test]
fn ticket_ops_partial_failure_returns_inspectable_ticket_and_preserves_files() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "facts/requirements/tickets/index.json",
        &json!({"issues":[]}),
    );
    fs::write(root.path().join(".fail-ticket-ops-after"), "fail").unwrap();

    let detail = create(root.path()).unwrap();

    assert_eq!(detail["project_id"], "test-project");
    assert_eq!(
        detail["index_path"],
        "facts/requirements/tickets/index.json"
    );
    assert_eq!(detail["ticket"]["id"], "01-new-implementation-task");
    assert_eq!(detail["ticket"]["title"], "New implementation task");
    assert_eq!(detail["ticket"]["body"]["id"], detail["ticket"]["id"]);
    let warning = detail["ticket"]["creation_warning"].as_str().unwrap();
    assert!(warning.contains("01-new-implementation-task"));
    assert!(warning.contains("请勿重复创建或启动"));
    assert!(warning.contains("ticket-ops"));
    let body_path = root
        .path()
        .join("facts/requirements/tickets/01-new-implementation-task.json");
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(body_path).unwrap()).unwrap(),
        detail["ticket"]["body"]
    );
    let index: Value = serde_json::from_slice(
        &fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(index["issues"][0]["id"], "01-new-implementation-task");
}

#[test]
fn ticket_creation_rejects_duplicate_index_ids_before_writing() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "facts/requirements/tickets/index.json",
        &json!({"issues":[{"id":"01-duplicate","status":"ready"},{"id":"01-duplicate","status":"done"}]}),
    );
    let before = fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap();

    assert_eq!(create(root.path()).unwrap_err().code, 400);
    assert_eq!(
        fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap(),
        before
    );
    assert_eq!(
        fs::read_dir(root.path().join("facts/requirements/tickets"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn ticket_draft_parser_requires_schema_fields_and_exact_profile_dimensions() {
    let valid = serde_json::to_string(&draft()).unwrap();
    assert_eq!(ticket_source::parse_ticket_draft(&valid).unwrap(), draft());

    let missing = json!({
        "title":"Task", "complexity":"C1", "profile":"D:Required,B:Required,I:Required,U:Required,S:N/A,M:N/A,O:N/A",
        "complexity_reason":"Reason", "priority_reason":"Reason", "goal":"A valid goal",
        "scope":["src/"], "acceptance":["Works"], "blocked_by":[]
    });
    assert!(ticket_source::parse_ticket_draft(&missing.to_string()).is_err());

    let mut malformed_profile = draft();
    malformed_profile.profile = "D:Required,B:Required,I:Required,U:Required,S:N/A,M:N/A".into();
    assert!(
        ticket_source::parse_ticket_draft(&serde_json::to_string(&malformed_profile).unwrap())
            .is_err()
    );

    let mut unknown = serde_json::to_value(draft()).unwrap();
    unknown["status"] = json!("ready");
    assert!(ticket_source::parse_ticket_draft(&unknown.to_string()).is_err());
}

#[test]
fn ticket_creation_rejects_dependencies_missing_from_index() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "facts/requirements/tickets/index.json",
        &json!({"issues":[]}),
    );
    let mut new_draft = draft();
    new_draft.blocked_by = vec!["09-missing".into()];
    let before = fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap();

    assert_eq!(
        ticket_source::create_ticket_from_root(
            root.path(),
            "p",
            &new_draft,
            &fake_ticket_ops(root.path()),
        )
        .unwrap_err()
        .code,
        400
    );
    assert_eq!(
        fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap(),
        before
    );
    assert_eq!(
        fs::read_dir(root.path().join("facts/requirements/tickets"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn ticket_creation_preserves_validated_dependency_edges() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "facts/requirements/tickets/index.json",
        &json!({"issues":[{"id":"01-existing","status":"done","complexity":"C1","blocked_by":[],"updated_at":"2026-10-10T09:00"}]}),
    );
    let mut new_draft = draft();
    new_draft.blocked_by = vec!["01-existing".into()];

    let detail = ticket_source::create_ticket_from_root(
        root.path(),
        "p",
        &new_draft,
        &fake_ticket_ops(root.path()),
    )
    .unwrap();

    assert_eq!(
        detail["ticket"]["body"]["blocked_by"],
        json!(["01-existing"])
    );
    let index: Value = serde_json::from_slice(
        &fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(index["issues"][1]["blocked_by"], json!(["01-existing"]));
    assert_eq!(detail["ticket"]["ready"], true);
}

#[cfg(unix)]
#[test]
fn ticket_creation_rejects_symlinked_index_and_body() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write(outside.path(), "index.json", &json!({"issues":[]}));
    fs::create_dir_all(root.path().join("facts/requirements/tickets")).unwrap();
    symlink(
        outside.path().join("index.json"),
        root.path().join("facts/requirements/tickets/index.json"),
    )
    .unwrap();
    let outside_index = fs::read(outside.path().join("index.json")).unwrap();
    assert_eq!(create(root.path()).unwrap_err().code, 400);
    assert_eq!(
        fs::read(outside.path().join("index.json")).unwrap(),
        outside_index
    );
    fs::remove_file(root.path().join("facts/requirements/tickets/index.json")).unwrap();

    write(
        root.path(),
        "facts/requirements/tickets/index.json",
        &json!({"issues":[]}),
    );
    write(
        outside.path(),
        "body.json",
        &body("01-new-implementation-task"),
    );
    symlink(
        outside.path().join("body.json"),
        root.path()
            .join("facts/requirements/tickets/01-new-implementation-task.json"),
    )
    .unwrap();
    let outside_body = fs::read(outside.path().join("body.json")).unwrap();
    assert_eq!(create(root.path()).unwrap_err().code, 409);
    assert_eq!(
        fs::read(outside.path().join("body.json")).unwrap(),
        outside_body
    );
    assert_eq!(
        serde_json::from_slice::<Value>(
            &fs::read(root.path().join("facts/requirements/tickets/index.json")).unwrap()
        )
        .unwrap(),
        json!({"issues":[]})
    );
    assert!(fs::read_dir(root.path().join("facts/requirements/tickets"))
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".agentup-ticket-")));
}
