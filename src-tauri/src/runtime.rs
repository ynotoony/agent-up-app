use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

const COMMANDS: [&str; 31] = [
    "scan_project",
    "preview_initialize",
    "initialize_project",
    "create_request",
    "load_project",
    "list_projects",
    "register_project",
    "rebind_project",
    "remove_agentup",
    "preview_remove_agentup",
    "post_discussion",
    "add_attachment",
    "load_request_thread",
    "put_scope",
    "diff_scope",
    "put_task",
    "set_task_state",
    "load_board",
    "start_run",
    "apply_fake_script",
    "finish_run",
    "commit_changes",
    "cancel_run",
    "load_run",
    "advance_run_clock",
    "publish_result",
    "accept_result",
    "reject_result",
    "submit_feedback",
    "load_result_history",
    "export_diagnostics",
];
const MAX_SCAN_FILES: usize = 50_000;
const ID_PATTERN_MAX: usize = 200;

#[derive(Clone)]
struct BoundProject {
    root: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfirmationKind {
    Initialize,
    Remove,
}

#[derive(Clone)]
struct Confirmation {
    kind: ConfirmationKind,
    project_id: String,
    root: PathBuf,
    fingerprint: String,
    planned_paths: Vec<String>,
    event_id: String,
    manifest_id: String,
}

enum WalkItem {
    File {
        rel: String,
        path: PathBuf,
        len: u64,
    },
    Symlink {
        rel: String,
        target: String,
    },
}

pub struct Runtime {
    projects: HashMap<String, BoundProject>,
    tokens: HashMap<String, Confirmation>,
    notifications: Vec<(String, Value)>,
    app_index: Connection,
    app_data_dir: Option<PathBuf>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            projects: HashMap::new(),
            tokens: HashMap::new(),
            notifications: Vec::new(),
            app_index: open_app_index(None).expect("in-memory app index"),
            app_data_dir: None,
        }
    }

    pub fn with_app_data_dir(dir: PathBuf) -> Self {
        Self {
            projects: HashMap::new(),
            tokens: HashMap::new(),
            notifications: Vec::new(),
            app_index: open_app_index(Some(&dir)).expect("app index"),
            app_data_dir: Some(dir),
        }
    }

    pub fn take_notifications(&mut self) -> Vec<(String, Value)> {
        std::mem::take(&mut self.notifications)
    }

    pub fn scan_project(&mut self, project_path: &str) -> Value {
        const CMD: &str = "scan_project";
        let root = match require_canonical_dir(project_path) {
            Ok(path) => path,
            Err(err) => return err.into_value(CMD),
        };
        let walk = match collect_walk(&root) {
            Ok(items) => items,
            Err(err) => return err.into_value(CMD),
        };
        let fingerprint = match fingerprint_v1(&walk) {
            Ok(fp) => fp,
            Err(err) => return err.into_value(CMD),
        };
        let agentup_state = agentup_state(&root);
        let project_id = match project_id_for(&root, agentup_state) {
            Ok(id) => id,
            Err(err) => return err.into_value(CMD),
        };
        if !valid_id(&project_id) {
            return fail(
                CMD,
                "io_error",
                "Project identity is not a valid stable id.",
                None,
            );
        }
        self.projects
            .insert(project_id.clone(), BoundProject { root: root.clone() });
        let documents = documents_from(&walk);
        let project_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("project")
            .to_string();
        let project_type = infer_project_type(&walk);
        let event = json!({
            "event_id": format!("evt-scan-{}", random_hex(8)),
            "event_type": "project.scan_completed",
            "aggregate_type": "project",
            "aggregate_id": project_id,
            "aggregate_revision": 1,
            "source": "system",
            "occurred_at": now_rfc3339(),
            "delivery": "runtime_notification",
            "payload": {
                "project_id": project_id,
                "root_fingerprint": fingerprint,
                "agentup_state": agentup_state
            }
        });
        self.notifications
            .push(("project.scan_completed".to_string(), event));
        ok(
            CMD,
            json!({
                "project_id": project_id,
                "project_name": project_name,
                "project_type": project_type,
                "documents": documents,
                "agentup_state": agentup_state,
                "root_fingerprint": fingerprint
            }),
        )
    }

    pub fn preview_initialize(
        &mut self,
        project_id: &str,
        project_path: &str,
        expected_root_fingerprint: &str,
    ) -> Value {
        const CMD: &str = "preview_initialize";
        if !valid_id(project_id) {
            return fail(
                CMD,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            );
        }
        if !valid_fingerprint(expected_root_fingerprint) {
            return fail(
                CMD,
                "invalid_input",
                "expected_root_fingerprint is invalid.",
                Some(json!({"field": "expected_root_fingerprint"})),
            );
        }
        let root = match self.bind_existing(project_id, project_path) {
            Ok(path) => path,
            Err(err) => return err.into_value(CMD),
        };
        match agentup_state(&root) {
            "absent" => {}
            _ => {
                return fail(
                    CMD,
                    "already_initialized",
                    "The project already has an AgentUp directory.",
                    None,
                )
            }
        }
        let walk = match collect_walk(&root) {
            Ok(items) => items,
            Err(err) => return err.into_value(CMD),
        };
        let fingerprint = match fingerprint_v1(&walk) {
            Ok(fp) => fp,
            Err(err) => return err.into_value(CMD),
        };
        if fingerprint != expected_root_fingerprint {
            return fail(
                CMD,
                "invalid_input",
                "Root fingerprint does not match the scanned project.",
                Some(json!({"field": "expected_root_fingerprint"})),
            );
        }
        let event_id = format!("evt-init-{}", random_hex(8));
        let manifest_id = format!("manifest-{}", random_hex(8));
        if !valid_id(&event_id) || !valid_id(&manifest_id) {
            return fail(
                CMD,
                "io_error",
                "Failed to allocate initialization identifiers.",
                None,
            );
        }
        let planned_paths = vec![
            ".agentup/manifest.json".to_string(),
            ".agentup/events".to_string(),
            format!(".agentup/events/{event_id}.json"),
        ];
        let token = format!("tok-{}", random_hex(16));
        self.tokens.insert(
            token.clone(),
            Confirmation {
                kind: ConfirmationKind::Initialize,
                project_id: project_id.to_string(),
                root,
                fingerprint: fingerprint.clone(),
                planned_paths: planned_paths.clone(),
                event_id,
                manifest_id,
            },
        );
        ok(
            CMD,
            json!({
                "project_id": project_id,
                "planned_paths": planned_paths,
                "root_fingerprint": fingerprint,
                "confirmation_token": token,
                "requires_confirmation": true
            }),
        )
    }

    pub fn initialize_project(
        &mut self,
        project_id: &str,
        project_path: &str,
        confirmation_token: &str,
        expected_root_fingerprint: &str,
    ) -> Value {
        const CMD: &str = "initialize_project";
        if !valid_id(project_id) {
            return fail(
                CMD,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            );
        }
        if confirmation_token.is_empty() {
            return fail(
                CMD,
                "confirmation_required",
                "Initialization requires a confirmation token from preview.",
                None,
            );
        }
        if !valid_fingerprint(expected_root_fingerprint) {
            return fail(
                CMD,
                "invalid_input",
                "expected_root_fingerprint is invalid.",
                Some(json!({"field": "expected_root_fingerprint"})),
            );
        }
        let root = match self.bind_existing(project_id, project_path) {
            Ok(path) => path,
            Err(err) => return err.into_value(CMD),
        };
        if agentup_dir(&root).exists() {
            return fail(
                CMD,
                "already_initialized",
                "The project already has an AgentUp directory.",
                None,
            );
        }
        let Some(confirmation) = self.tokens.get(confirmation_token).cloned() else {
            return fail(
                CMD,
                "confirmation_expired",
                "The confirmation token is unknown or no longer valid.",
                None,
            );
        };
        if confirmation.kind != ConfirmationKind::Initialize {
            return fail(
                CMD,
                "confirmation_expired",
                "The confirmation token is not bound to initialization.",
                None,
            );
        }
        if confirmation.project_id != project_id || confirmation.root != root {
            return fail(
                CMD,
                "confirmation_expired",
                "The confirmation token is not bound to this project.",
                None,
            );
        }
        let walk = match collect_walk(&root) {
            Ok(items) => items,
            Err(err) => return err.into_value(CMD),
        };
        let fingerprint = match fingerprint_v1(&walk) {
            Ok(fp) => fp,
            Err(err) => return err.into_value(CMD),
        };
        if confirmation.planned_paths.is_empty() {
            return fail(
                CMD,
                "confirmation_expired",
                "The confirmation token is not bound to a preview.",
                None,
            );
        }
        if fingerprint != expected_root_fingerprint || fingerprint != confirmation.fingerprint {
            self.tokens.remove(confirmation_token);
            return fail(
                CMD,
                "confirmation_expired",
                "The project fingerprint changed after preview.",
                None,
            );
        }
        self.tokens.remove(confirmation_token);
        let initialized_at = now_rfc3339();
        let manifest = json!({
            "id": confirmation.manifest_id,
            "type": "manifest",
            "schema_version": 1,
            "project_id": project_id,
            "request_id": Value::Null,
            "revision": 1,
            "source": "system",
            "created_at": initialized_at,
            "updated_at": initialized_at,
            "content": {
                "format_version": 1,
                "project_id": project_id,
                "revision": 1,
                "initialized_at": initialized_at,
                "root_fingerprint": fingerprint
            },
            "metadata": {}
        });
        let event = json!({
            "event_id": confirmation.event_id,
            "event_type": "project.initialized",
            "aggregate_type": "project",
            "aggregate_id": project_id,
            "aggregate_revision": 1,
            "source": "system",
            "occurred_at": initialized_at,
            "delivery": "persisted",
            "payload": {
                "project_id": project_id,
                "manifest_id": confirmation.manifest_id,
                "manifest_revision": 1,
                "initialized_at": initialized_at,
                "root_fingerprint": fingerprint
            }
        });
        if let Err(err) = commit_initialize(&root, &manifest, &event, &confirmation.event_id) {
            return err.into_value(CMD);
        }
        self.notifications
            .push(("project.initialized".to_string(), event.clone()));
        ok(
            CMD,
            json!({
                "project_id": project_id,
                "manifest": {
                    "id": confirmation.manifest_id,
                    "type": "manifest",
                    "revision": 1,
                    "project_id": project_id
                },
                "initialized_at": initialized_at,
                "initialization_event_id": confirmation.event_id
            }),
        )
    }

    pub fn create_request(
        &mut self,
        project_id: &str,
        request_id: &str,
        content: Value,
        metadata: Value,
        expected_revision: i64,
    ) -> Value {
        const CMD: &str = "create_request";
        if !valid_id(project_id) {
            return fail(
                CMD,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            );
        }
        if !valid_id(request_id) {
            return fail(
                CMD,
                "invalid_input",
                "request_id is invalid.",
                Some(json!({"field": "request_id"})),
            );
        }
        if !content.is_object() {
            return fail(
                CMD,
                "invalid_input",
                "content must be an object.",
                Some(json!({"field": "content"})),
            );
        }
        if !metadata.is_object() {
            return fail(
                CMD,
                "invalid_input",
                "metadata must be an object.",
                Some(json!({"field": "metadata"})),
            );
        }
        let Some(bound) = self.projects.get(project_id).cloned() else {
            return fail(
                CMD,
                "not_initialized",
                "No bound initialized project was found for this project_id.",
                None,
            );
        };
        let root = bound.root;
        if let Err(err) = reject_agentup_symlink(&root) {
            return err.into_value(CMD);
        }
        let agentup = agentup_dir(&root);
        if !agentup.is_dir() {
            return fail(
                CMD,
                "not_initialized",
                "The project has not been initialized.",
                None,
            );
        }
        let fact_path = agentup
            .join("facts")
            .join("requests")
            .join(format!("{request_id}.json"));
        let current_revision = if fact_path.exists() { 1 } else { 0 };
        if current_revision > 0 && expected_revision == 0 {
            return fail(
                CMD,
                "already_exists",
                "A request with this request_id already exists.",
                None,
            );
        }
        if expected_revision != current_revision {
            return fail(
                CMD,
                "revision_conflict",
                "expected_revision does not match the current request revision.",
                Some(json!({"revision": current_revision})),
            );
        }
        if expected_revision != 0 {
            return fail(
                CMD,
                "revision_conflict",
                "M0 only creates new requests at revision 0.",
                Some(json!({"revision": current_revision})),
            );
        }
        let mut content_obj = match content.as_object() {
            Some(map) => map.clone(),
            None => {
                return fail(
                    CMD,
                    "invalid_input",
                    "content must be an object.",
                    Some(json!({"field": "content"})),
                )
            }
        };
        if let Some(lifecycle) = content_obj.get("lifecycle") {
            if lifecycle != "draft" {
                return fail(
                    CMD,
                    "malformed_fact",
                    "schema_version 1 only allows RequestLifecycle draft.",
                    Some(json!({"field": "content"})),
                );
            }
        } else {
            content_obj.insert("lifecycle".to_string(), json!("draft"));
        }
        let title = content_obj
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("");
        let body = content_obj
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("");
        if title.is_empty() || title.len() > 500 || body.is_empty() || body.len() > 100_000 {
            return fail(
                CMD,
                "malformed_fact",
                "Request content must include a valid title and body.",
                Some(json!({"field": "content"})),
            );
        }
        if content_obj
            .keys()
            .any(|key| !matches!(key.as_str(), "lifecycle" | "title" | "body"))
        {
            return fail(
                CMD,
                "malformed_fact",
                "Request content contains unsupported fields.",
                Some(json!({"field": "content"})),
            );
        }
        let created_at = now_rfc3339();
        let fact = json!({
            "id": format!("fact-{request_id}"),
            "type": "request",
            "schema_version": 1,
            "project_id": project_id,
            "request_id": request_id,
            "revision": 1,
            "source": "user",
            "created_at": created_at,
            "updated_at": created_at,
            "content": {
                "lifecycle": "draft",
                "title": title,
                "body": body
            },
            "metadata": metadata
        });
        if !valid_id(fact["id"].as_str().unwrap_or("")) {
            return fail(CMD, "malformed_fact", "Generated fact id is invalid.", None);
        }
        let event_id = format!("evt-req-{}", random_hex(8));
        let event = json!({
            "event_id": event_id,
            "event_type": "request.created",
            "aggregate_type": "request",
            "aggregate_id": request_id,
            "aggregate_revision": 1,
            "source": "user",
            "occurred_at": created_at,
            "delivery": "persisted",
            "payload": {
                "request_id": request_id,
                "fact_revision": 1,
                "lifecycle": "draft"
            }
        });
        if let Err(err) = persist_request(&root, request_id, &fact, &event) {
            return err.into_value(CMD);
        }
        self.notifications
            .push(("request.created".to_string(), event.clone()));
        ok(
            CMD,
            json!({
                "fact": fact,
                "event": event,
                "event_id": event_id
            }),
        )
    }

    pub fn load_project(&mut self, project_path: &str, expected_project_id: Option<&str>) -> Value {
        const CMD: &str = "load_project";
        if let Some(id) = expected_project_id {
            if !id.is_empty() && !valid_id(id) {
                return fail(
                    CMD,
                    "invalid_input",
                    "project_id is invalid.",
                    Some(json!({"field": "project_id"})),
                );
            }
        }
        let root = match require_canonical_dir(project_path) {
            Ok(path) => path,
            Err(err) => return err.into_value(CMD),
        };
        if let Err(err) = reject_agentup_symlink(&root) {
            return err.into_value(CMD);
        }
        let agentup = agentup_dir(&root);
        if !agentup.exists() {
            return fail(
                CMD,
                "not_initialized",
                "The project has not been initialized.",
                None,
            );
        }
        if !agentup.is_dir() {
            return fail(
                CMD,
                "malformed_fact",
                "The AgentUp directory is not a readable fact directory.",
                None,
            );
        }
        let manifest = match read_json(&agentup.join("manifest.json")) {
            Ok(value) => value,
            Err(err) => return err.into_value(CMD),
        };
        if let Err(err) = validate_manifest(&manifest) {
            return err.into_value(CMD);
        }
        let project_id = manifest["project_id"].as_str().unwrap_or("").to_string();
        if let Some(expected) = expected_project_id {
            if !expected.is_empty() && expected != project_id {
                return fail(
                    CMD,
                    "invalid_input",
                    "project_id does not match the stored manifest.",
                    Some(json!({"field": "project_id"})),
                );
            }
        }
        let events = match load_persisted_events(&agentup) {
            Ok(events) => events,
            Err(err) => return err.into_value(CMD),
        };
        let facts = match load_facts(&agentup, &manifest) {
            Ok(facts) => facts,
            Err(err) => return err.into_value(CMD),
        };
        let mut requests = Vec::new();
        for fact in &facts {
            if fact["type"] == "request" {
                let request_id = fact["request_id"].as_str().unwrap_or("").to_string();
                let revision = fact["revision"].as_i64().unwrap_or(0);
                let lifecycle = fact["content"]["lifecycle"].as_str().unwrap_or("");
                if lifecycle != "draft" {
                    return fail(
                        CMD,
                        "malformed_fact",
                        "A stored request is not in the allowed lifecycle.",
                        None,
                    );
                }
                requests.push(json!({
                    "request_id": request_id,
                    "revision": revision,
                    "lifecycle": "draft"
                }));
                let event = json!({
                    "event_id": format!("evt-rehydrate-{}", random_hex(8)),
                    "event_type": "request.rehydrated",
                    "aggregate_type": "request",
                    "aggregate_id": request_id,
                    "aggregate_revision": revision,
                    "source": "system",
                    "occurred_at": now_rfc3339(),
                    "delivery": "runtime_notification",
                    "payload": {
                        "request_id": request_id,
                        "request_revision": revision
                    }
                });
                self.notifications
                    .push(("request.rehydrated".to_string(), event));
            }
        }
        self.projects
            .insert(project_id.clone(), BoundProject { root });
        ok(
            CMD,
            json!({
                "manifest": manifest,
                "facts": facts,
                "events": events,
                "requests": requests,
                "project_id": project_id
            }),
        )
    }

    fn bind_existing(&mut self, project_id: &str, project_path: &str) -> Result<PathBuf, AppError> {
        let root = require_canonical_dir(project_path)?;
        match self.projects.get(project_id) {
            Some(bound) if bound.root == root => Ok(root),
            Some(_) => Err(AppError::code(
                "path_outside_project",
                "The provided path is not the bound project root.",
            )),
            None => {
                self.projects
                    .insert(project_id.to_string(), BoundProject { root: root.clone() });
                Ok(root)
            }
        }
    }
}

