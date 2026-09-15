impl Runtime {
    pub fn app_index_path(&self) -> Option<PathBuf> {
        self.app_data_dir.as_ref().map(|dir| dir.join("app-index.sqlite"))
    }

    pub fn list_projects(&mut self) -> Value {
        const CMD: &str = "list_projects";
        let rows = match load_registered(&self.app_index) {
            Ok(rows) => rows,
            Err(err) => return err.into_value(CMD),
        };
        let mut projects = Vec::new();
        for row in rows {
            let path_state = classify_path(Path::new(&row.project_path));
            let last_seen_at = if path_state == "ok" {
                now_rfc3339()
            } else {
                row.last_seen_at.clone()
            };
            if let Err(err) = update_registered_state(
                &self.app_index,
                &row.project_id,
                path_state,
                &last_seen_at,
            ) {
                return err.into_value(CMD);
            }
            projects.push(json!({
                "project_id": row.project_id,
                "project_path": row.project_path,
                "registered_at": row.registered_at,
                "last_seen_at": last_seen_at,
                "path_state": path_state
            }));
        }
        ok(CMD, json!({ "projects": projects }))
    }

    pub fn register_project(&mut self, project_id: &str, project_path: &str) -> Value {
        const CMD: &str = "register_project";
        if !valid_id(project_id) {
            return fail(
                CMD,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            );
        }
        let root = match require_canonical_dir(project_path) {
            Ok(path) => path,
            Err(err) => return err.into_value(CMD),
        };
        match self.projects.get(project_id) {
            Some(bound) if bound.root == root => {}
            Some(_) => {
                return fail(
                    CMD,
                    "path_outside_project",
                    "The provided path is not the bound project root.",
                    None,
                )
            }
            None => {
                return fail(
                    CMD,
                    "invalid_input",
                    "Project must be scanned before registration.",
                    Some(json!({"field": "project_id"})),
                )
            }
        }
        let now = now_rfc3339();
        let path_text = root.to_string_lossy().into_owned();
        if let Err(err) = insert_registered(
            &self.app_index,
            project_id,
            &path_text,
            &now,
            "ok",
        ) {
            return err.into_value(CMD);
        }
        ok(
            CMD,
            json!({
                "project_id": project_id,
                "project_path": path_text,
                "registered_at": now,
                "path_state": "ok"
            }),
        )
    }

    pub fn rebind_project(&mut self, project_id: &str, project_path: &str) -> Value {
        const CMD: &str = "rebind_project";
        if !valid_id(project_id) {
            return fail(
                CMD,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            );
        }
        let current = match load_registered_one(&self.app_index, project_id) {
            Ok(Some(row)) => row,
            Ok(None) => {
                return fail(
                    CMD,
                    "invalid_input",
                    "Project is not registered.",
                    Some(json!({"field": "project_id"})),
                )
            }
            Err(err) => return err.into_value(CMD),
        };
        if classify_path(Path::new(&current.project_path)) != "missing" {
            return fail(
                CMD,
                "invalid_input",
                "The original project path is still present.",
                Some(json!({"field": "project_path"})),
            );
        }
        let root = match require_canonical_dir(project_path) {
            Ok(path) => path,
            Err(err) => return err.into_value(CMD),
        };
        if let Err(err) = reject_agentup_symlink(&root) {
            return err.into_value(CMD);
        }
        if agentup_state(&root) != "present" {
            return fail(
                CMD,
                "not_initialized",
                "The rebound path has no valid AgentUp manifest.",
                None,
            );
        }
        let manifest = match read_json(&agentup_dir(&root).join("manifest.json")) {
            Ok(value) => value,
            Err(err) => return err.into_value(CMD),
        };
        let manifest_id = manifest["project_id"].as_str().unwrap_or("");
        if manifest_id != project_id {
            return fail(
                CMD,
                "invalid_input",
                "Manifest project_id does not match the registered project.",
                Some(json!({"field": "project_id"})),
            );
        }
        let path_text = root.to_string_lossy().into_owned();
        let now = now_rfc3339();
        if let Err(err) = rebind_registered(&self.app_index, project_id, &path_text, &now) {
            return err.into_value(CMD);
        }
        self.projects
            .insert(project_id.to_string(), BoundProject { root });
        ok(
            CMD,
            json!({
                "project_id": project_id,
                "project_path": path_text,
                "path_state": "ok"
            }),
        )
    }

    pub fn mint_remove_confirmation(
        &mut self,
        project_id: &str,
        project_path: &str,
    ) -> Result<String, Value> {
        const CMD: &str = "remove_agentup";
        if !valid_id(project_id) {
            return Err(fail(
                CMD,
                "invalid_input",
                "project_id is invalid.",
                Some(json!({"field": "project_id"})),
            ));
        }
        let root = match self.bind_existing(project_id, project_path) {
            Ok(path) => path,
            Err(err) => return Err(err.into_value(CMD)),
        };
        if let Err(err) = reject_agentup_symlink(&root) {
            return Err(err.into_value(CMD));
        }
        if !agentup_dir(&root).is_dir() {
            return Err(fail(
                CMD,
                "not_initialized",
                "The project has not been initialized.",
                None,
            ));
        }
        let token = format!("tok-{}", random_hex(16));
        self.tokens.insert(
            token.clone(),
            Confirmation {
                kind: ConfirmationKind::Remove,
                project_id: project_id.to_string(),
                root,
                fingerprint: String::new(),
                planned_paths: vec![".agentup".to_string()],
                event_id: String::new(),
                manifest_id: String::new(),
            },
        );
        Ok(token)
    }

    pub fn remove_agentup(
        &mut self,
        project_id: &str,
        project_path: &str,
        confirmation_token: &str,
    ) -> Value {
        const CMD: &str = "remove_agentup";
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
                "Removing AgentUp requires a confirmation token.",
                None,
            );
        }
        let root = match self.bind_existing(project_id, project_path) {
            Ok(path) => path,
            Err(err) => return err.into_value(CMD),
        };
        let Some(confirmation) = self.tokens.get(confirmation_token).cloned() else {
            return fail(
                CMD,
                "confirmation_expired",
                "The confirmation token is unknown or no longer valid.",
                None,
            );
        };
        if confirmation.kind != ConfirmationKind::Remove
            || confirmation.project_id != project_id
            || confirmation.root != root
        {
            return fail(
                CMD,
                "confirmation_expired",
                "The confirmation token is not bound to this removal.",
                None,
            );
        }
        self.tokens.remove(confirmation_token);
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
        let stamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let backup_name = format!(".agentup.backup.{stamp}");
        let backup = root.join(&backup_name);
        if backup.exists() {
            return fail(
                CMD,
                "already_exists",
                "A backup directory with this name already exists.",
                None,
            );
        }
        if let Err(err) = copy_dir_recursive(&agentup, &backup) {
            let _ = fs::remove_dir_all(&backup);
            return err.into_value(CMD);
        }
        if let Err(_) = fs::remove_dir_all(&agentup) {
            return fail(
                CMD,
                "io_error",
                "The AgentUp directory could not be removed after backup.",
                None,
            );
        }
        ok(
            CMD,
            json!({
                "project_id": project_id,
                "backup_relative_path": backup_name
            }),
        )
    }
}

