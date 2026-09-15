impl Runtime {
    pub fn publish_result(
        &mut self,
        project_id: &str,
        result_id: &str,
        request_id: &str,
        summary: &str,
        evidence_paths: Option<Vec<String>>,
    ) -> Value {
        const CMD: &str = "publish_result";
        if let Err(err) = require_ids(project_id, result_id, request_id) {
            return err.into_value(CMD);
        }
        if summary.is_empty() || summary.len() > 10_000 {
            return fail(CMD, "invalid_input", "summary is invalid.", Some(json!({"field": "summary"})));
        }
        let paths = evidence_paths.unwrap_or_default();
        for path in &paths {
            if !valid_relative_path(path) {
                return fail(CMD, "invalid_input", "evidence_paths is invalid.", Some(json!({"field": "evidence_paths"})));
            }
        }
        let root = match require_bound_request(self, project_id, request_id) {
            Ok(root) => root,
            Err(err) => return err.into_value(CMD),
        };
        let version = match next_result_version(&agentup_dir(&root), request_id) {
            Ok(v) => v,
            Err(err) => return err.into_value(CMD),
        };
        let mut content = json!({
            "request_id": request_id,
            "version": version,
            "summary": summary,
            "acceptance": "pending"
        });
        if !paths.is_empty() {
            content["evidence_paths"] = json!(paths);
        }
        remap_command(self.write_fact(project_id, result_id, "result", content, json!({}), 0, None), CMD)
    }

    pub fn accept_result(
        &mut self,
        project_id: &str,
        request_id: &str,
        source_result_id: &str,
        result_id: &str,
    ) -> Value {
        settle_result(self, "accept_result", project_id, request_id, source_result_id, result_id, "accepted")
    }

    pub fn reject_result(
        &mut self,
        project_id: &str,
        request_id: &str,
        source_result_id: &str,
        result_id: &str,
    ) -> Value {
        settle_result(self, "reject_result", project_id, request_id, source_result_id, result_id, "rejected")
    }

    pub fn submit_feedback(
        &mut self,
        project_id: &str,
        request_id: &str,
        body: &str,
        title: Option<String>,
    ) -> Value {
        const CMD: &str = "submit_feedback";
        let posted = self.post_discussion(project_id, request_id, body, None);
        if posted["ok"] != true {
            return remap_command(posted, CMD);
        }
        if let Some(title) = title {
            if title.is_empty() {
                return fail(CMD, "invalid_input", "title is invalid.", Some(json!({"field": "title"})));
            }
            let task_id = format!("task-fb-{}", random_hex(6));
            let task = self.put_task(project_id, &task_id, request_id, &title, None, None, 0, None);
            if task["ok"] != true {
                return remap_command(task, CMD);
            }
            return ok(CMD, json!({"discussion": posted["data"], "task": task["data"]}));
        }
        remap_command(posted, CMD)
    }

    pub fn load_result_history(&self, project_id: &str, request_id: &str) -> Value {
        const CMD: &str = "load_result_history";
        let root = match require_bound_request(self, project_id, request_id) {
            Ok(root) => root,
            Err(err) => return err.into_value(CMD),
        };
        let mut facts = match load_request_facts(&agentup_dir(&root), "result", request_id) {
            Ok(facts) => facts,
            Err(err) => return err.into_value(CMD),
        };
        facts.sort_by(|a, b| a["content"]["version"].as_i64().unwrap_or(0).cmp(&b["content"]["version"].as_i64().unwrap_or(0)));
        ok(CMD, json!({"request_id": request_id, "results": facts}))
    }
}

fn require_ids(project_id: &str, result_id: &str, request_id: &str) -> Result<(), AppError> {
    if !valid_id(project_id) {
        return Err(AppError::with_details("invalid_input", "project_id is invalid.", json!({"field": "project_id"})));
    }
    if !valid_id(result_id) {
        return Err(AppError::with_details("invalid_input", "result_id is invalid.", json!({"field": "result_id"})));
    }
    if !valid_id(request_id) {
        return Err(AppError::with_details("invalid_input", "request_id is invalid.", json!({"field": "request_id"})));
    }
    Ok(())
}

fn next_result_version(agentup: &Path, request_id: &str) -> Result<i64, AppError> {
    let facts = load_request_facts(agentup, "result", request_id)?;
    let max = facts.iter().filter_map(|f| f["content"]["version"].as_i64()).max().unwrap_or(0);
    Ok(max + 1)
}

fn settle_result(
    runtime: &mut Runtime,
    command: &str,
    project_id: &str,
    request_id: &str,
    source_result_id: &str,
    result_id: &str,
    acceptance: &str,
) -> Value {
    if let Err(err) = require_ids(project_id, result_id, request_id) {
        return err.into_value(command);
    }
    if !valid_id(source_result_id) {
        return fail(command, "invalid_input", "source_result_id is invalid.", Some(json!({"field": "source_result_id"})));
    }
    let root = match require_bound_request(runtime, project_id, request_id) {
        Ok(root) => root,
        Err(err) => return err.into_value(command),
    };
    let agentup = agentup_dir(&root);
    let source = match latest_fact(&agentup.join("facts").join("results"), source_result_id) {
        Ok(fact) => fact,
        Err(err) => return err.into_value(command),
    };
    if source["content"]["request_id"].as_str() != Some(request_id) {
        return fail(command, "invalid_input", "source_result_id does not belong to this request.", Some(json!({"field": "source_result_id"})));
    }
    let version = match next_result_version(&agentup, request_id) {
        Ok(v) => v,
        Err(err) => return err.into_value(command),
    };
    let mut content = json!({
        "request_id": request_id,
        "version": version,
        "summary": source["content"]["summary"],
        "acceptance": acceptance
    });
    if let Some(paths) = source["content"].get("evidence_paths") {
        content["evidence_paths"] = paths.clone();
    }
    let written = remap_command(runtime.write_fact(project_id, result_id, "result", content, json!({}), 0, None), command);
    if written["ok"] != true {
        return written;
    }
    let decision_id = format!("dec-{result_id}");
    let chosen = if acceptance == "accepted" { "accepted" } else { "rejected" };
    let decision = runtime.write_fact(
        project_id,
        &decision_id,
        "decision",
        json!({
            "request_id": request_id,
            "question": "Accept result",
            "options": ["accepted", "rejected"],
            "status": "chosen",
            "chosen": chosen
        }),
        json!({}),
        0,
        None,
    );
    if decision["ok"] != true {
        return remap_command(decision, command);
    }
    written
}
