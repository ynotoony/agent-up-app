use agentup_harness_lib::types::{RequirementMode, RequirementStatus, StepType, TaskStatus};
use agentup_harness_lib::{db, orchestrator, project_init};
use rusqlite::params;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn temp_base(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agentup-test-{}-{}-{}",
        tag,
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("create temp base");
    dir
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, content).expect("write file");
}

fn app_state(base: &Path) -> Arc<db::AppState> {
    Arc::new(db::open_state(base).expect("open state"))
}

// ① 迁移幂等：同一目录重复 open_state 不炸、不重复加列。
#[test]
fn migration_is_idempotent() {
    let base = temp_base("migration");
    let _state = app_state(&base);
    let state2 = db::open_state(&base).expect("reopen state");
    let conn = state2.conn.lock().unwrap();
    let cols: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name = 'path'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cols, 1, "path 列应恰好出现一次");
}

// ② 旧数据搬迁：description 里的绝对路径 → path 归位、description 清空。
#[test]
fn migration_backfills_path_from_description() {
    let base = temp_base("backfill");
    let state = app_state(&base);
    {
        let conn = state.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, description, status, created_at, updated_at) VALUES ('p-old','legacy','/tmp/legacy-dir','active','2026-01-01','2026-01-01')",
            [],
        )
        .unwrap();
    }
    // 重新打开触发迁移
    let state2 = db::open_state(&base).expect("reopen");
    let conn = state2.conn.lock().unwrap();
    let (path, description): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT path, description FROM projects WHERE id = 'p-old'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(path.as_deref(), Some("/tmp/legacy-dir"));
    assert_eq!(description, None);
    drop(conn);
}

// ③ 初始化：空目录 → 建目录/git/种子；报告与治理数据齐全；目录零污染（不写治理文件）。
#[test]
fn init_empty_dir_seeds_governance() {
    let base = temp_base("init-empty");
    let project_dir = base.join("fresh-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let state = app_state(&base);

    let outcome =
        project_init::init_project(&state, &project_dir.display().to_string()).expect("init ok");
    assert!(!outcome.already_registered);
    assert_eq!(
        outcome.report.governance_seeded, 14,
        "规则 10 + 角色 3 + 待定 1（空目录无需求文档，仅验证命令待定）"
    );
    assert!(outcome.report.pending_open >= 1, "验证命令待定必须存在");

    // 目录零污染：只允许出现 .git（初始化自建），不得出现任何治理文件
    let entries: Vec<String> = std::fs::read_dir(&project_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        entries.iter().all(|n| n == ".git"),
        "目录应保持零污染，实际: {entries:?}"
    );
    assert!(!project_dir.join("AGENTS.md").exists());
    assert!(!project_dir.join("docs").exists());

    // 库内校验：规则/角色/待定齐备
    let conn = state.conn.lock().unwrap();
    let rules: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM governance_items WHERE project_id = ?1 AND kind = 'rule'",
            params![outcome.project.id],
            |r| r.get(0),
        )
        .unwrap();
    let roles: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM governance_items WHERE project_id = ?1 AND kind = 'role'",
            params![outcome.project.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rules, 10);
    assert_eq!(roles, 3);
    let canonical_project = project_dir.canonicalize().unwrap();
    assert_eq!(outcome.project.path.as_deref(), canonical_project.to_str());
}

#[cfg(unix)]
#[test]
fn init_resolves_symlink_root_to_canonical_directory() {
    use std::os::unix::fs::symlink;
    let base = temp_base("init-symlink");
    let real = base.join("real");
    let alias = base.join("alias");
    std::fs::create_dir_all(&real).unwrap();
    symlink(&real, &alias).unwrap();
    let state = app_state(&base);
    let outcome = project_init::init_project(&state, alias.to_str().unwrap()).unwrap();
    assert_eq!(
        outcome.project.path.as_deref(),
        Some(real.canonicalize().unwrap().to_str().unwrap())
    );
}

