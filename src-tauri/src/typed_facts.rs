use serde::{Deserialize, Serialize};

const WRITE_FACT: &str = "write_fact";
const READ_FACT: &str = "read_fact";
const MAX_ATTACHMENT_BYTES: i64 = 10 * 1024 * 1024;
const MAX_REQUEST_ATTACHMENT_BYTES: i64 = 100 * 1024 * 1024;

impl Runtime {
    pub fn write_fact(
        &mut self,
        project_id: &str,
        fact_id: &str,
        fact_type: &str,
        content: Value,
        metadata: Value,
        expected_revision: i64,
        event_id: Option<&str>,
    ) -> Value {
        if !valid_id(project_id) {
            return fail(
                WRITE_FACT,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            );
        }
        if !valid_id(fact_id) {
            return fail(
                WRITE_FACT,
                "invalid_input",
                "fact_id is invalid.",
                Some(json!({"field": "fact_id"})),
            );
        }
        let Some(dir_name) = fact_dir_name(fact_type) else {
            return fail(
                WRITE_FACT,
                "invalid_input",
                "fact_type is not a supported M1 fact type.",
                Some(json!({"field": "fact_type"})),
            );
        };
        if expected_revision < 0 {
            return fail(
                WRITE_FACT,
                "invalid_input",
                "expected_revision is invalid.",
                Some(json!({"field": "expected_revision"})),
            );
        }
        if !content.is_object() {
            return fail(
                WRITE_FACT,
                "invalid_input",
                "content must be an object.",
                Some(json!({"field": "content"})),
            );
        }
        if !metadata.is_object() {
            return fail(
                WRITE_FACT,
                "invalid_input",
                "metadata must be an object.",
                Some(json!({"field": "metadata"})),
            );
        }
        if let Some(id) = event_id {
            if !id.is_empty() && !valid_id(id) {
                return fail(
                    WRITE_FACT,
                    "invalid_input",
                    "event_id is invalid.",
                    Some(json!({"field": "event_id"})),
                );
            }
        }
        let Some(bound) = self.projects.get(project_id).cloned() else {
            return fail(
                WRITE_FACT,
                "not_initialized",
                "No bound initialized project was found for this project_id.",
                None,
            );
        };
        let root = bound.root;
        if let Err(err) = reject_agentup_symlink(&root) {
            return err.into_value(WRITE_FACT);
        }
        let agentup = agentup_dir(&root);
        if !agentup.is_dir() {
            return fail(
                WRITE_FACT,
                "not_initialized",
                "The project has not been initialized.",
                None,
            );
        }
        let facts_dir = agentup.join("facts").join(dir_name);
        let current_revision = match current_fact_revision(&facts_dir, fact_id) {
            Ok(rev) => rev,
            Err(err) => return err.into_value(WRITE_FACT),
        };
        if expected_revision != current_revision {
            return fail(
                WRITE_FACT,
                "revision_conflict",
                "expected_revision does not match the current fact revision.",
                Some(json!({"revision": current_revision})),
            );
        }
        if fact_type == "result" && current_revision > 0 {
            return fail(
                WRITE_FACT,
                "malformed_fact",
                "Result content is immutable for the same id.",
                Some(json!({"field": "content"})),
            );
        }
        let normalized = match normalize_typed_content(fact_type, &content) {
            Ok(value) => value,
            Err(err) => return err.into_value(WRITE_FACT),
        };
        let request_id = normalized["request_id"].as_str().unwrap_or("").to_string();
        if !valid_id(&request_id) {
            return fail(
                WRITE_FACT,
                "malformed_fact",
                "Fact content request_id is invalid.",
                Some(json!({"field": "content"})),
            );
        }
        if let Err(err) = reject_attachment_limits(&facts_dir, fact_type, fact_id, &normalized) {
            return err.into_value(WRITE_FACT);
        }
        let now = now_rfc3339();
        let created_at = if current_revision == 0 {
            now.clone()
        } else {
            match latest_fact(&facts_dir, fact_id) {
                Ok(previous) => previous["created_at"].as_str().unwrap_or(&now).to_string(),
                Err(err) => return err.into_value(WRITE_FACT),
            }
        };
        let revision = current_revision + 1;
        let source = fact_source(fact_type, &normalized);
        let fact = json!({
            "id": fact_id,
            "type": fact_type,
            "schema_version": 1,
            "project_id": project_id,
            "request_id": request_id,
            "revision": revision,
            "source": source,
            "created_at": created_at,
            "updated_at": now,
            "content": normalized,
            "metadata": metadata
        });
        let planned_event_id = event_id
            .filter(|id| !id.is_empty())
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("evt-{}-{}", fact_type, random_hex(8)));
        let event = match build_typed_event(
            fact_type,
            fact_id,
            &request_id,
            &normalized,
            revision,
            &source,
            &now,
            &planned_event_id,
        ) {
            Ok(event) => event,
            Err(err) => return err.into_value(WRITE_FACT),
        };
        if let Err(err) =
            persist_typed_fact(&root, &facts_dir, fact_id, revision, &fact, event.as_ref())
        {
            return err.into_value(WRITE_FACT);
        }
        if let Some(event) = event.clone() {
            self.notifications.push((
                event["event_type"].as_str().unwrap_or("").to_string(),
                event.clone(),
            ));
        }
        ok(
            WRITE_FACT,
            json!({
                "fact": fact,
                "event": event,
                "event_id": event.as_ref().map(|value| value["event_id"].clone())
            }),
        )
    }

    pub fn read_fact(&self, project_id: &str, fact_type: &str, fact_id: &str) -> Value {
        if !valid_id(project_id) {
            return fail(
                READ_FACT,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            );
        }
        if !valid_id(fact_id) {
            return fail(
                READ_FACT,
                "invalid_input",
                "fact_id is invalid.",
                Some(json!({"field": "fact_id"})),
            );
        }
        let Some(dir_name) = fact_dir_name(fact_type) else {
            return fail(
                READ_FACT,
                "invalid_input",
                "fact_type is not a supported M1 fact type.",
                Some(json!({"field": "fact_type"})),
            );
        };
        let Some(bound) = self.projects.get(project_id) else {
            return fail(
                READ_FACT,
                "not_initialized",
                "No bound initialized project was found for this project_id.",
                None,
            );
        };
        let agentup = agentup_dir(&bound.root);
        if !agentup.is_dir() {
            return fail(
                READ_FACT,
                "not_initialized",
                "The project has not been initialized.",
                None,
            );
        }
        let facts_dir = agentup.join("facts").join(dir_name);
        match latest_fact(&facts_dir, fact_id) {
            Ok(fact) => {
                if fact["type"] != fact_type || fact["id"] != fact_id {
                    return fail(
                        READ_FACT,
                        "malformed_fact",
                        "A stored fact record is malformed.",
                        None,
                    );
                }
                ok(READ_FACT, json!({ "fact": fact }))
            }
            Err(err) => err.into_value(READ_FACT),
        }
    }
}