struct AppError {
    code: &'static str,
    message: String,
    details: Option<Value>,
}

impl AppError {
    fn code(code: &'static str, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            details: None,
        }
    }

    fn with_details(code: &'static str, message: &str, details: Value) -> Self {
        Self {
            code,
            message: message.to_string(),
            details: Some(details),
        }
    }

    fn from_io(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::NotFound => {
                Self::code("path_not_found", "The selected path was not found.")
            }
            io::ErrorKind::PermissionDenied => Self::code(
                "permission_denied",
                "Permission was denied for the selected path.",
            ),
            _ => Self::code(
                "io_error",
                "The command failed because of a safe I/O error.",
            ),
        }
    }

    fn into_value(self, command: &str) -> Value {
        fail(command, self.code, &self.message, self.details)
    }
}

pub(crate) fn ok(command: &str, data: Value) -> Value {
    json!({ "ok": true, "command": command, "data": data })
}

pub(crate) fn fail(command: &str, code: &str, message: &str, details: Option<Value>) -> Value {
    let mut error = Map::new();
    error.insert("code".to_string(), json!(code));
    error.insert("message".to_string(), json!(sanitize_message(message)));
    if let Some(details) = details {
        if let Some(obj) = details.as_object() {
            let mut safe = Map::new();
            for key in ["relative_path", "revision", "field"] {
                if let Some(value) = obj.get(key) {
                    safe.insert(key.to_string(), value.clone());
                }
            }
            if !safe.is_empty() {
                error.insert("details".to_string(), Value::Object(safe));
            }
        }
    }
    json!({
        "ok": false,
        "command": command,
        "error": error
    })
}

