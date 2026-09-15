const REBUILD_PROJECTION: &str = "rebuild_projection";
const SQLITE_RELATIVE_PATH: &str = ".agentup/cache.sqlite";
const M1_FACT_TYPES: [&str; 8] = [
    "discussion",
    "plan",
    "task",
    "scope",
    "run",
    "decision",
    "result",
    "attachment",
];

impl Runtime {
    pub fn rebuild_projection(&self, project_id: &str) -> Value {
        if !valid_id(project_id) {
            return fail(
                REBUILD_PROJECTION,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            );
        }
        let Some(bound) = self.projects.get(project_id) else {
            return fail(
                REBUILD_PROJECTION,
                "not_initialized",
                "No bound initialized project was found for this project_id.",
                None,
            );
        };
        let root = bound.root.clone();
        if let Err(err) = reject_agentup_symlink(&root) {
            return err.into_value(REBUILD_PROJECTION);
        }
        let agentup = agentup_dir(&root);
        if !agentup.is_dir() {
            return fail(
                REBUILD_PROJECTION,
                "not_initialized",
                "The project has not been initialized.",
                None,
            );
        }
        let manifest = match read_json(&agentup.join("manifest.json")) {
            Ok(value) => value,
            Err(err) => return err.into_value(REBUILD_PROJECTION),
        };
        if let Err(err) = validate_manifest(&manifest) {
            return err.into_value(REBUILD_PROJECTION);
        }
        let stored_project_id = manifest["project_id"].as_str().unwrap_or("");
        if stored_project_id != project_id {
            return fail(
                REBUILD_PROJECTION,
                "invalid_input",
                "project_id does not match the stored manifest.",
                Some(json!({"field": "project_id"})),
            );
        }
        let facts = match scan_projection_facts(&agentup, &manifest) {
            Ok(facts) => facts,
            Err(err) => return err.into_value(REBUILD_PROJECTION),
        };
        let events = match load_persisted_events(&agentup) {
            Ok(events) => events,
            Err(err) => return err.into_value(REBUILD_PROJECTION),
        };
        let dest = agentup.join("cache.sqlite");
        let tmp = agentup.join("cache.sqlite.tmp");
        if let Err(err) = replace_projection_db(&tmp, &dest, &facts, &events) {
            let _ = fs::remove_file(&tmp);
            return err.into_value(REBUILD_PROJECTION);
        }
        let (indexed_facts, indexed_events) = match read_projection_db(&dest) {
            Ok(values) => values,
            Err(err) => return err.into_value(REBUILD_PROJECTION),
        };
        ok(
            REBUILD_PROJECTION,
            json!({
                "project_id": project_id,
                "sqlite_path": SQLITE_RELATIVE_PATH,
                "facts": indexed_facts,
                "events": indexed_events
            }),
        )
    }
}

fn sqlite_error() -> AppError {
    AppError::code(
        "io_error",
        "The projection database could not be rebuilt from facts.",
    )
}

fn json_text(value: &Value) -> Result<String, AppError> {
    serde_json::to_string(value).map_err(|_| sqlite_error())
}

fn parse_json_text(text: String) -> Result<Value, AppError> {
    serde_json::from_str(&text).map_err(|_| sqlite_error())
}

fn projection_row(
    fact_type: &str,
    fact_id: &str,
    revision: i64,
    request_id: &str,
    project_id: &str,
    source: &str,
    created_at: &str,
    updated_at: &str,
    content: &Value,
) -> Value {
    json!({
        "type": fact_type,
        "id": fact_id,
        "revision": revision,
        "request_id": request_id,
        "project_id": project_id,
        "source": source,
        "created_at": created_at,
        "updated_at": updated_at,
        "content": content
    })
}

fn scan_projection_facts(agentup: &Path, manifest: &Value) -> Result<Vec<Value>, AppError> {
    let project_id = manifest["project_id"].as_str().unwrap_or("");
    let mut facts = vec![projection_row(
        "manifest",
        manifest["id"].as_str().unwrap_or(""),
        manifest["revision"].as_i64().unwrap_or(0),
        "",
        project_id,
        manifest["source"].as_str().unwrap_or(""),
        manifest["created_at"].as_str().unwrap_or(""),
        manifest["updated_at"].as_str().unwrap_or(""),
        &manifest["content"],
    )];
    facts.extend(scan_request_facts(agentup, project_id)?);
    facts.extend(scan_typed_facts(agentup, project_id)?);
    facts.sort_by(|left, right| {
        (
            left["type"].as_str().unwrap_or(""),
            left["id"].as_str().unwrap_or(""),
            left["revision"].as_i64().unwrap_or(0),
        )
            .cmp(&(
                right["type"].as_str().unwrap_or(""),
                right["id"].as_str().unwrap_or(""),
                right["revision"].as_i64().unwrap_or(0),
            ))
    });
    let mut seen = HashSet::new();
    for fact in &facts {
        let key = (
            fact["type"].as_str().unwrap_or("").to_string(),
            fact["id"].as_str().unwrap_or("").to_string(),
            fact["revision"].as_i64().unwrap_or(0),
        );
        if !seen.insert(key) {
            return Err(AppError::code(
                "malformed_fact",
                "A stored fact revision is duplicated.",
            ));
        }
    }
    Ok(facts)
}