fn fact_dir_name(fact_type: &str) -> Option<&'static str> {
    match fact_type {
        "discussion" => Some("discussions"),
        "plan" => Some("plans"),
        "task" => Some("tasks"),
        "scope" => Some("scopes"),
        "run" => Some("runs"),
        "decision" => Some("decisions"),
        "result" => Some("results"),
        "attachment" => Some("attachments"),
        _ => None,
    }
}

fn fact_source(fact_type: &str, content: &Value) -> String {
    if fact_type == "discussion" {
        content["author_source"]
            .as_str()
            .unwrap_or("user")
            .to_string()
    } else {
        "user".to_string()
    }
}

fn malformed_content(message: &str) -> AppError {
    AppError::with_details("malformed_fact", message, json!({"field": "content"}))
}

fn current_fact_revision(dir: &Path, fact_id: &str) -> Result<i64, AppError> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut max = 0i64;
    for entry in fs::read_dir(dir).map_err(AppError::from_io)? {
        let entry = entry.map_err(AppError::from_io)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if let Some(revision) = parse_revision_name(name, fact_id) {
            max = max.max(revision);
        }
    }
    Ok(max)
}

fn parse_revision_name(name: &str, fact_id: &str) -> Option<i64> {
    let prefix = format!("{fact_id}.r");
    let rest = name.strip_prefix(&prefix)?.strip_suffix(".json")?;
    if rest.is_empty() || !rest.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok().filter(|revision| *revision >= 1)
}

