impl Runtime {
    pub fn post_discussion(
        &mut self,
        project_id: &str,
        request_id: &str,
        body: &str,
        attachment_ids: Option<Vec<String>>,
    ) -> Value {
        const CMD: &str = "post_discussion";
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
        if body.is_empty() || body.len() > 100_000 {
            return fail(
                CMD,
                "invalid_input",
                "body must be between 1 and 100000 UTF-8 bytes.",
                Some(json!({"field": "body"})),
            );
        }
        if let Some(ids) = &attachment_ids {
            for id in ids {
                if !valid_id(id) {
                    return fail(
                        CMD,
                        "invalid_input",
                        "attachment_ids contains an invalid id.",
                        Some(json!({"field": "attachment_ids"})),
                    );
                }
            }
        }
        if let Err(err) = require_bound_request(self, project_id, request_id) {
            return err.into_value(CMD);
        }
        let discussion_id = format!("disc-{}", random_hex(8));
        let mut content = json!({
            "request_id": request_id,
            "author_source": "user",
            "body": body,
            "posted_at": now_rfc3339()
        });
        if let Some(ids) = attachment_ids {
            if !ids.is_empty() {
                content["attachment_ids"] = json!(ids);
            }
        }
        remap_command(
            self.write_fact(
                project_id,
                &discussion_id,
                "discussion",
                content,
                json!({}),
                0,
                None,
            ),
            CMD,
        )
    }

    pub fn add_attachment(
        &mut self,
        project_id: &str,
        request_id: &str,
        media_type: &str,
        bytes: &[u8],
    ) -> Value {
        const CMD: &str = "add_attachment";
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
        if media_type.is_empty() || media_type.len() > 200 {
            return fail(
                CMD,
                "invalid_input",
                "media_type is invalid.",
                Some(json!({"field": "media_type"})),
            );
        }
        if bytes.len() as i64 > MAX_ATTACHMENT_BYTES {
            return fail(
                CMD,
                "invalid_input",
                "Attachment exceeds the single-file size limit.",
                Some(json!({"field": "bytes"})),
            );
        }
        let root = match require_bound_request(self, project_id, request_id) {
            Ok(root) => root,
            Err(err) => return err.into_value(CMD),
        };
        let agentup = agentup_dir(&root);
        let attachment_id = format!("att-{}", random_hex(8));
        let relative_path = format!("attachments/{attachment_id}");
        let sha = sha256_hex(bytes);
        let byte_length = bytes.len() as i64;
        let content = json!({
            "request_id": request_id,
            "relative_path": relative_path,
            "media_type": media_type,
            "byte_length": byte_length,
            "sha256": sha
        });
        let facts_dir = agentup.join("facts").join("attachments");
        if let Err(err) =
            reject_attachment_limits(&facts_dir, "attachment", &attachment_id, &content)
        {
            return err.into_value(CMD);
        }
        let bytes_dir = agentup.join("attachments");
        let bytes_path = bytes_dir.join(&attachment_id);
        if let Err(err) = write_attachment_bytes(&bytes_path, bytes) {
            return err.into_value(CMD);
        }
        let written = self.write_fact(
            project_id,
            &attachment_id,
            "attachment",
            content,
            json!({}),
            0,
            None,
        );
        if written["ok"] != true {
            let _ = fs::remove_file(&bytes_path);
            let _ = fs::remove_dir(&bytes_dir);
            return remap_command(written, CMD);
        }
        ok(
            CMD,
            json!({
                "fact": written["data"]["fact"].clone(),
                "relative_path": relative_path,
                "byte_length": byte_length,
                "sha256": sha
            }),
        )
    }