struct RegisteredRow {
    project_id: String,
    project_path: String,
    registered_at: String,
    last_seen_at: String,
}

fn open_app_index(dir: Option<&Path>) -> Result<Connection, String> {
    let conn = match dir {
        Some(dir) => {
            fs::create_dir_all(dir).map_err(|err| err.to_string())?;
            Connection::open(dir.join("app-index.sqlite")).map_err(|err| err.to_string())?
        }
        None => Connection::open_in_memory().map_err(|err| err.to_string())?,
    };
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS registered_projects (
            project_id TEXT PRIMARY KEY,
            project_path TEXT NOT NULL UNIQUE,
            registered_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            path_state TEXT NOT NULL
        );",
    )
    .map_err(|err| err.to_string())?;
    Ok(conn)
}

fn map_index_error(err: rusqlite::Error) -> AppError {
    match err {
        rusqlite::Error::SqliteFailure(info, _)
            if info.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            AppError::code("already_exists", "The project is already registered.")
        }
        _ => AppError::code("io_error", "The app project index could not be updated."),
    }
}

fn load_registered(conn: &Connection) -> Result<Vec<RegisteredRow>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT project_id, project_path, registered_at, last_seen_at
             FROM registered_projects
             ORDER BY registered_at ASC, project_id ASC",
        )
        .map_err(map_index_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(RegisteredRow {
                project_id: row.get(0)?,
                project_path: row.get(1)?,
                registered_at: row.get(2)?,
                last_seen_at: row.get(3)?,
            })
        })
        .map_err(map_index_error)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(map_index_error)?);
    }
    Ok(out)
}