fn latest_fact(dir: &Path, fact_id: &str) -> Result<Value, AppError> {
    let revision = current_fact_revision(dir, fact_id)?;
    if revision == 0 {
        return Err(AppError::with_details(
            "invalid_input",
            "The requested fact was not found.",
            json!({"field": "fact_id"}),
        ));
    }
    read_json(&dir.join(format!("{fact_id}.r{revision}.json")))
}

fn persist_typed_fact(
    root: &Path,
    facts_dir: &Path,
    fact_id: &str,
    revision: i64,
    fact: &Value,
    event: Option<&Value>,
) -> Result<(), AppError> {
    reject_agentup_symlink(root)?;
    let agentup = agentup_dir(root);
    if !agentup.is_dir() {
        return Err(AppError::code(
            "not_initialized",
            "The project has not been initialized.",
        ));
    }
    let events_dir = agentup.join("events");
    fs::create_dir_all(facts_dir).map_err(AppError::from_io)?;
    fs::create_dir_all(&events_dir).map_err(AppError::from_io)?;
    let fact_path = facts_dir.join(format!("{fact_id}.r{revision}.json"));
    if fact_path.exists() {
        return Err(AppError::with_details(
            "revision_conflict",
            "A fact file for this revision already exists.",
            json!({"revision": revision}),
        ));
    }
    if let Some(event) = event {
        let event_id = event["event_id"].as_str().unwrap_or("");
        let event_path = events_dir.join(format!("{event_id}.json"));
        if event_path.exists() {
            return Err(AppError::code(
                "event_replay",
                "A persisted event id was replayed.",
            ));
        }
        write_json_atomic(&fact_path, fact)?;
        if let Err(err) = write_json_atomic(&event_path, event) {
            let _ = fs::remove_file(&fact_path);
            return Err(err);
        }
    } else {
        write_json_atomic(&fact_path, fact)?;
    }
    Ok(())
}

fn build_typed_event(
    fact_type: &str,
    fact_id: &str,
    request_id: &str,
    content: &Value,
    revision: i64,
    source: &str,
    occurred_at: &str,
    event_id: &str,
) -> Result<Option<Value>, AppError> {
    let event = match fact_type {
        "discussion" => json!({
            "event_id": event_id,
            "event_type": "discussion.posted",
            "aggregate_type": "discussion",
            "aggregate_id": fact_id,
            "aggregate_revision": revision,
            "source": source,
            "occurred_at": occurred_at,
            "delivery": "persisted",
            "payload": {
                "discussion_id": fact_id,
                "request_id": request_id,
                "fact_revision": revision
            }
        }),
        "task" => json!({
            "event_id": event_id,
            "event_type": "task.state_changed",
            "aggregate_type": "task",
            "aggregate_id": fact_id,
            "aggregate_revision": revision,
            "source": "system",
            "occurred_at": occurred_at,
            "delivery": "persisted",
            "payload": {
                "task_id": fact_id,
                "request_id": request_id,
                "task_state": content["task_state"],
                "fact_revision": revision
            }
        }),
        "scope" => json!({
            "event_id": event_id,
            "event_type": "scope.updated",
            "aggregate_type": "scope",
            "aggregate_id": fact_id,
            "aggregate_revision": revision,
            "source": source,
            "occurred_at": occurred_at,
            "delivery": "persisted",
            "payload": {
                "scope_id": fact_id,
                "request_id": request_id,
                "task_id": content["task_id"],
                "fact_revision": revision
            }
        }),
        "decision" if content["status"] == "chosen" || content["status"] == "cancelled" => json!({
            "event_id": event_id,
            "event_type": "decision.recorded",
            "aggregate_type": "decision",
            "aggregate_id": fact_id,
            "aggregate_revision": revision,
            "source": "user",
            "occurred_at": occurred_at,
            "delivery": "persisted",
            "payload": {
                "decision_id": fact_id,
                "request_id": request_id,
                "status": content["status"],
                "fact_revision": revision
            }
        }),
        "result" => json!({
            "event_id": event_id,
            "event_type": "result.published",
            "aggregate_type": "result",
            "aggregate_id": fact_id,
            "aggregate_revision": revision,
            "source": "system",
            "occurred_at": occurred_at,
            "delivery": "persisted",
            "payload": {
                "result_id": fact_id,
                "request_id": request_id,
                "version": content["version"],
                "fact_revision": revision
            }
        }),
        _ => return Ok(None),
    };
    Ok(Some(event))
}

