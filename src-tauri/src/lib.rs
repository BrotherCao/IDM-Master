use idm_engine::engine;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct TaskDto {
    id: String,
    filename: String,
    state: String,
    progress: f64,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! IDM Master is running.", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
