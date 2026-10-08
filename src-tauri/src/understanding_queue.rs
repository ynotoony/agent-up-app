use crate::db::AppState;
use crate::error::ApiResult;
use crate::orchestrator;
use crate::types::now_iso;
use rusqlite::{params, OptionalExtension};
use std::sync::Arc;
use tauri::Manager;

/// 理解队列 —— 全部初始理解入口的唯一通道（对应 PRD《04》§4 的队列化治理）。
/// 行即锁（requirement_id UNIQUE）：重复入队幂等跳过；单例泵逐条认领执行，进程崩溃后由启动收尸复位僵尸行。

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub id: String,
    pub requirement_id: String,
    pub status: String,
    pub attempts: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 入队一条初始理解。幂等：该需求已有 queued/running 行时直接跳过。
/// 返回是否真正入队（false = 已在队列中）。
pub fn enqueue(conn: &rusqlite::Connection, requirement_id: &str) -> ApiResult<bool> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM understanding_queue WHERE requirement_id = ?1 AND status IN ('queued','running')",
            params![requirement_id],
            |row| row.get(0),
        )
        .ok();
    if existing.is_some() {
        return Ok(false);
    }
    let ts = now_iso();
    conn.execute(
        "INSERT INTO understanding_queue (id, requirement_id, status, attempts, created_at, updated_at)
         VALUES (?1, ?2, 'queued', 0, ?3, ?3)
         ON CONFLICT(requirement_id) DO NOTHING",
        params![crate::types::new_id(), requirement_id, ts],
    )?;
    Ok(conn.execute("UPDATE understanding_queue SET updated_at = ?2 WHERE requirement_id = ?1", params![requirement_id, ts])? > 0)
}

/// 原子认领队首一条 queued 行（置 running + attempts+1）。无任务返回 None。
/// BEGIN IMMEDIATE 一开始就取写锁：WAL 下延迟事务升级写锁会撞 SQLITE_BUSY_SNAPSHOT
/// （busy_timeout 不重试该错误），GUI 与 CLI 并发消化时必须立即事务。
pub fn claim_next(conn: &mut rusqlite::Connection) -> ApiResult<Option<QueueItem>> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let claim = (|| -> rusqlite::Result<Option<QueueItem>> {
        let mut stmt = tx.prepare(
            "SELECT id, requirement_id, status, attempts, created_at, updated_at
             FROM understanding_queue WHERE status = 'queued' ORDER BY created_at ASC LIMIT 1",
        )?;
        let item = stmt
            .query_row([], |row| {
                Ok(QueueItem {
                    id: row.get(0)?,
                    requirement_id: row.get(1)?,
                    status: row.get(2)?,
                    attempts: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .optional()?;
        if let Some(item) = &item {
            let changed = tx.execute(
                "UPDATE understanding_queue SET status = 'running', attempts = attempts + 1, updated_at = ?2 WHERE id = ?1 AND status = 'queued'",
                params![item.id, now_iso()],
            )?;
            if changed == 0 {
                return Ok(None); // 已被其他实例抢先认领
            }
        }
        Ok(item)
    })()?;
    tx.commit()?;
    Ok(claim)
}

/// 当前 queued 条数（GUI 进度展示用）。
pub fn queued_count(conn: &rusqlite::Connection) -> ApiResult<i64> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM understanding_queue WHERE status = 'queued'",
        [],
        |row| row.get(0),
    )?;
    Ok(n)
}

/// 单条任务的执行体：需求已消失或已归档则跳过，否则执行初始理解（内部自带降级）。
async fn run_one(state: &AppState, requirement_id: &str) {
    let skip = {
        let conn = state.conn.lock().unwrap();
        match crate::db::get_requirement(&conn, requirement_id) {
            Ok(Some(requirement)) => requirement.status == crate::types::RequirementStatus::Completed,
            _ => true, // 需求已不存在
        }
    };
    if skip {
        return;
    }
    orchestrator::start_initial_understanding(state, requirement_id.to_string()).await;
}

/// 泵循环：认领 → 执行 → 删行，直到队列为空。每次循环重新加锁（不复用跨 await 的锁守卫）。
async fn pump_loop(state: &AppState) {
    loop {
        let item = {
            let mut conn = state.conn.lock().unwrap();
            match claim_next(&mut conn) {
                Ok(item) => item,
                Err(e) => {
                    eprintln!("[understanding-queue] 认领失败: {e}");
                    return;
                }
            }
        };
        let Some(item) = item else { return }; // 队列空，泵退出
        run_one(state, &item.requirement_id).await;
        let conn = state.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM understanding_queue WHERE id = ?1", params![item.id]);
    }
}

/// 进程内单例泵：已在跑则忽略本次唤醒；空闲时被 enqueue 唤醒。
static PUMP_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 唤醒泵：仅当泵未在跑时启动一轮消化。需要 AppHandle 以从 Tauri 取管理的 Arc<AppState>。
/// 置位守卫统一由 spawn_pump_state 的 swap 完成；本函数不得预置 PUMP_RUNNING，
/// 否则 spawn_pump_state 的 swap 读到 true 会直接放弃 spawn，泵永远无法启动（2026-10-08 全量卡死事故根因）。
pub fn spawn_pump<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    let state: Arc<AppState> = app.state::<std::sync::Arc<AppState>>().inner().clone();
    spawn_pump_state(state);
}