fn reject_attachment_limits(
    attachments_dir: &Path,
    fact_type: &str,
    fact_id: &str,
    content: &Value,
) -> Result<(), AppError> {
    if fact_type != "attachment" {
        return Ok(());
    }
    let byte_length = content["byte_length"].as_i64().unwrap_or(-1);
    if byte_length < 0 || byte_length > MAX_ATTACHMENT_BYTES {
        return Err(AppError::with_details(
            "invalid_input",
            "Attachment exceeds the single-file size limit.",
            json!({"field": "content"}),
        ));
    }
    let request_id = content["request_id"].as_str().unwrap_or("");
    let mut total = byte_length;
    if attachments_dir.exists() {
        for entry in fs::read_dir(attachments_dir).map_err(AppError::from_io)? {
            let entry = entry.map_err(AppError::from_io)?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let stored = read_json(&path)?;
            if stored["type"] != "attachment" {
                continue;
            }
            if stored["id"].as_str() == Some(fact_id) {
                continue;
            }
            if stored["content"]["request_id"].as_str() != Some(request_id) {
                continue;
            }
            total += stored["content"]["byte_length"].as_i64().unwrap_or(0);
        }
    }
    if total > MAX_REQUEST_ATTACHMENT_BYTES {
        return Err(AppError::with_details(
            "invalid_input",
            "Attachment would exceed the per-request size limit.",
            json!({"field": "content"}),
        ));
    }
    Ok(())
}

fn normalize_typed_content(fact_type: &str, content: &Value) -> Result<Value, AppError> {
    let normalized = match fact_type {
        "discussion" => to_value(parse_discussion(content)?)?,
        "plan" => to_value(parse_plan(content)?)?,
        "task" => to_value(parse_task(content)?)?,
        "scope" => to_value(parse_scope(content)?)?,
        "run" => to_value(parse_run(content)?)?,
        "decision" => to_value(parse_decision(content)?)?,
        "result" => to_value(parse_result(content)?)?,
        "attachment" => to_value(parse_attachment(content)?)?,
        _ => return Err(malformed_content("Unsupported fact type.")),
    };
    Ok(normalized)
}

fn to_value<T: Serialize>(value: T) -> Result<Value, AppError> {
    serde_json::to_value(value).map_err(|_| malformed_content("Fact content could not be encoded."))
}

fn parse_err() -> AppError {
    malformed_content("Fact content does not match the type contract.")
}

fn require_id(value: &str) -> Result<(), AppError> {
    if valid_id(value) {
        Ok(())
    } else {
        Err(parse_err())
    }
}

fn require_time(value: &str) -> Result<(), AppError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|_| ())
        .map_err(|_| parse_err())
}

fn require_relative_path(value: &str) -> Result<(), AppError> {
    if valid_relative_path(value) {
        Ok(())
    } else {
        Err(parse_err())
    }
}

fn valid_relative_path(value: &str) -> bool {
    if value.is_empty() || value.len() > 1000 || value.starts_with('/') || value.contains('\\') {
        return false;
    }
    if value.chars().any(|ch| ch.is_control()) {
        return false;
    }
    !value.split('/').any(|segment| segment == "..")
}

