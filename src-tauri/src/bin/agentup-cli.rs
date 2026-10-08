use agentup_harness_lib::types::InitReport;
use agentup_harness_lib::{db, orchestrator, project_init, understanding_queue};
use std::path::PathBuf;

/// AgentUp Harness CLI —— 用脚本/命令行完成「初始化目录 → 存量文档转需求 → 串行理解」。
/// 与 GUI 共用同一数据库与同一理解队列（WAL 多进程安全），GUI 端轮询即可看到结果。
///
/// 用法：
///   agentup-cli init <目录路径> [--no-understand]   初始化目录为项目并转需求（默认入队理解并消化）
///   agentup-cli reinit <项目id> [--no-understand]   幂等重跑已有项目
///   agentup-cli classify <项目id>                   对存量 pending 需求回溯执行本地字段归类
///   agentup-cli understand-pending <项目id> [上限]   消化存量队列（入队 initializing 需求并串行消化）
///   agentup-cli list                                列出项目与其需求状态
///   agentup-cli understand <需求id>                 对单条需求执行初始理解（绕过队列，立即执行）
///   agentup-cli drain                               消化理解队列中全部 queued 行后退出
///
/// 环境变量：AGENTUP_CODEX / AGENTUP_OPENCODE 指定 runtime 路径（同 GUI）。

fn data_dir() -> PathBuf {
    // AGENTUP_DATA_DIR 可覆盖（测试/多实例）；默认与 GUI 同库（tauri.conf.json identifier 惯例路径）
    if let Ok(dir) = std::env::var("AGENTUP_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").expect("无法定位 HOME");
    PathBuf::from(home).join("Library/Application Support/com.agentup.harness")
}

fn open_app_state() -> db::AppState {
    let state = db::open_state(&data_dir()).expect("打开应用数据库失败");
    // 与运行中的 GUI 并发写库：给 SQLite 足够的锁等待
    state.conn.lock().unwrap().execute_batch("PRAGMA busy_timeout = 10000;").expect("set busy_timeout");
    state
}

/// CLI 侧的队列收尾：若 GUI 已把任务跑完（行已删）这里直接空转；若 GUI 掉线，由本进程把 queued 行消化掉。
/// 消化前先收尸（把 GUI 崩溃遗留的 running 行复位），保证断点续跑。
async fn drain_queue(state: &db::AppState) {
    let recovered = understanding_queue::recover(&state.conn.lock().unwrap()).unwrap_or((0, 0, 0));
    if recovered.0 > 0 {
        println!("收尸：复位 {} 条中断的理解任务，继续消化", recovered.0);
    }
    understanding_queue::drain_all(state).await;
}

fn print_report(report: &InitReport) {
    for step in &report.steps {
        println!("  [{:>7}] {}: {}", step.action, step.item, step.detail);
    }
    println!(
        "  汇总：文档 +{} / 更新 {} / 缺失 {} · 治理种子 {} · 待定 {} · 转需求 {}（预归档 {}）",
        report.docs_added, report.docs_updated, report.docs_missing, report.governance_seeded, report.pending_open, report.requirements_created, report.locally_archived
    );
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("用法：agentup-cli init <目录路径> [--no-understand] | reinit <项目id> | list | classify <项目id> | understand-pending <项目id> [上限] | drain | understand <需求id>");
        std::process::exit(2);
    }
    let state = open_app_state();
    match args[0].as_str() {
        "init" => {
            let path = args.get(1).cloned().unwrap_or_default();
            let no_understand = args.iter().any(|a| a == "--no-understand");
            let outcome = project_init::init_project(&state, &path).unwrap_or_else(|e| {
                eprintln!("初始化失败：{e}");
                std::process::exit(1);
            });
            if outcome.already_registered {
                println!("「{}」已是项目（id {}），直接复用", outcome.project.name, outcome.project.id);
            } else {
                println!("已初始化「{}」（id {}）", outcome.project.name, outcome.project.id);
            }
            print_report(&outcome.report);
            if no_understand {
                println!("按 --no-understand 跳过理解；稍后可用 drain 消化理解队列。");
                return;
            }
            {
                let conn = state.conn.lock().unwrap();
                for id in &outcome.report.converted_requirement_ids {
                    understanding_queue::enqueue(&conn, id).unwrap_or_else(|e| { eprintln!("入队失败 {id}: {e}"); false });
                }
            }
            drain_queue(&state).await;
        }
        "reinit" => {
            let id = args.get(1).cloned().unwrap_or_default();
            let no_understand = args.iter().any(|a| a == "--no-understand");
            let report = project_init::reinit_project(&state, &id).unwrap_or_else(|e| {
                eprintln!("重新初始化失败：{e}");
                std::process::exit(1);
            });
            println!("已重新初始化项目 {id}");
            print_report(&report);
            if !no_understand {
                {
                    let conn = state.conn.lock().unwrap();
                    for rid in &report.converted_requirement_ids {
                        understanding_queue::enqueue(&conn, rid).unwrap_or_else(|e| { eprintln!("入队失败 {rid}: {e}"); false });
                    }
                }
                drain_queue(&state).await;
            }
        }
        "list" => {
            let conn = state.conn.lock().unwrap();
            for project in db::list_projects(&conn).unwrap() {
                let hidden: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM requirements WHERE project_id = ?1 AND status = 'initializing'",
                        rusqlite::params![project.id],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                println!(
                    "{}  {}  {}{}",
                    project.id,
                    project.name,
                    project.path.as_deref().unwrap_or("(未绑定目录)"),
                    if hidden > 0 { format!("  （另有 {hidden} 条初始化中，隐藏）") } else { String::new() }
                );
                for requirement in db::list_requirements_by_project(&conn, &project.id).unwrap() {
                    println!(
                        "    {}  {:<22} {:<10} {}",
                        requirement.id,
                        requirement.status.as_str(),
                        requirement.lane.as_deref().unwrap_or("-"),
                        requirement.content_preview.chars().take(40).collect::<String>()
                    );
                }
            }
        }
        "classify" => {
            // 回溯归类：对存量 pending 的文档转换需求补跑本地字段规则（老数据在预归档功能上线前生成）
            let project_id = args.get(1).cloned().unwrap_or_default();
            let (checked, archived) = classify_pending(&state, &project_id).unwrap_or_else(|e| {
                eprintln!("归类失败：{e}");
                std::process::exit(1);
            });
            println!("本地规则回溯归类：检查 {checked} 条待分类需求，归档 {archived} 条为已完成");
        }
        "understand-pending" => {
            // 消化初始化队列：initializing 需求全部入队后串行消化（可带上限截断）
            let project_id = args.get(1).cloned().unwrap_or_default();
            let limit: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
            let ids: Vec<String> = (|| {
                let conn = state.conn.lock().unwrap();
                let mut stmt = conn
                    .prepare(
                        "SELECT id FROM requirements WHERE project_id = ?1 AND status = 'initializing' ORDER BY created_at ASC",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = stmt
                    .query_map(rusqlite::params![project_id], |row| row.get::<_, String>(0))
                    .map_err(|e| e.to_string())?;
                rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
            })()
            .unwrap_or_else(|e| {
                eprintln!("查询初始化队列失败：{e}");
                std::process::exit(1);
            });
            if ids.is_empty() {
                println!("初始化队列为空（没有待理解的 initializing 需求）。");
                return;
            }
            let batch: Vec<String> = ids.into_iter().take(limit).collect();
            println!("初始化队列共 {} 条（本次消化 {} 条）", {
                let conn = state.conn.lock().unwrap();
                conn.query_row::<i64, _, _>(
                    "SELECT COUNT(*) FROM requirements WHERE project_id = ?1 AND status = 'initializing'",
                    rusqlite::params![project_id],
                    |row| row.get(0),
                )
                .unwrap_or(0)
            }, batch.len());
            {
                let conn = state.conn.lock().unwrap();
                for id in &batch {
                    understanding_queue::enqueue(&conn, id).unwrap_or_else(|e| { eprintln!("入队失败 {id}: {e}"); false });
                }
            }
            drain_queue(&state).await;
        }
        "drain" => {
            // 消化理解队列中全部 queued 行（含 GUI 遗留的中断任务）后退出
            drain_queue(&state).await;
            println!("队列消化完毕。");
        }
        "understand" => {
            let id = args.get(1).cloned().unwrap_or_default();
            understand_one(&state, &id).await;
        }
        other => {
            eprintln!("未知子命令：{other}");
            std::process::exit(2);
        }
    }
}