fn scan_request_facts(agentup: &Path, project_id: &str) -> Result<Vec<Value>, AppError> {
    let requests_dir = agentup.join("facts").join("requests");
    if !requests_dir.exists() {
        return Ok(Vec::new());
    }
    let mut facts = Vec::new();
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
            || fact["schema_version"] != 1
            || fact["project_id"] != project_id
            || fact["content"]["lifecycle"] != "draft"
            || !valid_id(fact["id"].as_str().unwrap_or(""))
            || !valid_id(fact["request_id"].as_str().unwrap_or(""))
        {
            return Err(AppError::code(
                "malformed_fact",
                "A stored request fact is malformed.",
            ));
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let expected = format!("{}.json", fact["request_id"].as_str().unwrap_or(""));
        if file_name != expected {
            return Err(AppError::code(
                "malformed_fact",
                "A stored request fact is malformed.",
            ));
        }
        facts.push(projection_row(
            "request",
            fact["id"].as_str().unwrap_or(""),
            1,
            fact["request_id"].as_str().unwrap_or(""),
            project_id,
            "user",
            fact["created_at"].as_str().unwrap_or(""),
            fact["updated_at"].as_str().unwrap_or(""),
            &fact["content"],
        ));
    }
    Ok(facts)
}

fn scan_typed_facts(agentup: &Path, project_id: &str) -> Result<Vec<Value>, AppError> {
    let mut facts = Vec::new();
    for fact_type in M1_FACT_TYPES {
        let Some(dir_name) = fact_dir_name(fact_type) else {
            continue;
        };
        let dir = agentup.join("facts").join(dir_name);
        if !dir.exists() {
            continue;
        }
        if !dir.is_dir() {
            return Err(AppError::code(
                "malformed_fact",
                "A stored fact directory is malformed.",
            ));
        }
        for entry in fs::read_dir(&dir).map_err(AppError::from_io)? {
            let entry = entry.map_err(AppError::from_io)?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            let Some((fact_id, revision)) = parse_typed_fact_name(file_name) else {
                return Err(AppError::code(
                    "malformed_fact",
                    "A stored fact record is malformed.",
                ));
            };
            let fact = read_json(&path)?;
            if fact["type"] != fact_type
                || fact["id"] != fact_id
                || fact["revision"] != revision
                || fact["schema_version"] != 1
                || fact["project_id"] != project_id
                || !fact["content"].is_object()
                || !fact["metadata"].is_object()
            {
                return Err(AppError::code(
                    "malformed_fact",
                    "A stored fact record is malformed.",
                ));
            }
            if let Err(err) = normalize_typed_content(fact_type, &fact["content"]) {
                return Err(err);
            }
            let request_id = fact["request_id"].as_str().unwrap_or("");
            let content_request_id = fact["content"]["request_id"].as_str().unwrap_or("");
            if request_id != content_request_id || !valid_id(request_id) {
                return Err(AppError::code(
                    "malformed_fact",
                    "A stored fact record is malformed.",
                ));
            }
            facts.push(projection_row(
                fact_type,
                fact_id,
                revision,
                request_id,
                project_id,
                fact["source"].as_str().unwrap_or(""),
                fact["created_at"].as_str().unwrap_or(""),
                fact["updated_at"].as_str().unwrap_or(""),
                &fact["content"],
            ));
        }
    }
    Ok(facts)
}

fn parse_typed_fact_name(name: &str) -> Option<(&str, i64)> {
    let rest = name.strip_suffix(".json")?;
    let (fact_id, revision) = rest.rsplit_once(".r")?;
    if !valid_id(fact_id) {
        return None;
    }
    if revision.is_empty() || !revision.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let parsed = revision.parse().ok().filter(|value| *value >= 1)?;
    Some((fact_id, parsed))
}

fn replace_projection_db(
    tmp: &Path,
    dest: &Path,
    facts: &[Value],
    events: &[Value],
) -> Result<(), AppError> {
    let _ = fs::remove_file(tmp);
    write_projection_db(tmp, facts, events)?;
    {
        let file = File::open(tmp).map_err(AppError::from_io)?;
        file.sync_all().map_err(AppError::from_io)?;
    }
    if let Err(err) = fs::rename(tmp, dest) {
        let _ = fs::remove_file(tmp);
        return Err(AppError::from_io(err));
    }
    if let Some(parent) = dest.parent() {
        let _ = fsync_dir(parent);
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        let leftover = PathBuf::from(format!("{}{suffix}", dest.display()));
        let _ = fs::remove_file(leftover);
    }
    Ok(())
}

