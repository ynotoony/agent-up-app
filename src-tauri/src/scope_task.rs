impl Runtime {
    pub fn put_scope(
        &mut self,
        project_id: &str,
        scope_id: &str,
        request_id: &str,
        task_id: &str,
        entries: Value,
        expected_revision: i64,
    ) -> Value {
        const CMD: &str = "put_scope";
        if !valid_id(project_id) {
            return fail(
                CMD,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            );
        }
        if !valid_id(scope_id) {
            return fail(
                CMD,
                "invalid_input",
                "scope_id is invalid.",
                Some(json!({"field": "scope_id"})),
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
        if !valid_id(task_id) {
            return fail(
                CMD,
                "invalid_input",
                "task_id is invalid.",
                Some(json!({"field": "task_id"})),
            );
        }
        if expected_revision < 0 {
            return fail(
                CMD,
                "invalid_input",
                "expected_revision is invalid.",
                Some(json!({"field": "expected_revision"})),
            );
        }
        let Some(entries_arr) = entries.as_array() else {
            return fail(
                CMD,
                "invalid_input",
                "entries must be an array.",
                Some(json!({"field": "entries"})),
            );
        };
        for entry in entries_arr {
            let Some(path) = entry.get("relative_path").and_then(Value::as_str) else {
                return fail(
                    CMD,
                    "invalid_input",
                    "entries require relative_path.",
                    Some(json!({"field": "relative_path"})),
                );
            };
            if !valid_relative_path(path) {
                return fail(
                    CMD,
                    "invalid_input",
                    "relative_path is invalid.",
                    Some(json!({"field": "relative_path"})),
                );
            }
        }
        remap_command(
            self.write_fact(
                project_id,
                scope_id,
                "scope",
                json!({
                    "request_id": request_id,
                    "task_id": task_id,
                    "entries": entries
                }),
                json!({}),
                expected_revision,
                None,
            ),
            CMD,
        )
    }

    pub fn diff_scope(&self, scope_id: &str, from_revision: i64, to_revision: i64) -> Value {
        const CMD: &str = "diff_scope";
        if !valid_id(scope_id) {
            return fail(
                CMD,
                "invalid_input",
                "scope_id is invalid.",
                Some(json!({"field": "scope_id"})),
            );
        }
        if from_revision < 1 {
            return fail(
                CMD,
                "invalid_input",
                "from_revision is invalid.",
                Some(json!({"field": "from_revision"})),
            );
        }
        if to_revision < 1 {
            return fail(
                CMD,
                "invalid_input",
                "to_revision is invalid.",
                Some(json!({"field": "to_revision"})),
            );
        }
        let project_id = match find_project_for_fact(self, "scope", scope_id) {
            Ok(id) => id,
            Err(err) => return err.into_value(CMD),
        };
        let bound = match self.projects.get(&project_id) {
            Some(bound) => bound,
            None => {
                return fail(
                    CMD,
                    "not_initialized",
                    "No bound initialized project was found for this project_id.",
                    None,
                )
            }
        };
        let dir = agentup_dir(&bound.root).join("facts").join("scopes");
        let from = match read_fact_at_revision(&dir, scope_id, from_revision) {
            Ok(fact) => fact,
            Err(err) => return err.into_value(CMD),
        };
        let to = match read_fact_at_revision(&dir, scope_id, to_revision) {
            Ok(fact) => fact,
            Err(err) => return err.into_value(CMD),
        };
        let from_map = match scope_entry_map(&from) {
            Ok(map) => map,
            Err(err) => return err.into_value(CMD),
        };
        let to_map = match scope_entry_map(&to) {
            Ok(map) => map,
            Err(err) => return err.into_value(CMD),
        };
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();
        for path in to_map.keys() {
            match from_map.get(path) {
                None => added.push(path.clone()),
                Some(prev) if prev != &to_map[path] => changed.push(path.clone()),
                Some(_) => {}
            }
        }
        for path in from_map.keys() {
            if !to_map.contains_key(path) {
                removed.push(path.clone());
            }
        }
        added.sort();
        removed.sort();
        changed.sort();
        ok(
            CMD,
            json!({
                "scope_id": scope_id,
                "from_revision": from_revision,
                "to_revision": to_revision,
                "added": added,
                "removed": removed,
                "changed": changed
            }),
        )
    }

    pub fn put_task(
        &mut self,
        project_id: &str,
        task_id: &str,
        request_id: &str,
        title: &str,
        parent_id: Option<&str>,
        depends_on: Option<Vec<String>>,
        expected_revision: i64,
        content: Option<Value>,
    ) -> Value {
        const CMD: &str = "put_task";
        if let Some(content) = &content {
            if content.get("task_state").is_some() {
                return fail(
                    CMD,
                    "invalid_input",
                    "task_state cannot be set through put_task content.",
                    Some(json!({"field": "content"})),
                );
            }
        }
        if !valid_id(project_id) {
            return fail(
                CMD,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            );
        }
        if !valid_id(task_id) {
            return fail(
                CMD,
                "invalid_input",
                "task_id is invalid.",
                Some(json!({"field": "task_id"})),
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
        if title.is_empty() || title.len() > 500 {
            return fail(
                CMD,
                "invalid_input",
                "title is invalid.",
                Some(json!({"field": "title"})),
            );
        }
        if let Some(parent_id) = parent_id {
            if !valid_id(parent_id) {
                return fail(
                    CMD,
                    "invalid_input",
                    "parent_id is invalid.",
                    Some(json!({"field": "parent_id"})),
                );
            }
        }
        if let Some(ids) = &depends_on {
            for id in ids {
                if !valid_id(id) {
                    return fail(
                        CMD,
                        "invalid_input",
                        "depends_on contains an invalid id.",
                        Some(json!({"field": "depends_on"})),
                    );
                }
            }
        }
        if expected_revision < 0 {
            return fail(
                CMD,
                "invalid_input",
                "expected_revision is invalid.",
                Some(json!({"field": "expected_revision"})),
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
        if let Err(err) = reject_agentup_symlink(&bound.root) {
            return err.into_value(CMD);
        }
        let facts_dir = agentup_dir(&bound.root).join("facts").join("tasks");
        let current_revision = match current_fact_revision(&facts_dir, task_id) {
            Ok(rev) => rev,
            Err(err) => return err.into_value(CMD),
        };
        let task_state = if current_revision == 0 {
            "ready".to_string()
        } else {
            match latest_fact(&facts_dir, task_id) {
                Ok(previous) => previous["content"]["task_state"]
                    .as_str()
                    .unwrap_or("ready")
                    .to_string(),
                Err(err) => return err.into_value(CMD),
            }
        };
        let mut fact_content = json!({
            "request_id": request_id,
            "title": title,
            "task_state": task_state,
            "parent_id": parent_id
        });
        if let Some(ids) = depends_on {
            fact_content["depends_on"] = json!(ids);
        }
        remap_command(
            self.write_fact(
                project_id,
                task_id,
                "task",
                fact_content,
                json!({}),
                expected_revision,
                None,
            ),
            CMD,
        )
    }

    pub fn set_task_state(
        &mut self,
        task_id: &str,
        task_state: &str,
        expected_revision: i64,
    ) -> Value {
        const CMD: &str = "set_task_state";
        if !valid_id(task_id) {
            return fail(
                CMD,
                "invalid_input",
                "task_id is invalid.",
                Some(json!({"field": "task_id"})),
            );
        }
        if !valid_task_state(task_state) {
            return fail(
                CMD,
                "invalid_input",
                "task_state is invalid.",
                Some(json!({"field": "task_state"})),
            );
        }
        if expected_revision < 0 {
            return fail(
                CMD,
                "invalid_input",
                "expected_revision is invalid.",
                Some(json!({"field": "expected_revision"})),
            );
        }
        let project_id = match find_project_for_fact(self, "task", task_id) {
            Ok(id) => id,
            Err(err) => return err.into_value(CMD),
        };
        let bound = match self.projects.get(&project_id).cloned() {
            Some(bound) => bound,
            None => {
                return fail(
                    CMD,
                    "not_initialized",
                    "No bound initialized project was found for this project_id.",
                    None,
                )
            }
        };
        if let Err(err) = reject_agentup_symlink(&bound.root) {
            return err.into_value(CMD);
        }
        let facts_dir = agentup_dir(&bound.root).join("facts").join("tasks");
        let previous = match latest_fact(&facts_dir, task_id) {
            Ok(fact) => fact,
            Err(err) => return err.into_value(CMD),
        };
        let mut content = previous["content"].clone();
        content["task_state"] = json!(task_state);
        let metadata = previous
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json!({}));
        remap_command(
            self.write_fact(
                &project_id,
                task_id,
                "task",
                content,
                metadata,
                expected_revision,
                None,
            ),
            CMD,
        )
    }

    pub fn load_board(&self, project_id: &str, request_id: &str) -> Value {
        const CMD: &str = "load_board";
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
        let root = match require_bound_request(self, project_id, request_id) {
            Ok(root) => root,
            Err(err) => return err.into_value(CMD),
        };
        let agentup = agentup_dir(&root);
        let mut tasks = match load_request_facts(&agentup, "task", request_id) {
            Ok(facts) => facts,
            Err(err) => return err.into_value(CMD),
        };
        let scopes = match load_request_facts(&agentup, "scope", request_id) {
            Ok(facts) => facts,
            Err(err) => return err.into_value(CMD),
        };
        tasks.sort_by(|a, b| {
            a["id"]
                .as_str()
                .unwrap_or("")
                .cmp(b["id"].as_str().unwrap_or(""))
        });
        let board_tasks = tasks
            .into_iter()
            .map(|task| {
                let task_id = task["id"].as_str().unwrap_or("").to_string();
                let latest_scope = scopes
                    .iter()
                    .filter(|scope| scope["content"]["task_id"].as_str() == Some(task_id.as_str()))
                    .max_by(|a, b| {
                        let left = (
                            a["updated_at"].as_str().unwrap_or(""),
                            a["revision"].as_i64().unwrap_or(0),
                            a["id"].as_str().unwrap_or(""),
                        );
                        let right = (
                            b["updated_at"].as_str().unwrap_or(""),
                            b["revision"].as_i64().unwrap_or(0),
                            b["id"].as_str().unwrap_or(""),
                        );
                        left.cmp(&right)
                    });
                let depends_on = match &task["content"]["depends_on"] {
                    Value::Array(ids) => Value::Array(ids.clone()),
                    _ => json!([]),
                };
                json!({
                    "task_id": task_id,
                    "title": task["content"]["title"],
                    "parent_id": task["content"]["parent_id"],
                    "depends_on": depends_on,
                    "task_state": task["content"]["task_state"],
                    "revision": task["revision"],
                    "scope": latest_scope.map(|scope| json!({
                        "scope_id": scope["id"],
                        "revision": scope["revision"],
                        "entries": scope["content"]["entries"]
                    }))
                })
            })
            .collect::<Vec<_>>();
        ok(
            CMD,
            json!({
                "project_id": project_id,
                "request_id": request_id,
                "tasks": board_tasks
            }),
        )
    }
}

fn valid_task_state(value: &str) -> bool {
    matches!(
        value,
        "ready"
            | "in_progress"
            | "blocked"
            | "review_ready"
            | "review_pass"
            | "review_fail"
            | "done"
    )
}

fn find_project_for_fact(
    runtime: &Runtime,
    fact_type: &str,
    fact_id: &str,
) -> Result<String, AppError> {
    if runtime.projects.is_empty() {
        return Err(AppError::code(
            "not_initialized",
            "No bound initialized project was found for this project_id.",
        ));
    }
    let Some(dir_name) = fact_dir_name(fact_type) else {
        return Err(AppError::with_details(
            "invalid_input",
            "fact_type is not a supported M1 fact type.",
            json!({"field": "fact_type"}),
        ));
    };
    let mut found = None;
    for (project_id, bound) in &runtime.projects {
        reject_agentup_symlink(&bound.root)?;
        let agentup = agentup_dir(&bound.root);
        if !agentup.is_dir() {
            continue;
        }
        let revision = current_fact_revision(&agentup.join("facts").join(dir_name), fact_id)?;
        if revision > 0 {
            if found.is_some() {
                return Err(AppError::with_details(
                    "invalid_input",
                    "The requested fact id matched more than one bound project.",
                    json!({"field": "fact_id"}),
                ));
            }
            found = Some(project_id.clone());
        }
    }
    found.ok_or_else(|| {
        AppError::with_details(
            "invalid_input",
            "The requested fact was not found.",
            json!({"field": "fact_id"}),
        )
    })
}

fn read_fact_at_revision(dir: &Path, fact_id: &str, revision: i64) -> Result<Value, AppError> {
    let path = dir.join(format!("{fact_id}.r{revision}.json"));
    if !path.is_file() {
        return Err(AppError::with_details(
            "invalid_input",
            "The requested fact revision was not found.",
            json!({"revision": revision}),
        ));
    }
    let fact = read_json(&path)?;
    if fact["id"].as_str() != Some(fact_id) || fact["revision"].as_i64() != Some(revision) {
        return Err(AppError::code(
            "malformed_fact",
            "A stored fact record is malformed.",
        ));
    }
    Ok(fact)
}

fn scope_entry_map(fact: &Value) -> Result<HashMap<String, Value>, AppError> {
    let Some(entries) = fact["content"]["entries"].as_array() else {
        return Err(AppError::code(
            "malformed_fact",
            "A stored fact record is malformed.",
        ));
    };
    let mut map = HashMap::new();
    for entry in entries {
        let Some(path) = entry["relative_path"].as_str() else {
            return Err(AppError::code(
                "malformed_fact",
                "A stored fact record is malformed.",
            ));
        };
        map.insert(path.to_string(), entry.clone());
    }
    Ok(map)
}
