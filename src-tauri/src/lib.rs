mod runtime;

use std::sync::Mutex;

use runtime::Runtime;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

pub use runtime::Runtime as AppRuntime;

fn emit_notifications(app: &AppHandle, runtime: &mut Runtime) {
    for (name, payload) in runtime.take_notifications() {
        let _ = app.emit(&name, payload);
    }
}

#[tauri::command(rename_all = "snake_case")]
fn scan_project(project_path: String, app: AppHandle, state: State<'_, Mutex<Runtime>>) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.scan_project(&project_path);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn preview_initialize(
    project_id: String,
    project_path: String,
    expected_root_fingerprint: String,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.preview_initialize(&project_id, &project_path, &expected_root_fingerprint);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn initialize_project(
    project_id: String,
    project_path: String,
    confirmation_token: String,
    expected_root_fingerprint: String,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.initialize_project(
        &project_id,
        &project_path,
        &confirmation_token,
        &expected_root_fingerprint,
    );
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn create_request(
    project_id: String,
    request_id: String,
    content: Value,
    metadata: Value,
    expected_revision: i64,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.create_request(
        &project_id,
        &request_id,
        content,
        metadata,
        expected_revision,
    );
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn load_project(
    project_path: String,
    project_id: Option<String>,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.load_project(&project_path, project_id.as_deref());
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn list_projects(app: AppHandle, state: State<'_, Mutex<Runtime>>) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.list_projects();
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn register_project(
    project_id: String,
    project_path: String,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.register_project(&project_id, &project_path);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn rebind_project(
    project_id: String,
    project_path: String,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.rebind_project(&project_id, &project_path);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn preview_remove_agentup(
    project_id: String,
    project_path: String,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.preview_remove_agentup(&project_id, &project_path);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn remove_agentup(
    project_id: String,
    project_path: String,
    confirmation_token: String,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.remove_agentup(&project_id, &project_path, &confirmation_token);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn post_discussion(
    project_id: String,
    request_id: String,
    body: String,
    attachment_ids: Option<Vec<String>>,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.post_discussion(&project_id, &request_id, &body, attachment_ids);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn add_attachment(
    project_id: String,
    request_id: String,
    media_type: String,
    bytes: Vec<u8>,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.add_attachment(&project_id, &request_id, &media_type, &bytes);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn load_request_thread(
    project_id: String,
    request_id: String,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.load_request_thread(&project_id, &request_id);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn put_scope(
    project_id: String,
    scope_id: String,
    request_id: String,
    task_id: String,
    entries: Value,
    expected_revision: i64,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.put_scope(
        &project_id,
        &scope_id,
        &request_id,
        &task_id,
        entries,
        expected_revision,
    );
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn diff_scope(
    scope_id: String,
    from_revision: i64,
    to_revision: i64,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.diff_scope(&scope_id, from_revision, to_revision);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn put_task(
    project_id: String,
    task_id: String,
    request_id: String,
    title: String,
    parent_id: Option<String>,
    depends_on: Option<Vec<String>>,
    expected_revision: i64,
    content: Option<Value>,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.put_task(
        &project_id,
        &task_id,
        &request_id,
        &title,
        parent_id.as_deref(),
        depends_on,
        expected_revision,
        content,
    );
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn set_task_state(
    task_id: String,
    task_state: String,
    expected_revision: i64,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.set_task_state(&task_id, &task_state, expected_revision);
    emit_notifications(&app, &mut runtime);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn load_board(
    project_id: String,
    request_id: String,
    app: AppHandle,
    state: State<'_, Mutex<Runtime>>,
) -> Value {
    let mut runtime = state.lock().expect("runtime mutex");
    let result = runtime.load_board(&project_id, &request_id);
    emit_notifications(&app, &mut runtime);
    result
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
            app.manage(Mutex::new(Runtime::with_app_data_dir(dir)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_project,
            preview_initialize,
            initialize_project,
            create_request,
            load_project,
            list_projects,
            register_project,
            rebind_project,
            preview_remove_agentup,
            remove_agentup,
            post_discussion,
            add_attachment,
            load_request_thread,
            put_scope,
            diff_scope,
            put_task,
            set_task_state,
            load_board
        ])
        .run(tauri::generate_context!())
        .expect("error while running AgentUp");
}