fn sanitize_message(message: &str) -> String {
    let mut out = message.to_string();
    for needle in ['/', '\\'] {
        if out.contains(needle) {
            out = "A safe error occurred while handling the selected path.".to_string();
            break;
        }
    }
    if out.len() > 1000 {
        out.truncate(1000);
    }
    if out.is_empty() {
        out = "Command failed.".to_string();
    }
    out
}

fn require_canonical_dir(project_path: &str) -> Result<PathBuf, AppError> {
    if project_path.trim().is_empty() {
        return Err(AppError::with_details(
            "invalid_input",
            "project_path is required.",
            json!({"field": "project_path"}),
        ));
    }
    let path = PathBuf::from(project_path);
    if !path.exists() {
        return Err(AppError::code(
            "path_not_found",
            "The selected path was not found.",
        ));
    }
    let meta = fs::symlink_metadata(&path).map_err(AppError::from_io)?;
    if meta.file_type().is_symlink() {
        // Selecting a directory via symlink is allowed only after canonicalize stays a directory.
        // Traversal of child symlinks is rejected during walk.
    }
    let canonical = fs::canonicalize(&path).map_err(AppError::from_io)?;
    if !canonical.is_dir() {
        return Err(AppError::with_details(
            "invalid_input",
            "project_path must be a directory.",
            json!({"field": "project_path"}),
        ));
    }
    Ok(canonical)
}

