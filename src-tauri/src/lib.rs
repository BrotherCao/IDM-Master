use idm_engine::engine::connection::ConnectionPool;
use idm_engine::engine::db::Database;
use idm_engine::engine::scheduler::{DownloadScheduler, ProgressEvent};
use idm_engine::engine::task::TaskState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

static SCHEDULER: OnceLock<Mutex<DownloadScheduler>> = OnceLock::new();

fn get_scheduler() -> &'static Mutex<DownloadScheduler> {
    SCHEDULER.get_or_init(|| {
        let pool = ConnectionPool::new(32);
        let app_dir = dirs_next().unwrap_or_else(|| PathBuf::from("."));
        let db_path = app_dir.join("idm-master.db");
        match Database::open(&db_path) {
            Ok(db) => Mutex::new(DownloadScheduler::with_db(pool, Arc::new(db))),
            Err(_) => Mutex::new(DownloadScheduler::new(pool)),
        }
    })
}

fn dirs_next() -> Option<PathBuf> {
    // 使用 Windows AppData 目录
    std::env::var("APPDATA").ok().map(|d| PathBuf::from(d).join("IDM-Master"))
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub state: String,
    pub progress: f64,
    pub speed_bps: f64,
    pub downloaded: u64,
    pub total: u64,
}

impl From<ProgressEvent> for TaskInfo {
    fn from(ev: ProgressEvent) -> Self {
        Self {
            id: ev.task_id.to_string(),
            filename: ev.filename,
            url: String::new(),
            state: match ev.state {
                TaskState::Pending => "pending",
                TaskState::Running => "running",
                TaskState::Paused => "paused",
                TaskState::Completed => "completed",
                TaskState::Error(_) => "error",
                TaskState::Cancelled => "cancelled",
            }
            .into(),
            progress: ev.progress,
            speed_bps: ev.speed_bps,
            downloaded: ev.downloaded,
            total: ev.total,
        }
    }
}

#[tauri::command]
async fn add_download(url: String, save_dir: String, window: tauri::Window) -> Result<String, String> {
    let s = get_scheduler().lock().await;
    s.on_progress(move |ev: ProgressEvent| {
        let info = TaskInfo::from(ev);
        let _ = window.emit("download-progress", &info);
    });
    let path = PathBuf::from(&save_dir);
    let id = s.submit(url, path).await.map_err(|e| e.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
async fn pause_task(id: String) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    get_scheduler().lock().await.pause(id);
    Ok(())
}

#[tauri::command]
async fn resume_task(id: String) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    get_scheduler().lock().await.resume(id);
    Ok(())
}

#[tauri::command]
async fn cancel_task(id: String) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    get_scheduler().lock().await.cancel(id);
    Ok(())
}

#[tauri::command]
async fn list_tasks() -> Result<Vec<TaskInfo>, String> {
    let tasks = get_scheduler().lock().await.list();
    Ok(tasks
        .into_iter()
        .map(|(id, filename, state, progress)| TaskInfo {
            id: id.to_string(),
            filename,
            url: String::new(),
            state: match state {
                TaskState::Pending => "pending",
                TaskState::Running => "running",
                TaskState::Paused => "paused",
                TaskState::Completed => "completed",
                TaskState::Error(_) => "error",
                TaskState::Cancelled => "cancelled",
            }
            .into(),
            progress,
            speed_bps: 0.0,
            downloaded: 0,
            total: 0,
        })
        .collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    get_scheduler(); // 预热调度器

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            add_download,
            pause_task,
            resume_task,
            cancel_task,
            list_tasks,
        ])
        .setup(|app| {
            use tauri::{
                image::Image,
                menu::{MenuBuilder, MenuItemBuilder},
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
            };

            let show = MenuItemBuilder::with_id("show", "显示").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            let icon = Image::from_bytes(include_bytes!("../icons/icon.ico"))
                .unwrap_or_else(|_| Image::new(&[], 0, 0));

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("IDM Master")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