/// 对项目内 pending 的文档转换需求回溯执行本地字段归类。
fn classify_pending(state: &db::AppState, project_id: &str) -> Result<(i64, i64), String> {
    use agentup_harness_lib::types::RequirementStatus;
    let conn = state.conn.lock().unwrap();
    let requirements = db::list_requirements_by_project(&conn, project_id).map_err(|e| e.to_string())?;
    let mut checked = 0i64;
    let mut archived = 0i64;
    for requirement in requirements {
        if requirement.status != RequirementStatus::Pending || requirement.source_doc_path.is_none() {
            continue;
        }
        checked += 1;
        if project_init::looks_done("", "", &requirement.content_preview) {
            db::update_requirement(
                &conn,
                &requirement.id,
                &db::RequirementUpdates {
                    status: Some(RequirementStatus::Completed),
                    current_step: None,
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())?;
            db::create_log(
                &conn,
                &requirement.id,
                None,
                "classify",
                "info",
                "本地规则归类（回溯）：需求内容命中完成信号，归档为已完成；可重新理解以复核",
                None,
            )
            .map_err(|e| e.to_string())?;
            archived += 1;
        }
    }
    // initializing（隐藏中）的也可用同样规则补归档
    let mut stmt = conn
        .prepare("SELECT id, content FROM requirements WHERE project_id = ?1 AND status = 'initializing'")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![project_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let initializing: Vec<(String, String)> = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    drop(stmt);
    for (id, content) in initializing {
        checked += 1;
        if project_init::looks_done("", "", &content) {
            db::update_requirement(
                &conn,
                &id,
                &db::RequirementUpdates {
                    status: Some(RequirementStatus::Completed),
                    current_step: None,
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())?;
            db::create_log(
                &conn,
                &id,
                None,
                "classify",
                "info",
                "本地规则归类（回溯）：需求内容命中完成信号，归档为已完成；可重新理解以复核",
                None,
            )
            .map_err(|e| e.to_string())?;
            archived += 1;
        }
    }
    Ok((checked, archived))
}

async fn understand_one(state: &db::AppState, requirement_id: &str) {
    println!("理解中：{requirement_id} …");
    orchestrator::start_initial_understanding(state, requirement_id.to_string()).await;
    let conn = state.conn.lock().unwrap();
    if let Some(requirement) = db::get_requirement(&conn, requirement_id).unwrap() {
        println!(
            "  → {}（{} 判级 · {} 车道 · {}）",
            requirement.status.as_str(),
            requirement.complexity.as_deref().unwrap_or("-"),
            requirement.lane.as_deref().unwrap_or("-"),
            requirement.goal_summary.as_deref().unwrap_or("无目标复述").chars().take(60).collect::<String>()
        );
    }
}