fn load_registered_one(conn: &Connection, project_id: &str) -> Result<Option<RegisteredRow>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT project_id, project_path, registered_at, last_seen_at
             FROM registered_projects
             WHERE project_id = ?1",
        )
        .map_err(map_index_error)?;
    let mut rows = stmt
        .query(params![project_id])
        .map_err(map_index_error)?;
    match rows.next().map_err(map_index_error)? {
        Some(row) => Ok(Some(RegisteredRow {
            project_id: row.get(0).map_err(map_index_error)?,
            project_path: row.get(1).map_err(map_index_error)?,
            registered_at: row.get(2).map_err(map_index_error)?,
            last_seen_at: row.get(3).map_err(map_index_error)?,
        })),
        None => Ok(None),
    }
}

fn insert_registered(
    conn: &Connection,
    project_id: &str,
    project_path: &str,
    now: &str,
    path_state: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO registered_projects (
            project_id, project_path, registered_at, last_seen_at, path_state
        ) VALUES (?1, ?2, ?3, ?3, ?4)",
        params![project_id, project_path, now, path_state],
    )
    .map(|_| ())
    .map_err(map_index_error)
}

fn update_registered_state(
    conn: &Connection,
    project_id: &str,
    path_state: &str,
    last_seen_at: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE registered_projects
         SET path_state = ?1, last_seen_at = ?2
         WHERE project_id = ?3",
        params![path_state, last_seen_at, project_id],
    )
    .map(|_| ())
    .map_err(map_index_error)
}

fn rebind_registered(
    conn: &Connection,
    project_id: &str,
    project_path: &str,
    now: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE registered_projects
         SET project_path = ?1, path_state = 'ok', last_seen_at = ?2
         WHERE project_id = ?3",
        params![project_path, now, project_id],
    )
    .map(|_| ())
    .map_err(map_index_error)
}

fn classify_path(path: &Path) -> &'static str {
    match fs::symlink_metadata(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => "missing",
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => "unreadable",
        Err(_) => "unreadable",
        Ok(meta) => {
            if meta.file_type().is_symlink() || meta.is_dir() {
                match fs::canonicalize(path) {
                    Ok(canonical) if canonical.is_dir() => "ok",
                    Ok(_) => "not_dir",
                    Err(err) if err.kind() == io::ErrorKind::NotFound => "missing",
                    Err(err) if err.kind() == io::ErrorKind::PermissionDenied => "unreadable",
                    Err(_) => "unreadable",
                }
            } else {
                "not_dir"
            }
        }
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), AppError> {
    fs::create_dir(dst).map_err(AppError::from_io)?;
    for entry in fs::read_dir(src).map_err(AppError::from_io)? {
        let entry = entry.map_err(AppError::from_io)?;
        let src_child = entry.path();
        let dst_child = dst.join(entry.file_name());
        let meta = fs::symlink_metadata(&src_child).map_err(AppError::from_io)?;
        if meta.file_type().is_symlink() {
            let target = fs::read_link(&src_child).map_err(AppError::from_io)?;
            std::os::unix::fs::symlink(&target, &dst_child).map_err(AppError::from_io)?;
        } else if meta.is_dir() {
            copy_dir_recursive(&src_child, &dst_child)?;
        } else {
            fs::copy(&src_child, &dst_child).map_err(AppError::from_io)?;
        }
    }
    Ok(())
}