fn agentup_dir(root: &Path) -> PathBuf {
    root.join(".agentup")
}

fn reject_agentup_symlink(root: &Path) -> Result<(), AppError> {
    let path = agentup_dir(root);
    if !path.exists() && path.symlink_metadata().is_err() {
        return Ok(());
    }
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if meta.file_type().is_symlink() {
            return Err(AppError::code(
                "path_outside_project",
                "Refusing to follow a symbolic link at the AgentUp directory.",
            ));
        }
    }
    Ok(())
}

fn agentup_state(root: &Path) -> &'static str {
    let path = agentup_dir(root);
    if !path.exists() {
        return "absent";
    }
    if fs::symlink_metadata(&path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        return "invalid";
    }
    match read_json(&path.join("manifest.json")) {
        Ok(manifest) => {
            if validate_manifest(&manifest).is_ok() {
                "present"
            } else {
                "invalid"
            }
        }
        Err(_) => "invalid",
    }
}

fn project_id_for(root: &Path, state: &str) -> Result<String, AppError> {
    if state == "present" {
        let manifest = read_json(&agentup_dir(root).join("manifest.json"))?;
        if let Some(id) = manifest["project_id"].as_str() {
            return Ok(id.to_string());
        }
    }
    let digest = sha256_hex(root.to_string_lossy().as_bytes());
    Ok(format!("p-{}", &digest[..16]))
}