// ④ 重复初始化同一目录 → 幂等返回已有项目。
#[test]
fn init_same_dir_twice_returns_existing() {
    let base = temp_base("init-dup");
    let project_dir = base.join("dup-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let state = app_state(&base);

    let first =
        project_init::init_project(&state, &project_dir.display().to_string()).expect("first init");
    let second = project_init::init_project(&state, &project_dir.display().to_string())
        .expect("second init");
    assert!(!first.already_registered);
    assert!(second.already_registered);
    assert_eq!(first.project.id, second.project.id);

    // 种子不重复
    let conn = state.conn.lock().unwrap();
    let rules: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM governance_items WHERE project_id = ?1 AND kind = 'rule'",
            params![first.project.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rules, 10);
}

// ⑤ 全目录遍历 + 全量转需求：所有层级的文本文档都入库并转为需求（数量对得上）；
//    README/doc 同样转（是否真是需求由理解阶段判定）；reinit 不重复；需求被删后允许重建。
#[test]
fn scan_classify_and_missing_marking() {
    let base = temp_base("scan");
    let project_dir = base.join("scan-project");
    write_file(&project_dir.join("README.md"), "# 项目说明");
    write_file(&project_dir.join("notes.md"), "一般文档");
    write_file(
        &project_dir.join("AgentUp-Harness-PRD/01-总览.md"),
        "# PRD 总览\n需求内容",
    );
    write_file(&project_dir.join("docs/spec.md"), "规格说明");
    write_file(
        &project_dir.join("src/deep/nested.md"),
        "# 深层目录里的文档",
    );
    let state = app_state(&base);

    let outcome =
        project_init::init_project(&state, &project_dir.display().to_string()).expect("init");
    assert_eq!(
        outcome.report.requirements_created, 5,
        "全部 5 篇文本文档都应转为需求（数量对得上）"
    );
    assert_eq!(
        outcome.report.locally_archived, 0,
        "无完成信号 → 不应预归档"
    );
    assert_eq!(outcome.report.converted_requirement_ids.len(), 5);
    let conn = state.conn.lock().unwrap();
    let docs = db::list_project_docs(&conn, &outcome.project.id).unwrap();
    let kind_of = |rel: &str| {
        docs.iter()
            .find(|d| d.rel_path == rel)
            .map(|d| d.kind.clone())
    };
    assert_eq!(kind_of("README.md").as_deref(), Some("readme"));
    assert_eq!(
        kind_of("AgentUp-Harness-PRD/01-总览.md").as_deref(),
        Some("requirement"),
        "目录名含 prd 命中需求类"
    );
    assert_eq!(
        kind_of("docs/spec.md").as_deref(),
        Some("requirement"),
        "文件名含 spec 命中需求类"
    );
    assert_eq!(kind_of("notes.md").as_deref(), Some("doc"));
    assert!(
        docs.iter().any(|d| d.rel_path == "src/deep/nested.md"),
        "深层目录必须被遍历到"
    );
    assert_eq!(docs.len(), 5);
    let linked = docs.iter().filter(|d| d.requirement_id.is_some()).count();
    assert_eq!(linked, 5, "全部文本类文档都应记录 requirement_id 链接");
    for rid in &outcome.report.converted_requirement_ids {
        let requirement = db::get_requirement(&conn, rid)
            .unwrap()
            .expect("requirement exists");
        assert!(
            requirement
                .content
                .as_deref()
                .unwrap_or_default()
                .contains("来自存量文档"),
            "需求内容应标注文档出处"
        );
    }
    drop(conn);

    // 外部删除后重新初始化 → file_missing 标记；无新增文档 → 不重复转换
    std::fs::remove_file(project_dir.join("notes.md")).unwrap();
    let report = project_init::reinit_project(&state, &outcome.project.id).expect("reinit");
    assert_eq!(report.docs_missing, 1);
    assert_eq!(
        report.requirements_created, 0,
        "已转换过的文档不得重复转需求"
    );
    let conn = state.conn.lock().unwrap();
    let (missing, content): (i64, Option<String>) = conn
        .query_row(
            "SELECT file_missing, content FROM project_docs WHERE project_id = ?1 AND rel_path = 'notes.md'",
            params![outcome.project.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(missing, 1);
    assert_eq!(
        content.as_deref(),
        Some("一般文档"),
        "数据在库：文件删除后记录与全文保留"
    );

    // 需求被删除后，重新初始化允许该文档再次转换（链接失效自动重建）。
    // 注意：只能删「文件仍在盘上」的文档对应需求——notes.md 文件已删，缺失文档不参与转换。
    let readme_rid = docs
        .iter()
        .find(|d| d.rel_path == "README.md")
        .and_then(|d| d.requirement_id.clone())
        .expect("README.md 已链接需求");
    conn.execute(
        "DELETE FROM requirements WHERE id = ?1",
        params![readme_rid],
    )
    .unwrap();
    drop(conn);
    let report = project_init::reinit_project(&state, &outcome.project.id).expect("reinit 2");
    assert_eq!(report.requirements_created, 1, "需求被删后文档应重新转换");
}

// ⑤b 空目录不产生需求。
#[test]
fn empty_dir_creates_no_requirements() {
    let base = temp_base("no-reqs");
    let project_dir = base.join("empty-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let state = app_state(&base);
    let outcome =
        project_init::init_project(&state, &project_dir.display().to_string()).expect("init");
    assert_eq!(outcome.report.requirements_created, 0, "空目录不应产生需求");
    assert!(outcome.report.converted_requirement_ids.is_empty());
}

// ⑨ 自动归类（两级）：本地规则在转换时即刻预归档；未归档的进入 initializing（列表隐藏），
//    LLM 理解完成后才转待确认并对列表可见。
#[tokio::test(flavor = "multi_thread")]
async fn auto_classify_done_docs_after_understanding() {
    std::env::set_var("AGENTUP_TEST_FAST", "1");
    std::env::set_var("AGENTUP_TEST_FAKE_RUNTIME", "1");
    let base = temp_base("classify");
    let project_dir = base.join("classify-project");
    write_file(
        &project_dir.join("docs/done-feature.md"),
        "# 已完成功能\n该功能已实现并上线，实现代码见 src/ 目录。",
    );
    write_file(
        &project_dir.join("docs/todo-feature.md"),
        "# 待实施\n新增导出报告功能：支持按周导出项目进度。",
    );
    let state = app_state(&base);
    let outcome =
        project_init::init_project(&state, &project_dir.display().to_string()).expect("init");
    assert_eq!(outcome.report.requirements_created, 2);
    assert_eq!(
        outcome.report.locally_archived, 1,
        "含完成信号的文档应在转换时即被本地规则预归档"
    );

    let status_of = |id: &str| {
        let conn = state.conn.lock().unwrap();
        db::get_requirement(&conn, id).unwrap().unwrap().status
    };
    let done_id = outcome
        .report
        .converted_requirement_ids
        .iter()
        .find(|id| status_of(id) == RequirementStatus::Completed)
        .expect("预归档需求存在")
        .clone();
    let todo_id = outcome
        .report
        .converted_requirement_ids
        .iter()
        .find(|id| status_of(id) == RequirementStatus::Initializing)
        .expect("待实施需求存在（initializing）")
        .clone();

    // 列表隐藏：理解完成前 initializing 不出现在需求列表
    {
        let conn = state.conn.lock().unwrap();
        let visible = db::list_requirements_by_project(&conn, &outcome.project.id).unwrap();
        assert_eq!(visible.len(), 1, "只有已归档的可见，初始化中的必须隐藏");
        assert_eq!(visible[0].id, done_id);
    }

    // 理解队列只处理未归档的需求（与命令层跳过逻辑同构）
    orchestrator::start_initial_understanding(&state, todo_id.clone()).await;
    assert_eq!(
        status_of(&done_id),
        RequirementStatus::Completed,
        "预归档需求不因理解而丢失状态"
    );
    assert_eq!(
        status_of(&todo_id),
        RequirementStatus::AwaitingConfirmation,
        "理解完成后转待确认并可见"
    );
    {
        let conn = state.conn.lock().unwrap();
        let visible = db::list_requirements_by_project(&conn, &outcome.project.id).unwrap();
        assert_eq!(visible.len(), 2, "理解完成后列表应展示两条");
    }
    // 本地归档依据落了日志（可审计）
    let conn = state.conn.lock().unwrap();
    let logs = db::list_logs_by_requirement(&conn, &done_id).unwrap();
    assert!(
        logs.iter().any(|l| l.message.contains("本地规则归类")),
        "预归档判定必须落日志"
    );
}

// ⑩ 存量迁移：doc-pending → initializing（幂等）。
#[test]
fn legacy_doc_pending_migrates_to_initializing() {
    let base = temp_base("migrate-pending");
    let state = app_state(&base);
    {
        let conn = state.conn.lock().unwrap();
        let project = db::create_project(&conn, "迁移项目", None).unwrap();
        for (content, expect_initializing) in [
            ("来自存量文档「docs/spec.md」：\n旧数据", true),
            ("用户手敲的正常需求内容", false),
        ] {
            db::create_requirement(
                &conn,
                &project.id,
                content,
                None,
                RequirementMode::Standard,
                None,
            )
            .unwrap();
            let _ = expect_initializing;
        }
    }
    // 重新打开触发迁移
    let state2 = db::open_state(&base).expect("reopen");
    let conn = state2.conn.lock().unwrap();
    let statuses: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT status FROM requirements WHERE content LIKE '来自存量文档%'")
            .unwrap();
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
        rows.collect::<Result<Vec<_>, _>>().unwrap()
    };
    assert_eq!(
        statuses,
        vec!["initializing".to_string()],
        "存量文档需求应迁移为 initializing"
    );
    let normal: String = conn
        .query_row(
            "SELECT status FROM requirements WHERE content NOT LIKE '来自存量文档%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(normal, "pending", "正常 pending 需求不受迁移影响");
}

// ⑥ 待定项人工作答 → resolved。
#[test]
fn resolve_pending_stores_answer() {
    let base = temp_base("pending");
    let project_dir = base.join("pending-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let state = app_state(&base);
    let outcome =
        project_init::init_project(&state, &project_dir.display().to_string()).expect("init");

    let pending = {
        let conn = state.conn.lock().unwrap();
        db::list_governance_items(&conn, &outcome.project.id, Some("pending"))
            .unwrap()
            .into_iter()
            .find(|i| i.status == "active")
            .expect("pending exists")
    };
    let resolved = {
        let conn = state.conn.lock().unwrap();
        db::resolve_governance_pending(&conn, &pending.id, "cargo test --test regression")
            .expect("resolve")
            .expect("resolved item returned")
    };
    assert_eq!(resolved.status, "resolved");
    assert_eq!(resolved.body["answer"], "cargo test --test regression");

    // 二次作答应 409
    let second = {
        let conn = state.conn.lock().unwrap();
        db::resolve_governance_pending(&conn, &pending.id, "again")
    };
    assert!(second.is_err(), "已解决的待定项不允许再答");
}

// ⑦ 指标聚合：Σ tasks.tokens 与实际时长落 requirements。
#[test]
fn refresh_requirement_metrics_aggregates() {
    let base = temp_base("metrics");
    let project_dir = base.join("metrics-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let state = app_state(&base);
    let outcome =
        project_init::init_project(&state, &project_dir.display().to_string()).expect("init");

    let (requirement, task_ids) = {
        let conn = state.conn.lock().unwrap();
        let requirement = db::create_requirement(
            &conn,
            &outcome.project.id,
            "测试指标聚合的需求内容",
            None,
            RequirementMode::Standard,
            Some(10),
        )
        .unwrap();
        let t1 = db::create_task(
            &conn,
            &requirement.id,
            StepType::Understand,
            "理解需求",
            TaskStatus::Running,
        )
        .unwrap();
        let t2 = db::create_task(
            &conn,
            &requirement.id,
            StepType::Implement,
            "实施",
            TaskStatus::Running,
        )
        .unwrap();
        (requirement, vec![t1.id, t2.id])
    };
    {
        let conn = state.conn.lock().unwrap();
        db::update_task(
            &conn,
            &task_ids[0],
            &db::TaskUpdates {
                tokens: Some(1200),
                status: Some(TaskStatus::Completed),
                ..Default::default()
            },
        )
        .unwrap();
        db::update_task(
            &conn,
            &task_ids[1],
            &db::TaskUpdates {
                tokens: Some(800),
                status: Some(TaskStatus::Completed),
                ..Default::default()
            },
        )
        .unwrap();
        db::refresh_requirement_metrics(&conn, &requirement.id).unwrap();
    }
    let fresh = {
        let conn = state.conn.lock().unwrap();
        db::get_requirement(&conn, &requirement.id)
            .unwrap()
            .unwrap()
    };
    assert_eq!(fresh.total_tokens, Some(2000));
    assert_eq!(fresh.estimated_minutes, Some(10));
    assert!(
        fresh.actual_minutes.is_some(),
        "实际耗时应被聚合（≥0 分钟）"
    );
}

// ⑧ 导出 agentup-files：生成技能兼容文件集（单向投影），目录出现治理文件仅发生在显式导出时。
#[test]
fn export_agentup_files_writes_projection() {
    let base = temp_base("export");
    let project_dir = base.join("export-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let state = app_state(&base);
    let outcome =
        project_init::init_project(&state, &project_dir.display().to_string()).expect("init");

    let result = project_init::export_agentup_files(&state, &outcome.project.id).expect("export");
    assert!(result.written.iter().any(|p| p.ends_with("AGENTS.md")));
    assert!(result
        .written
        .iter()
        .any(|p| p.ends_with("docs/development-process.md")));
    assert!(result
        .written
        .iter()
        .any(|p| p.ends_with("docs/agent/roles/implementation.md")));
    assert!(result
        .written
        .iter()
        .any(|p| p.ends_with("docs/agent/artifacts.yaml")));
    let agents = std::fs::read_to_string(project_dir.join("AGENTS.md")).unwrap();
    assert!(
        agents.contains("生成于 AgentUp Harness"),
        "投影文件必须带生成头"
    );
    assert!(
        agents.contains("docs/development-process.md"),
        "路由必须指向流程权威"
    );

    // 二次导出内容未变 → 全部 skipped（幂等）
    let second = project_init::export_agentup_files(&state, &outcome.project.id).expect("export 2");
    assert!(
        second.written.is_empty(),
        "内容未变应跳过，实际写入 {:?}",
        second.written
    );
    assert_eq!(second.skipped.len(), result.written.len());
}