fn unique_ids(ids: &[String]) -> Result<(), AppError> {
    let mut seen = HashSet::new();
    for id in ids {
        require_id(id)?;
        if !seen.insert(id) {
            return Err(parse_err());
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiscussionContent {
    request_id: String,
    author_source: String,
    body: String,
    posted_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachment_ids: Option<Vec<String>>,
}

fn parse_discussion(content: &Value) -> Result<DiscussionContent, AppError> {
    let parsed: DiscussionContent =
        serde_json::from_value(content.clone()).map_err(|_| parse_err())?;
    require_id(&parsed.request_id)?;
    require_time(&parsed.posted_at)?;
    if parsed.body.is_empty() || parsed.body.len() > 100_000 {
        return Err(parse_err());
    }
    if !matches!(parsed.author_source.as_str(), "user" | "agent" | "system") {
        return Err(parse_err());
    }
    if let Some(ids) = &parsed.attachment_ids {
        unique_ids(ids)?;
    }
    Ok(parsed)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PlanContent {
    request_id: String,
    summary: String,
    success_criteria: Vec<String>,
    constraints: Vec<String>,
    status: String,
}

fn parse_plan(content: &Value) -> Result<PlanContent, AppError> {
    let parsed: PlanContent = serde_json::from_value(content.clone()).map_err(|_| parse_err())?;
    require_id(&parsed.request_id)?;
    if parsed.summary.is_empty() || parsed.summary.len() > 10_000 {
        return Err(parse_err());
    }
    if parsed.success_criteria.is_empty()
        || parsed
            .success_criteria
            .iter()
            .any(|item| item.is_empty() || item.len() > 2000)
        || parsed
            .constraints
            .iter()
            .any(|item| item.is_empty() || item.len() > 2000)
    {
        return Err(parse_err());
    }
    if !matches!(parsed.status.as_str(), "draft" | "active" | "superseded") {
        return Err(parse_err());
    }
    Ok(parsed)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TaskContent {
    request_id: String,
    title: String,
    task_state: String,
    parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depends_on: Option<Vec<String>>,
}

fn parse_task(content: &Value) -> Result<TaskContent, AppError> {
    let parsed: TaskContent = serde_json::from_value(content.clone()).map_err(|_| parse_err())?;
    require_id(&parsed.request_id)?;
    if parsed.title.is_empty() || parsed.title.len() > 500 {
        return Err(parse_err());
    }
    if !matches!(
        parsed.task_state.as_str(),
        "ready"
            | "in_progress"
            | "blocked"
            | "review_ready"
            | "review_pass"
            | "review_fail"
            | "done"
    ) {
        return Err(parse_err());
    }
    if let Some(parent_id) = &parsed.parent_id {
        require_id(parent_id)?;
    }
    if let Some(ids) = &parsed.depends_on {
        unique_ids(ids)?;
    }
    Ok(parsed)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScopeEntry {
    relative_path: String,
    reason: String,
    source: String,
    included: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScopeContent {
    request_id: String,
    task_id: String,
    entries: Vec<ScopeEntry>,
}

fn parse_scope(content: &Value) -> Result<ScopeContent, AppError> {
    let parsed: ScopeContent = serde_json::from_value(content.clone()).map_err(|_| parse_err())?;
    require_id(&parsed.request_id)?;
    require_id(&parsed.task_id)?;
    for entry in &parsed.entries {
        require_relative_path(&entry.relative_path)?;
        if entry.reason.is_empty() || entry.reason.len() > 2000 {
            return Err(parse_err());
        }
        if !matches!(entry.source.as_str(), "user" | "agent" | "system" | "tool") {
            return Err(parse_err());
        }
    }
    Ok(parsed)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RunContent {
    request_id: String,
    task_id: Option<String>,
    kind: String,
    run_state: String,
    started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ended_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_input: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_output: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verdict: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_timeout_count: Option<i64>,
}

fn parse_run(content: &Value) -> Result<RunContent, AppError> {
    let parsed: RunContent = serde_json::from_value(content.clone()).map_err(|_| parse_err())?;
    require_id(&parsed.request_id)?;
    if let Some(task_id) = &parsed.task_id {
        require_id(task_id)?;
    }
    if !matches!(
        parsed.kind.as_str(),
        "implement" | "review" | "commit" | "command" | "verify"
    ) {
        return Err(parse_err());
    }
    if !matches!(
        parsed.run_state.as_str(),
        "active" | "interrupted" | "unknown" | "recovering" | "closed"
    ) {
        return Err(parse_err());
    }
    require_time(&parsed.started_at)?;
    if let Some(ended_at) = &parsed.ended_at {
        require_time(ended_at)?;
    }
    if let Some(code) = &parsed.error_code {
        if code.is_empty() || code.len() > 100 {
            return Err(parse_err());
        }
    }
    if let Some(prompt_version) = &parsed.prompt_version {
        if prompt_version.is_empty() || prompt_version.len() > 200 {
            return Err(parse_err());
        }
    }
    if let Some(provider) = &parsed.provider {
        if !matches!(provider.as_str(), "fake" | "replay") {
            return Err(parse_err());
        }
    }
    if parsed.token_input.unwrap_or(0) < 0 || parsed.token_output.unwrap_or(0) < 0 {
        return Err(parse_err());
    }
    if let Some(verdict) = &parsed.verdict {
        if !matches!(verdict.as_str(), "pass" | "fail") {
            return Err(parse_err());
        }
    }
    if parsed.elapsed_ms.unwrap_or(0) < 0 || parsed.provider_timeout_count.unwrap_or(0) < 0 {
        return Err(parse_err());
    }
    Ok(parsed)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DecisionContent {
    request_id: String,
    question: String,
    options: Vec<String>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    chosen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rationale: Option<String>,
}

fn parse_decision(content: &Value) -> Result<DecisionContent, AppError> {
    let parsed: DecisionContent =
        serde_json::from_value(content.clone()).map_err(|_| parse_err())?;
    require_id(&parsed.request_id)?;
    if parsed.question.is_empty() || parsed.question.len() > 2000 {
        return Err(parse_err());
    }
    if parsed.options.len() < 2
        || parsed
            .options
            .iter()
            .any(|item| item.is_empty() || item.len() > 500)
    {
        return Err(parse_err());
    }
    if !matches!(parsed.status.as_str(), "open" | "chosen" | "cancelled") {
        return Err(parse_err());
    }
    if parsed.status == "chosen" {
        let Some(chosen) = &parsed.chosen else {
            return Err(parse_err());
        };
        if chosen.is_empty() || chosen.len() > 500 {
            return Err(parse_err());
        }
    }
    if let Some(rationale) = &parsed.rationale {
        if rationale.is_empty() || rationale.len() > 10_000 {
            return Err(parse_err());
        }
    }
    Ok(parsed)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultContent {
    request_id: String,
    version: i64,
    summary: String,
    acceptance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence_paths: Option<Vec<String>>,
}

fn parse_result(content: &Value) -> Result<ResultContent, AppError> {
    let parsed: ResultContent = serde_json::from_value(content.clone()).map_err(|_| parse_err())?;
    require_id(&parsed.request_id)?;
    if parsed.version < 1 {
        return Err(parse_err());
    }
    if parsed.summary.is_empty() || parsed.summary.len() > 10_000 {
        return Err(parse_err());
    }
    if !matches!(
        parsed.acceptance.as_str(),
        "pending" | "accepted" | "rejected"
    ) {
        return Err(parse_err());
    }
    if let Some(paths) = &parsed.evidence_paths {
        for path in paths {
            require_relative_path(path)?;
        }
    }
    Ok(parsed)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AttachmentContent {
    request_id: String,
    relative_path: String,
    media_type: String,
    byte_length: i64,
    sha256: String,
}

fn parse_attachment(content: &Value) -> Result<AttachmentContent, AppError> {
    if content.get("bytes").is_some()
        || content.get("data").is_some()
        || content.get("content").is_some()
    {
        return Err(malformed_content(
            "Attachment content must not include file bytes.",
        ));
    }
    let parsed: AttachmentContent =
        serde_json::from_value(content.clone()).map_err(|_| parse_err())?;
    require_id(&parsed.request_id)?;
    require_relative_path(&parsed.relative_path)?;
    if parsed.relative_path.starts_with("attachments/") == false {
        return Err(parse_err());
    }
    if parsed.media_type.is_empty() || parsed.media_type.len() > 200 {
        return Err(parse_err());
    }
    if parsed.byte_length < 0 {
        return Err(parse_err());
    }
    if parsed.sha256.len() != 64
        || !parsed
            .sha256
            .chars()
            .all(|ch| matches!(ch, '0'..='9' | 'a'..='f'))
    {
        return Err(parse_err());
    }
    Ok(parsed)
}