fn collect_walk(root: &Path) -> Result<Vec<WalkItem>, AppError> {
    let mut stack = vec![(root.to_path_buf(), String::new())];
    let mut items = Vec::new();
    let mut seen = 0usize;
    while let Some((dir, rel)) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(AppError::from_io)?;
        for entry in entries {
            let entry = entry.map_err(AppError::from_io)?;
            let name = entry.file_name();
            let Some(name_str) = name.to_str() else {
                return Err(AppError::code(
                    "io_error",
                    "A project path contained unsupported characters.",
                ));
            };
            if name_str == ".agentup" || name_str == ".git" {
                continue;
            }
            let child_rel = if rel.is_empty() {
                name_str.to_string()
            } else {
                format!("{rel}/{name_str}")
            };
            if child_rel.contains('\\') || child_rel.contains('\0') {
                return Err(AppError::code(
                    "io_error",
                    "A project path contained unsupported characters.",
                ));
            }
            let child_path = entry.path();
            let meta = fs::symlink_metadata(&child_path).map_err(AppError::from_io)?;
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&child_path).map_err(AppError::from_io)?;
                if symlink_escapes(root, &dir, &target) {
                    return Err(AppError::code(
                        "path_outside_project",
                        "Refusing to traverse a symbolic link outside the project root.",
                    ));
                }
                items.push(WalkItem::Symlink {
                    rel: child_rel,
                    target: target.to_string_lossy().replace('\\', "/"),
                });
                seen += 1;
            } else if meta.is_dir() {
                stack.push((child_path, child_rel));
            } else if meta.is_file() {
                items.push(WalkItem::File {
                    rel: child_rel,
                    path: child_path,
                    len: meta.len(),
                });
                seen += 1;
            }
            if seen > MAX_SCAN_FILES {
                return Err(AppError::code(
                    "io_error",
                    "The project exceeded the scan file limit.",
                ));
            }
        }
    }
    items.sort_by(|a, b| rel_of(a).cmp(rel_of(b)));
    Ok(items)
}