    pub fn load_request_thread(&self, project_id: &str, request_id: &str) -> Value {
        const CMD: &str = "load_request_thread";
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
        let discussions = match load_request_facts(&agentup, "discussion", request_id) {
            Ok(mut facts) => {
                facts.sort_by(|a, b| {
                    (
                        a["content"]["posted_at"].as_str().unwrap_or(""),
                        a["id"].as_str().unwrap_or(""),
                    )
                        .cmp(&(
                            b["content"]["posted_at"].as_str().unwrap_or(""),
                            b["id"].as_str().unwrap_or(""),
                        ))
                });
                facts
            }
            Err(err) => return err.into_value(CMD),
        };
        let attachments = match load_request_facts(&agentup, "attachment", request_id) {
            Ok(mut facts) => {
                facts.sort_by(|a, b| {
                    (
                        a["created_at"].as_str().unwrap_or(""),
                        a["id"].as_str().unwrap_or(""),
                    )
                        .cmp(&(
                            b["created_at"].as_str().unwrap_or(""),
                            b["id"].as_str().unwrap_or(""),
                        ))
                });
                facts
                    .into_iter()
                    .map(|fact| {
                        json!({
                            "id": fact["id"],
                            "request_id": fact["request_id"],
                            "relative_path": fact["content"]["relative_path"],
                            "media_type": fact["content"]["media_type"],
                            "byte_length": fact["content"]["byte_length"],
                            "sha256": fact["content"]["sha256"]
                        })
                    })
                    .collect::<Vec<_>>()
            }
            Err(err) => return err.into_value(CMD),
        };
        ok(
            CMD,
            json!({
                "project_id": project_id,
                "request_id": request_id,
                "discussions": discussions,
                "attachments": attachments
            }),
        )
    }
}

fn require_bound_request(
    runtime: &Runtime,
    project_id: &str,
    request_id: &str,
) -> Result<PathBuf, AppError> {
    let Some(bound) = runtime.projects.get(project_id) else {
        return Err(AppError::code(
            "not_initialized",
            "No bound initialized project was found for this project_id.",
        ));
    };
    let root = bound.root.clone();
    reject_agentup_symlink(&root)?;
    let agentup = agentup_dir(&root);
    if !agentup.is_dir() {
        return Err(AppError::code(
            "not_initialized",
            "The project has not been initialized.",
        ));
    }
    let request_path = agentup
        .join("facts")
        .join("requests")
        .join(format!("{request_id}.json"));
    if !request_path.is_file() {
        return Err(AppError::with_details(
            "invalid_input",
            "request_id does not match a stored request.",
            json!({"field": "request_id"}),
        ));
    }
    Ok(root)
}

fn load_request_facts(
    agentup: &Path,
    fact_type: &str,
    request_id: &str,
) -> Result<Vec<Value>, AppError> {
    let Some(dir_name) = fact_dir_name(fact_type) else {
        return Err(AppError::with_details(
            "invalid_input",
            "fact_type is not a supported M1 fact type.",
            json!({"field": "fact_type"}),
        ));
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
            return Err(AppError::code(
                "malformed_fact",
                "A stored fact record is malformed.",
            ));
        }
        if fact["request_id"].as_str() != Some(request_id) {
            continue;
        }
        facts.push(fact);
    }
    Ok(facts)
}

fn split_fact_revision_name(name: &str) -> Option<(String, i64)> {
    let stem = name.strip_suffix(".json")?;
    let (fact_id, rest) = stem.rsplit_once(".r")?;
    if fact_id.is_empty() || rest.is_empty() || !rest.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let revision = rest.parse().ok().filter(|revision| *revision >= 1)?;
    Some((fact_id.to_string(), revision))
}

fn write_attachment_bytes(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    if path.exists() {
        return Err(AppError::code(
            "already_exists",
            "An attachment file with this id already exists.",
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        AppError::code(
            "io_error",
            "The command failed because of a safe I/O error.",
        )
    })?;
    fs::create_dir_all(parent).map_err(AppError::from_io)?;
    let tmp = path.with_file_name(format!(
        "{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("attachment")
    ));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(AppError::from_io)?;
        file.write_all(bytes).map_err(AppError::from_io)?;
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

fn remap_command(mut value: Value, command: &str) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.insert("command".to_string(), json!(command));
    }
    value
}
