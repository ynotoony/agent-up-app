impl Runtime {
    pub fn start_run(
        &mut self,
        project_id: &str,
        run_id: &str,
        request_id: &str,
        task_id: &str,
        kind: &str,
        provider: &str,
        prompt_version: &str,
        expected_revision: i64,
    ) -> Value {
        const CMD: &str = "start_run";
        if !valid_id(project_id) {
            return fail(CMD, "invalid_input", "project_id is invalid.", Some(json!({"field": "project_id"})));
        }
        if !valid_id(run_id) {
            return fail(CMD, "invalid_input", "run_id is invalid.", Some(json!({"field": "run_id"})));
        }
        if !valid_id(request_id) {
            return fail(CMD, "invalid_input", "request_id is invalid.", Some(json!({"field": "request_id"})));
        }
        if !valid_id(task_id) {
            return fail(CMD, "invalid_input", "task_id is invalid.", Some(json!({"field": "task_id"})));
        }
        if !matches!(kind, "implement" | "review") {
            return fail(CMD, "invalid_input", "kind is invalid.", Some(json!({"field": "kind"})));
        }
        if !matches!(provider, "fake" | "replay") {
            return fail(CMD, "invalid_input", "provider is invalid.", Some(json!({"field": "provider"})));
        }
        if prompt_version.is_empty() || prompt_version.len() > 200 {
            return fail(CMD, "invalid_input", "prompt_version is invalid.", Some(json!({"field": "prompt_version"})));
        }
        if expected_revision < 0 {
            return fail(CMD, "invalid_input", "expected_revision is invalid.", Some(json!({"field": "expected_revision"})));
        }
        let root = match require_bound_request(self, project_id, request_id) {
            Ok(root) => root,
            Err(err) => return err.into_value(CMD),
        };
        let agentup = agentup_dir(&root);
        let runs = match load_latest_typed_facts(&agentup, "run") {
            Ok(facts) => facts,
            Err(err) => return err.into_value(CMD),
        };
        let mut active_implement = 0;
        let mut active_review = 0;
        for run in &runs {
            if run["content"]["run_state"] != "active" {
                continue;
            }
            match run["content"]["kind"].as_str() {
                Some("implement") => active_implement += 1,
                Some("review") => active_review += 1,
                _ => {}
            }
        }
        let blocked = match kind {
            "implement" => active_implement >= 1,
            "review" => active_review >= 1,
            _ => false,
        };
        if blocked {
            return fail(
                CMD,
                "conflict",
                "An active run of this kind already exists for the project.",
                None,
            );
        }
        remap_command(
            self.write_fact(
                project_id,
                run_id,
                "run",
                json!({
                    "request_id": request_id,
                    "task_id": task_id,
                    "kind": kind,
                    "run_state": "active",
                    "started_at": now_rfc3339(),
                    "prompt_version": prompt_version,
                    "provider": provider
                }),
                json!({}),
                expected_revision,
                None,
            ),
            CMD,
        )
    }

    pub fn apply_fake_script(
        &mut self,
        project_id: &str,
        run_id: &str,
        calls: Value,
        expected_revision: i64,
    ) -> Value {
        const CMD: &str = "apply_fake_script";
        if !valid_id(project_id) {
            return fail(CMD, "invalid_input", "project_id is invalid.", Some(json!({"field": "project_id"})));
        }
        if !valid_id(run_id) {
            return fail(CMD, "invalid_input", "run_id is invalid.", Some(json!({"field": "run_id"})));
        }
        if expected_revision < 0 {
            return fail(CMD, "invalid_input", "expected_revision is invalid.", Some(json!({"field": "expected_revision"})));
        }
        let Some(bound) = self.projects.get(project_id).cloned() else {
            return fail(CMD, "not_initialized", "No bound initialized project was found for this project_id.", None);
        };
        let root = bound.root.clone();
        let agentup = agentup_dir(&root);
        if !agentup.is_dir() {
            return fail(CMD, "not_initialized", "The project has not been initialized.", None);
        }
        let fact = match latest_run_fact(&agentup, run_id) {
            Ok(fact) => fact,
            Err(err) => return err.into_value(CMD),
        };
        if fact["revision"].as_i64() != Some(expected_revision) {
            return fail(
                CMD,
                "revision_conflict",
                "expected_revision does not match the current fact revision.",
                Some(json!({"revision": fact["revision"]})),
            );
        }
        if fact["content"]["run_state"] != "active" {
            return fail(CMD, "invalid_input", "run is not active.", Some(json!({"field": "run_id"})));
        }
        let provider = fact["content"]["provider"].as_str().unwrap_or("");
        if !matches!(provider, "fake" | "replay") {
            return fail(CMD, "invalid_input", "provider cannot apply a fake script.", Some(json!({"field": "provider"})));
        }
        let Some(call_list) = calls.as_array() else {
            return fail(CMD, "invalid_input", "calls must be an array.", Some(json!({"field": "calls"})));
        };
        let request_id = fact["content"]["request_id"].as_str().unwrap_or("").to_string();
        let task_id = match fact["content"]["task_id"].as_str() {
            Some(id) => id.to_string(),
            None => return fail(CMD, "invalid_input", "run task_id is invalid.", Some(json!({"field": "task_id"}))),
        };
        let allowed = match included_scope_paths(&agentup, &request_id, &task_id) {
            Ok(paths) => paths,
            Err(err) => return err.into_value(CMD),
        };
        let mut prepared: Vec<(String, String, Option<String>)> = Vec::new();
        for call in call_list {
            let tool = call.get("tool").and_then(Value::as_str).unwrap_or("");
            let rel = call.get("relative_path").and_then(Value::as_str).unwrap_or("");
            if !valid_relative_path(rel) {
                return fail(CMD, "invalid_input", "relative_path is invalid.", Some(json!({"field": "relative_path"})));
            }
            match tool {
                "read" => prepared.push(("read".into(), rel.into(), None)),
                "write" => {
                    if allowed.is_empty() || !allowed.contains(rel) {
                        return fail(
                            CMD,
                            "invalid_input",
                            "write is outside the approved scope.",
                            Some(json!({"field": "relative_path"})),
                        );
                    }
                    let Some(content) = call.get("content").and_then(Value::as_str) else {
                        return fail(CMD, "invalid_input", "write requires content.", Some(json!({"field": "content"})));
                    };
                    prepared.push(("write".into(), rel.into(), Some(content.to_string())));
                }
                "run_command" => {
                    return fail(CMD, "invalid_input", "run_command is not allowed.", Some(json!({"field": "tool"})));
                }
                _ => {
                    return fail(CMD, "invalid_input", "tool is invalid.", Some(json!({"field": "tool"})));
                }
            }
        }
        let mut results = Vec::new();
        for (tool, rel, content) in &prepared {
            match tool.as_str() {
                "read" => match read_project_file(&root, rel) {
                    Ok(body) => results.push(json!({"tool": "read", "relative_path": rel, "content": body})),
                    Err(err) => return err.into_value(CMD),
                },
                "write" => {
                    if let Err(err) = write_project_file(&root, rel, content.as_deref().unwrap_or("")) {
                        return err.into_value(CMD);
                    }
                    results.push(json!({"tool": "write", "relative_path": rel}));
                }
                _ => {}
            }
        }
        let content = fact["content"].clone();
        let written = remap_command(
            self.write_fact(
                project_id,
                run_id,
                "run",
                content,
                json!({}),
                expected_revision,
                None,
            ),
            CMD,
        );
        if written["ok"] != true {
            return written;
        }
        let mut data = written["data"].clone();
        if let Some(obj) = data.as_object_mut() {
            obj.insert("results".to_string(), json!(results));
        }
        json!({"ok": true, "command": CMD, "data": data})
    }

    pub fn finish_run(
        &mut self,
        project_id: &str,
        run_id: &str,
        expected_revision: i64,
        token_input: i64,
        token_output: i64,
        verdict: Option<String>,
    ) -> Value {
        const CMD: &str = "finish_run";
        match close_run(
            self,
            CMD,
            project_id,
            run_id,
            expected_revision,
            "closed",
            token_input,
            token_output,
            verdict.as_deref(),
            true,
        ) {
            Ok(value) => value,
            Err(err) => err.into_value(CMD),
        }
    }

    pub fn cancel_run(
        &mut self,
        project_id: &str,
        run_id: &str,
        expected_revision: i64,
    ) -> Value {
        const CMD: &str = "cancel_run";
        match close_run(
            self,
            CMD,
            project_id,
            run_id,
            expected_revision,
            "interrupted",
            0,
            0,
            None,
            false,
        ) {
            Ok(value) => value,
            Err(err) => err.into_value(CMD),
        }
    }

    pub fn load_run(&self, project_id: &str, run_id: &str) -> Value {
        const CMD: &str = "load_run";
        if !valid_id(project_id) {
            return fail(CMD, "invalid_input", "project_id is invalid.", Some(json!({"field": "project_id"})));
        }
        if !valid_id(run_id) {
            return fail(CMD, "invalid_input", "run_id is invalid.", Some(json!({"field": "run_id"})));
        }
        let Some(bound) = self.projects.get(project_id) else {
            return fail(CMD, "not_initialized", "No bound initialized project was found for this project_id.", None);
        };
        let agentup = agentup_dir(&bound.root);
        match latest_run_fact(&agentup, run_id) {
            Ok(fact) => ok(CMD, json!({"fact": fact})),
            Err(err) => err.into_value(CMD),
        }
    }

    pub fn commit_changes(
        &mut self,
        project_id: &str,
        task_id: &str,
        request_id: &str,
    ) -> Value {
        const CMD: &str = "commit_changes";
        if !valid_id(project_id) {
            return fail(CMD, "invalid_input", "project_id is invalid.", Some(json!({"field": "project_id"})));
        }
        if !valid_id(task_id) {
            return fail(CMD, "invalid_input", "task_id is invalid.", Some(json!({"field": "task_id"})));
        }
        if !valid_id(request_id) {
            return fail(CMD, "invalid_input", "request_id is invalid.", Some(json!({"field": "request_id"})));
        }
        let root = match require_bound_request(self, project_id, request_id) {
            Ok(root) => root,
            Err(err) => return err.into_value(CMD),
        };
        let agentup = agentup_dir(&root);
        let runs = match load_latest_typed_facts(&agentup, "run") {
            Ok(facts) => facts,
            Err(err) => return err.into_value(CMD),
        };
        let latest_review = runs
            .iter()
            .filter(|run| {
                run["content"]["kind"] == "review"
                    && run["content"]["task_id"].as_str() == Some(task_id)
                    && run["content"]["request_id"].as_str() == Some(request_id)
            })
            .max_by(|a, b| {
                (
                    a["content"]["started_at"].as_str().unwrap_or(""),
                    a["revision"].as_i64().unwrap_or(0),
                    a["id"].as_str().unwrap_or(""),
                )
                    .cmp(&(
                        b["content"]["started_at"].as_str().unwrap_or(""),
                        b["revision"].as_i64().unwrap_or(0),
                        b["id"].as_str().unwrap_or(""),
                    ))
            });
        let Some(review) = latest_review else {
            return fail(CMD, "review_required", "A passing review is required before commit_changes.", None);
        };
        if review["content"]["verdict"].as_str() != Some("pass")
            || review["content"]["run_state"].as_str() != Some("closed")
        {
            return fail(CMD, "review_required", "A passing review is required before commit_changes.", None);
        }
        let decision_id = format!("dec-{task_id}-accept");
        remap_command(
            self.write_fact(
                project_id,
                &decision_id,
                "decision",
                json!({
                    "request_id": request_id,
                    "question": "Accept implement outcome",
                    "options": ["accept", "reject"],
                    "status": "chosen",
                    "chosen": "accept"
                }),
                json!({}),
                0,
                None,
            ),
            CMD,
        )
    }
}