/// 同 spawn_pump，但直接持有 AppState（命令层从 State<Arc<AppState>> 调用，绕开 State 未实现 Manager 的限制）。
/// PUMP_RUNNING 的 swap 是唯一置位点：换到 true 的一方负责 spawn，泵退出时复位。
pub fn spawn_pump_state(state: Arc<AppState>) {
    if PUMP_RUNNING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        pump_loop(&state).await;
        PUMP_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
        // 收尾双检：泵退出前若又有入队（被吞掉的那次唤醒），补一轮
        let pending = state.conn.lock().unwrap().query_row(
            "SELECT COUNT(*) FROM understanding_queue WHERE status = 'queued'",
            [],
            |row| row.get::<_, i64>(0),
        ).unwrap_or(0);
        if pending > 0 {
            spawn_pump_state(state);
        }
    });
}

/// CLI / 测试用：在当前线程串行消化整条队列（不 spawn，跑完返回）。
pub async fn drain_all(state: &AppState) {
    pump_loop(state).await;
}

// ---------------------------------------------------------------- 启动收尸

/// 崩溃恢复（App 启动时调用一次）：
/// ① 僵尸队列行：running 的队列行复位为 queued（进程死亡时任务不可能还活着）；
/// ② 孤儿 running 理解 task：没有对应 running 队列行的 understand 任务置 failed；
/// ③ 永挂 decision：需求已 failed 但决策仍 pending → skipped（重试路由会重新走流程）。
pub fn recover(conn: &rusqlite::Connection) -> ApiResult<(usize, usize, usize)> {
    let ts = now_iso();

    // ① 僵尸队列行 → queued（泵启动后自然消化；attempts 已在认领时 +1 留档）
    let revived = conn.execute(
        "UPDATE understanding_queue SET status = 'queued', updated_at = ?1 WHERE status = 'running'",
        params![ts],
    )? as usize;

    // ② 孤儿理解 task：running 但所属需求既无 running 队列行也无 running/pending 需求态 → failed
    //    （保守口径：只杀「队列里没有任何 running 行」时期的 understand 任务，避免误杀正在跑的）
    let orphans = conn.execute(
        "UPDATE tasks SET status = 'failed', error_message = '进程中断，任务被收尸复位（可重试）', completed_at = ?1
         WHERE status = 'running' AND step_type = 'understand'
           AND requirement_id NOT IN (SELECT requirement_id FROM understanding_queue WHERE status = 'running')",
        params![ts],
    )? as usize;

    // ③ 永挂决策：需求已 failed 且决策仍 pending → skipped（failed 重试会从失败阶段恢复，不再等这个决策）
    let stuck_decisions = conn.execute(
        "UPDATE decisions SET status = 'skipped', resolved_at = ?1
         WHERE status = 'pending'
           AND requirement_id IN (SELECT id FROM requirements WHERE status = 'failed')",
        params![ts],
    )? as usize;

    Ok((revived, orphans, stuck_decisions))
}