fn rel_of(item: &WalkItem) -> &str {
    match item {
        WalkItem::File { rel, .. } | WalkItem::Symlink { rel, .. } => rel,
    }
}

fn symlink_escapes(root: &Path, parent: &Path, target: &Path) -> bool {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        parent.join(target)
    };
    let normalized = normalize_lexically(&joined);
    let root_norm = normalize_lexically(root);
    !normalized.starts_with(&root_norm)
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

fn fingerprint_v1(items: &[WalkItem]) -> Result<String, AppError> {
    let mut records = Vec::new();
    for item in items {
        match item {
            WalkItem::File { rel, path, len } => {
                let bytes = fs::read(path).map_err(AppError::from_io)?;
                if bytes.len() as u64 != *len {
                    return Err(AppError::code(
                        "io_error",
                        "A file changed while it was being scanned.",
                    ));
                }
                let digest = sha256_hex(&bytes);
                records.push(format!("{rel}\0{len}\0{digest}\n"));
            }
            WalkItem::Symlink { rel, target } => {
                records.push(format!("{rel}\0SYMLINK\0{target}\n"));
            }
        }
    }
    records.sort();
    let mut joined = String::new();
    for record in records {
        joined.push_str(&record);
    }
    let digest = sha256_hex(joined.as_bytes());
    Ok(format!("fp-v1:{}", &digest[..16]))
}

fn documents_from(items: &[WalkItem]) -> Vec<Value> {
    let mut docs = Vec::new();
    for item in items {
        if let WalkItem::File { rel, .. } = item {
            if let Some(document_type) = document_type_for(rel) {
                docs.push(json!({
                    "relative_path": rel,
                    "document_type": document_type
                }));
            }
        }
    }
    docs
}

fn document_type_for(rel: &str) -> Option<&'static str> {
    let lower = rel.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if name == "readme.md" {
        Some("readme")
    } else if name.starts_with("changelog") {
        Some("changelog")
    } else if name.starts_with("license") {
        Some("license")
    } else if lower.ends_with(".md") {
        Some("markdown")
    } else {
        None
    }
}

fn infer_project_type(items: &[WalkItem]) -> &'static str {
    let names: HashSet<_> = items
        .iter()
        .filter_map(|item| match item {
            WalkItem::File { rel, .. } => Some(rel.as_str()),
            WalkItem::Symlink { .. } => None,
        })
        .collect();
    if names.contains("package.json") {
        "node"
    } else if names.contains("Cargo.toml") {
        "rust"
    } else if names.contains("pyproject.toml") || names.contains("requirements.txt") {
        "python"
    } else {
        "unknown"
    }
}

fn commit_initialize(
    root: &Path,
    manifest: &Value,
    event: &Value,
    event_id: &str,
) -> Result<(), AppError> {
    if let Err(err) = reject_agentup_symlink(root) {
        return Err(err);
    }
    let dest = agentup_dir(root);
    if dest.exists() {
        return Err(AppError::code(
            "already_initialized",
            "The project already has an AgentUp directory.",
        ));
    }
    let tmp_name = format!(".agentup.tmp.{}", random_hex(4));
    let tmp = root.join(&tmp_name);
    if tmp.exists() {
        return Err(AppError::code(
            "io_error",
            "A temporary initialization directory already exists.",
        ));
    }
    let cleanup = |tmp: &Path| {
        let _ = fs::remove_dir_all(tmp);
    };
    if let Err(err) = fs::create_dir(&tmp) {
        cleanup(&tmp);
        return Err(AppError::from_io(err));
    }
    let events_dir = tmp.join("events");
    if let Err(err) = fs::create_dir(&events_dir) {
        cleanup(&tmp);
        return Err(AppError::from_io(err));
    }
    if let Err(err) = write_json_atomic(&tmp.join("manifest.json"), manifest) {
        cleanup(&tmp);
        return Err(err);
    }
    if let Err(err) = write_json_atomic(&events_dir.join(format!("{event_id}.json")), event) {
        cleanup(&tmp);
        return Err(err);
    }
    fsync_dir(&tmp).map_err(AppError::from_io)?;
    fsync_dir(&events_dir).map_err(AppError::from_io)?;
    if dest.exists() {
        cleanup(&tmp);
        return Err(AppError::code(
            "already_initialized",
            "The project already has an AgentUp directory.",
        ));
    }
    if let Err(err) = fs::rename(&tmp, &dest) {
        cleanup(&tmp);
        return Err(AppError::from_io(err));
    }
    let _ = fsync_dir(root);
    Ok(())
}