fn write_projection_db(path: &Path, facts: &[Value], events: &[Value]) -> Result<(), AppError> {
    let conn = Connection::open(path).map_err(|_| sqlite_error())?;
    conn.pragma_update(None, "journal_mode", "OFF")
        .map_err(|_| sqlite_error())?;
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(|_| sqlite_error())?;
    conn.execute_batch(
        "
        CREATE TABLE fact_index (
            fact_type TEXT NOT NULL,
            fact_id TEXT NOT NULL,
            revision INTEGER NOT NULL,
            request_id TEXT NOT NULL,
            project_id TEXT NOT NULL,
            source TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            content_json TEXT NOT NULL,
            PRIMARY KEY (fact_type, fact_id, revision)
        );
        CREATE TABLE event_index (
            event_id TEXT NOT NULL PRIMARY KEY,
            event_type TEXT NOT NULL,
            aggregate_type TEXT NOT NULL,
            aggregate_id TEXT NOT NULL,
            aggregate_revision INTEGER NOT NULL,
            source TEXT NOT NULL,
            occurred_at TEXT NOT NULL,
            payload_json TEXT NOT NULL
        );
        ",
    )
    .map_err(|_| sqlite_error())?;
    {
        let mut insert = conn
            .prepare(
                "INSERT INTO fact_index (
                    fact_type, fact_id, revision, request_id, project_id, source,
                    created_at, updated_at, content_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .map_err(|_| sqlite_error())?;
        for fact in facts {
            insert
                .execute(params![
                    fact["type"].as_str().unwrap_or(""),
                    fact["id"].as_str().unwrap_or(""),
                    fact["revision"].as_i64().unwrap_or(0),
                    fact["request_id"].as_str().unwrap_or(""),
                    fact["project_id"].as_str().unwrap_or(""),
                    fact["source"].as_str().unwrap_or(""),
                    fact["created_at"].as_str().unwrap_or(""),
                    fact["updated_at"].as_str().unwrap_or(""),
                    json_text(&fact["content"])?,
                ])
                .map_err(|_| sqlite_error())?;
        }
    }
    {
        let mut insert = conn
            .prepare(
                "INSERT INTO event_index (
                    event_id, event_type, aggregate_type, aggregate_id,
                    aggregate_revision, source, occurred_at, payload_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .map_err(|_| sqlite_error())?;
        for event in events {
            insert
                .execute(params![
                    event["event_id"].as_str().unwrap_or(""),
                    event["event_type"].as_str().unwrap_or(""),
                    event["aggregate_type"].as_str().unwrap_or(""),
                    event["aggregate_id"].as_str().unwrap_or(""),
                    event["aggregate_revision"].as_i64().unwrap_or(0),
                    event["source"].as_str().unwrap_or(""),
                    event["occurred_at"].as_str().unwrap_or(""),
                    json_text(&event["payload"])?,
                ])
                .map_err(|_| sqlite_error())?;
        }
    }
    conn.close().map_err(|_| sqlite_error())?;
    Ok(())
}

fn read_projection_db(path: &Path) -> Result<(Vec<Value>, Vec<Value>), AppError> {
    let conn = Connection::open(path).map_err(|_| sqlite_error())?;
    let mut facts = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT fact_type, fact_id, revision, request_id, project_id, source,
                        created_at, updated_at, content_json
                 FROM fact_index
                 ORDER BY fact_type, fact_id, revision",
            )
            .map_err(|_| sqlite_error())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .map_err(|_| sqlite_error())?;
        for row in rows {
            let (
                fact_type,
                fact_id,
                revision,
                request_id,
                project_id,
                source,
                created_at,
                updated_at,
                content_json,
            ) = row.map_err(|_| sqlite_error())?;
            facts.push(projection_row(
                &fact_type,
                &fact_id,
                revision,
                &request_id,
                &project_id,
                &source,
                &created_at,
                &updated_at,
                &parse_json_text(content_json)?,
            ));
        }
    }
    let mut events = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT event_id, event_type, aggregate_type, aggregate_id,
                        aggregate_revision, source, occurred_at, payload_json
                 FROM event_index
                 ORDER BY occurred_at, event_id",
            )
            .map_err(|_| sqlite_error())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(|_| sqlite_error())?;
        for row in rows {
            let (
                event_id,
                event_type,
                aggregate_type,
                aggregate_id,
                aggregate_revision,
                source,
                occurred_at,
                payload_json,
            ) = row.map_err(|_| sqlite_error())?;
            events.push(json!({
                "event_id": event_id,
                "event_type": event_type,
                "aggregate_type": aggregate_type,
                "aggregate_id": aggregate_id,
                "aggregate_revision": aggregate_revision,
                "source": source,
                "occurred_at": occurred_at,
                "payload": parse_json_text(payload_json)?
            }));
        }
    }
    Ok((facts, events))
}
