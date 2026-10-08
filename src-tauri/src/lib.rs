pub mod types;
pub mod error;
pub mod db;
pub mod task_engine;
pub mod understanding;
pub mod understanding_queue;
pub mod attachments;
pub mod agent_runtime;
pub mod roles;
pub mod project_init;
pub mod orchestrator;
pub mod ticket_source;
pub mod commands;

use tauri::Manager;

pub fn run() {
    attachments::register(tauri::Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = db::init_state(&handle)?;
            roles::seed_roles(&state)?;
            // 流式推流出口：runtime 事件回调经此 emit 给前端
            agent_runtime::set_app_handle(handle.clone());
            // 启动收尸：复位僵尸队列行 / 孤儿理解任务 / 永挂决策（上次进程中断的残留）
            match understanding_queue::recover(&state.conn.lock().unwrap()) {
                Ok((revived, orphans, decisions)) => {
                    if revived + orphans + decisions > 0 {
                        eprintln!("[startup-recover] 僵尸队列行 {revived} · 孤儿理解任务 {orphans} · 永挂决策 {decisions}");
                    }
                }
                Err(e) => eprintln!("[startup-recover] 收尸失败: {e}"),
            }
            app.manage(std::sync::Arc::new(state));
            // 拉起理解泵：消化上次收尸复位的 queued 行；空队列时泵空转一次即退出
            understanding_queue::spawn_pump(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::workspace_get,
            commands::projects_list,
            commands::projects_create,
            commands::projects_init,
            commands::projects_reinit,
            commands::projects_docs,
            commands::projects_doc_read,
            commands::projects_doc_to_requirement,
            commands::projects_tickets,
            commands::projects_governance,
            commands::projects_initializing,
            commands::projects_governance_answer,
            commands::projects_governance_complete,
            commands::projects_export,
            commands::projects_get,
            commands::projects_update,
            commands::projects_delete,
            commands::requirements_create,
            commands::requirements_get,
            commands::requirements_list_tasks,
            commands::requirements_list_decisions,
            commands::requirements_retry,
            commands::requirements_retry_initializing,
            commands::requirements_retry_initializing_batch,
            commands::requirements_confirm,
            commands::requirements_reunderstand,
            commands::requirements_iterate,
            commands::requirements_create_decision,
            commands::decisions_resolve,
            commands::requirements_cancel,
            commands::settings_get_orchestration,
            commands::settings_save_orchestration,
            commands::runtimes_list,
            commands::runtimes_check,
            commands::runtimes_open_app,
            commands::settings_get_runtime,
            commands::settings_set_runtime,
            commands::roles_list,
            commands::roles_get,
            commands::roles_save,
            commands::roles_reset,
            commands::mode_steps,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
