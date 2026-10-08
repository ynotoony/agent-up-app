use agentup_harness_lib::{db, understanding_queue};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::atomic::AtomicU64;
use tauri::Manager;

static SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_base(tag: &str) -> PathBuf {
    // 进程内唯一目录：cargo 同进程并行跑多个测试，纳秒时间戳可能撞车
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "agentup-uq-{}-{}-{}-{}",
        tag,
        std::process::id(),
        n,
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("create temp base");
    dir
}

fn state() -> db::AppState {
    let base = temp_base("uq");
    let state = db::open_state(&base).expect("open state");
    // 测试内的跨连接写（认领事务等）需要锁等待
    state.conn.lock().unwrap().execute_batch("PRAGMA busy_timeout = 10000;").expect("busy_timeout");
    state
}

fn make_requirement(conn: &rusqlite::Connection, project_id: &str, status: &str) -> String {
    let id = agentup_harness_lib::types::new_id();
    conn.execute(
        "INSERT INTO requirements (id, project_id, content, status, mode, current_version, created_at, updated_at)
         VALUES (?1, ?2, '测试需求内容', ?3, 'standard', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        rusqlite::params![id, project_id, status],
    )
    .unwrap();
    id
}

// ① 入队幂等：同一需求重复入队只占一行；出队后可再次入队（重试语义）。
#[test]
fn enqueue_is_idempotent_and_reusable() {
    let state = state();
    let req = {
        let conn = state.conn.lock().unwrap();
        conn.execute("INSERT INTO projects (id, name, status, created_at, updated_at) VALUES ('p1','测试','active','2026-01-01','2026-01-01')", []).unwrap();
        make_requirement(&conn, "p1", "pending")
    };
    {
        let conn = state.conn.lock().unwrap();
        assert!(understanding_queue::enqueue(&conn, &req).unwrap(), "首次入队应成功");
        assert!(!understanding_queue::enqueue(&conn, &req).unwrap(), "重复入队应被跳过");
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM understanding_queue", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "队列里恰好一行");
    }
    // 认领 → 队列变 running，再次入队仍幂等（running 也占位）
    {
        let mut conn = state.conn.lock().unwrap();
        let claimed = understanding_queue::claim_next(&mut conn).unwrap().expect("应认领到");
        assert_eq!(claimed.requirement_id, req);
        assert!(!understanding_queue::enqueue(&conn, &req).unwrap(), "running 期间重复入队应被跳过");
    }
    // 行删除后重试入队成功
    {
        let conn = state.conn.lock().unwrap();
        conn.execute("DELETE FROM understanding_queue", []).unwrap();
        assert!(understanding_queue::enqueue(&conn, &req).unwrap(), "删除后重新入队应成功");
    }
}

// ② 认领原子性：并发两个连接认领同一条队列，只有一方拿到。
#[test]
fn claim_next_is_atomic_across_connections() {
    let state = state();
    let req = {
        let conn = state.conn.lock().unwrap();
        conn.execute("INSERT INTO projects (id, name, status, created_at, updated_at) VALUES ('p1','测试','active','2026-01-01','2026-01-01')", []).unwrap();
        make_requirement(&conn, "p1", "pending")
    };
    {
        let conn = state.conn.lock().unwrap();
        assert!(understanding_queue::enqueue(&conn, &req).unwrap());
    }
    let hits = std::sync::Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let state = state_clone(&state);
        let hits = hits.clone();
        handles.push(std::thread::spawn(move || {
            let mut conn = state.conn.lock().unwrap();
            if understanding_queue::claim_next(&mut conn).unwrap().is_some() {
                hits.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(hits.load(Ordering::SeqCst), 1, "4 路并发认领恰好 1 方成功");
}

fn state_clone(state: &db::AppState) -> std::sync::Arc<db::AppState> {
    // 共享同一 Connection Mutex（与 GUI 内部一致的并发口径）
    std::sync::Arc::new(db::AppState {
        conn: std::sync::Mutex::new(clone_conn(&state.conn.lock().unwrap())),
        attachments_dir: state.attachments_dir.clone(),
        roles_dir: state.roles_dir.clone(),
    })
}

fn clone_conn(conn: &rusqlite::Connection) -> rusqlite::Connection {
    // 独立连接（内存库场景不适用，这里文件库按路径重开，WAL 允许多连接）；
    // 并发写需要 busy_timeout 等锁，否则立即 SQLITE_BUSY
    let path = conn.path().expect("file-backed db").to_string();
    let conn = rusqlite::Connection::open(path).expect("reopen");
    conn.execute_batch("PRAGMA busy_timeout = 10000;").expect("busy_timeout");
    conn
}

// ③ 启动收尸：running 队列行复位 / 孤儿理解任务置 failed / 永挂决策跳过。
#[test]
fn recover_revives_zombies_and_sets_orphans_failed() {
    let state = state();
    {
        let conn = state.conn.lock().unwrap();
        conn.execute("INSERT INTO projects (id, name, status, created_at, updated_at) VALUES ('p1','测试','active','2026-01-01','2026-01-01')", []).unwrap();
    }
    let (req_a, req_b, req_c) = {
        let conn = state.conn.lock().unwrap();
        (
            make_requirement(&conn, "p1", "initializing"),
            make_requirement(&conn, "p1", "understanding"),
            make_requirement(&conn, "p1", "failed"),
        )
    };
    {
        let conn = state.conn.lock().unwrap();
        // 僵尸队列行：req_a 的 running 行（进程崩溃残留）
        conn.execute(
            "INSERT INTO understanding_queue (id, requirement_id, status, attempts, created_at, updated_at) VALUES ('q1', ?1, 'running', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![req_a],
        ).unwrap();
        // 正常 queued 行：req_b，不受收尸影响
        conn.execute(
            "INSERT INTO understanding_queue (id, requirement_id, status, attempts, created_at, updated_at) VALUES ('q2', ?1, 'queued', 0, '2026-01-01', '2026-01-01')",
            rusqlite::params![req_b],
        ).unwrap();
        // 孤儿理解任务：req_b 有 running 的 understand 任务，但队列里没有 running 行 → 应被置 failed
        conn.execute(
            "INSERT INTO tasks (id, requirement_id, step_type, title, status, created_at) VALUES ('t-orphan', ?1, 'understand', '理解需求', 'running', '2026-01-01')",
            rusqlite::params![req_b],
        ).unwrap();
        // 永挂决策：req_c 已 failed 但决策仍 pending → 应被 skipped
        conn.execute(
            "INSERT INTO decisions (id, requirement_id, question, options, status, created_at) VALUES ('d-stuck', ?1, '选哪个？', '[]', 'pending', '2026-01-01')",
            rusqlite::params![req_c],
        ).unwrap();
    }

    let (revived, orphans, stuck) = understanding_queue::recover(&state.conn.lock().unwrap()).unwrap();
    assert_eq!(revived, 1, "复位 1 条僵尸队列行");
    assert_eq!(orphans, 1, "置失败 1 条孤儿理解任务");
    assert_eq!(stuck, 1, "跳过 1 条永挂决策");

    let conn = state.conn.lock().unwrap();
    let zombie_status: String = conn.query_row("SELECT status FROM understanding_queue WHERE id = 'q1'", [], |r| r.get(0)).unwrap();
    assert_eq!(zombie_status, "queued", "僵尸行应复位为 queued");
    let orphan_status: String = conn.query_row("SELECT status FROM tasks WHERE id = 't-orphan'", [], |r| r.get(0)).unwrap();
    assert_eq!(orphan_status, "failed", "孤儿任务应置 failed");
    let decision_status: String = conn.query_row("SELECT status FROM decisions WHERE id = 'd-stuck'", [], |r| r.get(0)).unwrap();
    assert_eq!(decision_status, "skipped", "永挂决策应 skipped");
}

// ④ 泵单例守卫回归：spawn_pump（App 启动路径）必须真正 spawn 泵并消化队列。
// 曾因 spawn_pump 预置 PUMP_RUNNING=true，spawn_pump_state 的 swap 读到 true 直接放弃 spawn，
// 泵从未启动、队列永不消化（2026-10-08 初始化 62 条全量卡死的根因）。
// completed 需求会被 run_one 跳过（队列行删除），以此作为「泵跑过」的可观测信号。
#[test]
fn spawn_pump_actually_spawns_and_drains() {
    let app = tauri::test::mock_app();
    let state = state();
    let req = {
        let conn = state.conn.lock().unwrap();
        conn.execute("INSERT INTO projects (id, name, status, created_at, updated_at) VALUES ('p1','测试','active','2026-01-01','2026-01-01')", []).unwrap();
        make_requirement(&conn, "p1", "completed")
    };
    {
        let conn = state.conn.lock().unwrap();
        assert!(understanding_queue::enqueue(&conn, &req).unwrap(), "入队应成功");
    }
    app.manage(state_clone(&state));
    understanding_queue::spawn_pump(app.handle().clone());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let n: i64 = state
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM understanding_queue", [], |r| r.get(0))
            .unwrap();
        if n == 0 {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "泵未在 10s 内消化队列行（spawn 被单例守卫吞掉）");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