fn close_run(
    runtime: &mut Runtime,
    command: &str,
    project_id: &str,
    run_id: &str,
    expected_revision: i64,
    run_state: &str,
    token_input: i64,
    token_output: i64,
    verdict: Option<&str>,
    require_review_verdict: bool,
) -> Result<Value, AppError> {
    if !valid_id(project_id) {
        return Err(AppError::with_details("invalid_input", "project_id is invalid.", json!({"field": "project_id"})));
    }
    if !valid_id(run_id) {
        return Err(AppError::with_details("invalid_input", "run_id is invalid.", json!({"field": "run_id"})));
    }
    if expected_revision < 0 {
        return Err(AppError::with_details("invalid_input", "expected_revision is invalid.", json!({"field": "expected_revision"})));
    }
    if token_input < 0 || token_output < 0 {
        return Err(AppError::with_details("invalid_input", "token estimates are invalid.", json!({"field": "token_input"})));
    }
    let Some(bound) = runtime.projects.get(project_id).cloned() else {
        return Err(AppError::code("not_initialized", "No bound initialized project was found for this project_id."));
    };
    let agentup = agentup_dir(&bound.root);
    let fact = latest_run_fact(&agentup, run_id)?;
    if fact["revision"].as_i64() != Some(expected_revision) {
        return Err(AppError::with_details(
            "revision_conflict",
            "expected_revision does not match the current fact revision.",
            json!({"revision": fact["revision"]}),
        ));
    }
    if fact["content"]["run_state"] != "active" {
        return Err(AppError::with_details("invalid_input", "run is not active.", json!({"field": "run_id"})));
    }
    let kind = fact["content"]["kind"].as_str().unwrap_or("");
    if require_review_verdict && kind == "review" {
        match verdict {
            Some("pass") | Some("fail") => {}
            _ => {
                return Err(AppError::with_details(
                    "invalid_input",
                    "review finish_run requires verdict.",
                    json!({"field": "verdict"}),
                ));
            }
        }
    }
    let mut content = fact["content"].clone();
    content["run_state"] = json!(run_state);
    content["ended_at"] = json!(now_rfc3339());
    content["token_input"] = json!(token_input);
    content["token_output"] = json!(token_output);
    if kind == "review" {
        if let Some(verdict) = verdict {
            content["verdict"] = json!(verdict);
        }
    }
    Ok(remap_command(
        runtime.write_fact(project_id, run_id, "run", content, json!({}), expected_revision, None),
        command,
    ))
}

