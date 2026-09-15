impl Runtime {
    pub fn export_diagnostics(&self, project_id: &str) -> Value {
        const CMD: &str = "export_diagnostics";
        if !valid_id(project_id) {
            return fail(CMD, "invalid_input", "project_id is invalid.", Some(json!({"field": "project_id"})));
        }
        let Some(bound) = self.projects.get(project_id) else {
            return fail(CMD, "not_initialized", "No bound initialized project was found for this project_id.", None);
        };
        let agentup = agentup_dir(&bound.root);
        if !agentup.is_dir() {
            return fail(CMD, "not_initialized", "The project has not been initialized.", None);
        }
        let facts = match collect_redacted_facts(&agentup) {
            Ok(facts) => facts,
            Err(err) => return err.into_value(CMD),
        };
        let diag_id = format!("diag-{}", random_hex(8));
        let dir = agentup.join("diagnostics");
        if let Err(err) = fs::create_dir_all(&dir) {
            return AppError::from_io(err).into_value(CMD);
        }
        let path = dir.join(format!("{diag_id}.json"));
        let payload = json!({
            "id": diag_id,
            "schema_version": 1,
            "facts": facts
        });
        if let Err(err) = fs::write(&path, serde_json::to_vec_pretty(&payload).unwrap_or_else(|_| b"{}".to_vec())) {
            return AppError::from_io(err).into_value(CMD);
        }
        ok(CMD, json!({
            "diagnostic_id": diag_id,
            "relative_path": format!("diagnostics/{diag_id}.json")
        }))
    }
}

fn collect_redacted_facts(agentup: &Path) -> Result<Vec<Value>, AppError> {
    let facts_root = agentup.join("facts");
    if !facts_root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&facts_root).map_err(AppError::from_io)? {
        let entry = entry.map_err(AppError::from_io)?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        for file in fs::read_dir(&path).map_err(AppError::from_io)? {
            let file = file.map_err(AppError::from_io)?;
            let file_path = file.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = read_json(&file_path)?;
            out.push(redact_fact(raw));
        }
    }
    Ok(out)
}

fn redact_fact(mut fact: Value) -> Value {
    if let Some(obj) = fact.as_object_mut() {
        obj.remove("project_path");
        if let Some(content) = obj.get_mut("content") {
            *content = redact_value(content.clone());
        }
        if let Some(metadata) = obj.get_mut("metadata") {
            *metadata = redact_value(metadata.clone());
        }
    }
    redact_value(fact)
}

fn redact_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, val) in map {
                let lower = key.to_ascii_lowercase();
                if lower.contains("api_key")
                    || lower.contains("secret")
                    || lower.contains("password")
                    || lower == "token"
                    || lower.ends_with("_token")
                {
                    out.insert(key, json!("[redacted]"));
                } else {
                    out.insert(key, redact_value(val));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(redact_value).collect()),
        Value::String(s) => {
            if s.starts_with('/') || (s.len() > 2 && s.as_bytes()[1] == b':' && s.as_bytes()[0].is_ascii_alphabetic()) {
                json!("[path]")
            } else {
                Value::String(s)
            }
        }
        other => other,
    }
}