fn persist_request(
    root: &Path,
    request_id: &str,
    fact: &Value,
    event: &Value,
) -> Result<(), AppError> {
    reject_agentup_symlink(root)?;
    let agentup = agentup_dir(root);
    if !agentup.is_dir() {
        return Err(AppError::code(
            "not_initialized",
            "The project has not been initialized.",
        ));
    }
    let facts_dir = agentup.join("facts").join("requests");
    let events_dir = agentup.join("events");
    fs::create_dir_all(&facts_dir).map_err(AppError::from_io)?;
    fs::create_dir_all(&events_dir).map_err(AppError::from_io)?;
    let fact_path = facts_dir.join(format!("{request_id}.json"));
    let event_id = event["event_id"].as_str().unwrap_or("event");
    let event_path = events_dir.join(format!("{event_id}.json"));
    if fact_path.exists() {
        return Err(AppError::code(
            "already_exists",
            "A request with this request_id already exists.",
        ));
    }
    if event_path.exists() {
        return Err(AppError::code(
            "malformed_event",
            "The event id already exists in the persisted event log.",
        ));
    }
    write_json_exclusive(&fact_path, fact)?;
    if let Err(err) = write_json_exclusive(&event_path, event) {
        let _ = fs::remove_file(&fact_path);
        return Err(err);
    }
    Ok(())
}

fn write_json_exclusive(path: &Path, value: &Value) -> Result<(), AppError> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(AppError::from_io)?;
        let payload = serde_json::to_vec_pretty(value)
            .map_err(|_| AppError::code("io_error", "Failed to encode JSON."))?;
        file.write_all(&payload).map_err(AppError::from_io)?;
        file.write_all(b"\n").map_err(AppError::from_io)?;
        file.flush().map_err(AppError::from_io)?;
        file.sync_all().map_err(AppError::from_io)?;
    }
    if let Err(err) = fs::hard_link(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        if path.exists() {
            return Err(AppError::code(
                "already_exists",
                "Refusing to replace an existing fact or event file.",
            ));
        }
        return Err(AppError::from_io(err));
    }
    let _ = fs::remove_file(&tmp);
    if let Some(parent) = path.parent() {
        let _ = fsync_dir(parent);
    }
    Ok(())
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<(), AppError> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(AppError::from_io)?;
        let payload = serde_json::to_vec_pretty(value)
            .map_err(|_| AppError::code("io_error", "Failed to encode JSON."))?;
        file.write_all(&payload).map_err(AppError::from_io)?;
        file.write_all(b"\n").map_err(AppError::from_io)?;
        file.flush().map_err(AppError::from_io)?;
        file.sync_all().map_err(AppError::from_io)?;
    }
    if let Err(err) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::from_io(err));
    }
    if let Some(parent) = path.parent() {
        let _ = fsync_dir(parent);
    }
    Ok(())
}

fn fsync_dir(path: &Path) -> io::Result<()> {
    let file = File::open(path)?;
    file.sync_all()
}

fn read_json(path: &Path) -> Result<Value, AppError> {
    let bytes = fs::read(path).map_err(AppError::from_io)?;
    serde_json::from_slice(&bytes).map_err(|_| {
        if path.file_name().and_then(|n| n.to_str()) == Some("manifest.json")
            || path.to_string_lossy().contains("/facts/")
        {
            AppError::code("malformed_fact", "A stored fact record is malformed.")
        } else {
            AppError::code("malformed_event", "A stored event record is malformed.")
        }
    })
}

fn validate_manifest(manifest: &Value) -> Result<(), AppError> {
    let type_ok = manifest["type"] == "manifest";
    let revision_ok = manifest["revision"] == 1;
    let source_ok = manifest["source"] == "system";
    let id_ok = manifest["id"].as_str().map(valid_id).unwrap_or(false);
    let project_ok = manifest["project_id"]
        .as_str()
        .map(valid_id)
        .unwrap_or(false);
    let content = &manifest["content"];
    let content_ok = content["format_version"] == 1
        && content["revision"] == 1
        && content["project_id"] == manifest["project_id"]
        && valid_fingerprint(content["root_fingerprint"].as_str().unwrap_or(""));
    if type_ok && revision_ok && source_ok && id_ok && project_ok && content_ok {
        Ok(())
    } else {
        Err(AppError::code(
            "malformed_fact",
            "The stored manifest is not a valid fact record.",
        ))
    }
}