fn latest_run_fact(agentup: &Path, run_id: &str) -> Result<Value, AppError> {
    let dir = agentup.join("facts").join("runs");
    latest_fact(&dir, run_id)
}

fn load_latest_typed_facts(agentup: &Path, fact_type: &str) -> Result<Vec<Value>, AppError> {
    let Some(dir_name) = fact_dir_name(fact_type) else {
        return Err(AppError::with_details("invalid_input", "fact_type is not a supported M1 fact type.", json!({"field": "fact_type"})));
    };
    let dir = agentup.join("facts").join(dir_name);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut latest: HashMap<String, i64> = HashMap::new();
    for entry in fs::read_dir(&dir).map_err(AppError::from_io)? {
        let entry = entry.map_err(AppError::from_io)?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if let Some((fact_id, revision)) = split_fact_revision_name(&name) {
            let current = latest.entry(fact_id).or_insert(0);
            *current = (*current).max(revision);
        }
    }
    let mut facts = Vec::new();
    for (fact_id, revision) in latest {
        let fact = read_json(&dir.join(format!("{fact_id}.r{revision}.json")))?;
        if fact["type"] != fact_type || fact["id"] != fact_id {
            return Err(AppError::code("malformed_fact", "A stored fact record is malformed."));
        }
        facts.push(fact);
    }
    Ok(facts)
}

fn included_scope_paths(agentup: &Path, request_id: &str, task_id: &str) -> Result<HashSet<String>, AppError> {
    let scopes = load_request_facts(agentup, "scope", request_id)?;
    let latest = scopes.iter().filter(|scope| scope["content"]["task_id"].as_str() == Some(task_id)).max_by(|a, b| {
        (
            a["updated_at"].as_str().unwrap_or(""),
            a["revision"].as_i64().unwrap_or(0),
            a["id"].as_str().unwrap_or(""),
        )
            .cmp(&(
                b["updated_at"].as_str().unwrap_or(""),
                b["revision"].as_i64().unwrap_or(0),
                b["id"].as_str().unwrap_or(""),
            ))
    });
    let Some(scope) = latest else {
        return Ok(HashSet::new());
    };
    let mut paths = HashSet::new();
    if let Some(entries) = scope["content"]["entries"].as_array() {
        for entry in entries {
            if entry["included"] == true {
                if let Some(path) = entry["relative_path"].as_str() {
                    paths.insert(path.to_string());
                }
            }
        }
    }
    Ok(paths)
}

fn resolve_project_path(root: &Path, relative_path: &str) -> Result<PathBuf, AppError> {
    if !valid_relative_path(relative_path) {
        return Err(AppError::with_details("invalid_input", "relative_path is invalid.", json!({"field": "relative_path"})));
    }
    let mut cur = root.to_path_buf();
    for segment in relative_path.split('/') {
        cur.push(segment);
        if let Ok(meta) = fs::symlink_metadata(&cur) {
            if meta.file_type().is_symlink() {
                return Err(AppError::code("path_outside_project", "The path is outside the selected project."));
            }
        }
    }
    if !cur.starts_with(root) {
        return Err(AppError::code("path_outside_project", "The path is outside the selected project."));
    }
    Ok(cur)
}

fn read_project_file(root: &Path, relative_path: &str) -> Result<String, AppError> {
    let path = resolve_project_path(root, relative_path)?;
    fs::read_to_string(&path).map_err(AppError::from_io)
}

fn write_project_file(root: &Path, relative_path: &str, content: &str) -> Result<(), AppError> {
    let path = resolve_project_path(root, relative_path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(AppError::from_io)?;
    }
    fs::write(&path, content.as_bytes()).map_err(AppError::from_io)
}