fn load_persisted_events(agentup: &Path) -> Result<Vec<Value>, AppError> {
    let events_dir = agentup.join("events");
    if !events_dir.exists() {
        return Err(AppError::code(
            "malformed_event",
            "The event directory is missing.",
        ));
    }
    let mut events = Vec::new();
    let mut ids = HashSet::new();
    for entry in fs::read_dir(&events_dir).map_err(AppError::from_io)? {
        let entry = entry.map_err(AppError::from_io)?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let event = read_json(&path)?;
        let event_id = event["event_id"].as_str().unwrap_or("").to_string();
        if !valid_id(&event_id) {
            return Err(AppError::code(
                "malformed_event",
                "A stored event id is invalid.",
            ));
        }
        if !ids.insert(event_id.clone()) {
            return Err(AppError::code(
                "event_replay",
                "A persisted event id was replayed.",
            ));
        }
        let event_type = event["event_type"].as_str().unwrap_or("");
        let delivery = event["delivery"].as_str().unwrap_or("");
        if delivery != "persisted"
            || !matches!(
                event_type,
                "project.initialized"
                    | "request.created"
                    | "discussion.posted"
                    | "task.state_changed"
                    | "scope.updated"
                    | "decision.recorded"
                    | "result.published"
            )
        {
            return Err(AppError::code(
                "malformed_event",
                "A stored event is not an allowed persisted event.",
            ));
        }
        events.push(event);
    }
    events.sort_by(|a, b| {
        let left = (
            a["occurred_at"].as_str().unwrap_or(""),
            a["event_id"].as_str().unwrap_or(""),
        );
        let right = (
            b["occurred_at"].as_str().unwrap_or(""),
            b["event_id"].as_str().unwrap_or(""),
        );
        left.cmp(&right)
    });
    // Recheck revisions in replay order after sorting.
    let mut replayed: HashMap<String, i64> = HashMap::new();
    let mut replay_ids = HashSet::new();
    for event in &events {
        let event_id = event["event_id"].as_str().unwrap_or("");
        if !replay_ids.insert(event_id.to_string()) {
            return Err(AppError::code(
                "event_replay",
                "A persisted event id was replayed.",
            ));
        }
        let aggregate_id = event["aggregate_id"].as_str().unwrap_or("").to_string();
        let revision = event["aggregate_revision"].as_i64().unwrap_or(0);
        let previous = replayed.get(&aggregate_id).copied().unwrap_or(0);
        if revision != previous + 1 {
            return Err(AppError::code(
                "malformed_event",
                "Persisted event revisions are not contiguous.",
            ));
        }
        replayed.insert(aggregate_id, revision);
    }
    Ok(events)
}

fn load_facts(agentup: &Path, manifest: &Value) -> Result<Vec<Value>, AppError> {
    let mut facts = vec![manifest.clone()];
    let requests_dir = agentup.join("facts").join("requests");
    if !requests_dir.exists() {
        return Ok(facts);
    }
    for entry in fs::read_dir(&requests_dir).map_err(AppError::from_io)? {
        let entry = entry.map_err(AppError::from_io)?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let fact = read_json(&path)?;
        if fact["type"] != "request"
            || fact["source"] != "user"
            || fact["revision"] != 1
            || fact["content"]["lifecycle"] != "draft"
            || !valid_id(fact["id"].as_str().unwrap_or(""))
            || !valid_id(fact["request_id"].as_str().unwrap_or(""))
        {
            return Err(AppError::code(
                "malformed_fact",
                "A stored request fact is malformed.",
            ));
        }
        facts.push(fact);
    }
    facts.sort_by(|a, b| {
        let left = (
            a["revision"].as_i64().unwrap_or(0),
            a["id"].as_str().unwrap_or(""),
        );
        let right = (
            b["revision"].as_i64().unwrap_or(0),
            b["id"].as_str().unwrap_or(""),
        );
        left.cmp(&right)
    });
    Ok(facts)
}

fn valid_id(value: &str) -> bool {
    if value.is_empty() || value.len() > ID_PATTERN_MAX {
        return false;
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ':' | '-'))
}

fn valid_fingerprint(value: &str) -> bool {
    value.len() == 22
        && value.starts_with("fp-v1:")
        && value[6..]
            .chars()
            .all(|ch| matches!(ch, '0'..='9' | 'a'..='f'))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("system rng");
    buf.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[allow(dead_code)]
fn known_command(name: &str) -> bool {
    COMMANDS.contains(&name)
}

include!("typed_facts.rs");
include!("discussion.rs");
include!("scope_task.rs");
include!("agent_run.rs");
include!("results.rs");
include!("diagnostics.rs");
include!("projection.rs");
include!("project_index.rs");
