use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use idm_engine::engine::scheduler::DownloadScheduler;
use idm_engine::engine::task::TaskState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

/// 由所有路由共享的应用状态
#[derive(Clone)]
pub struct AppState {
    pub scheduler: Arc<Mutex<DownloadScheduler>>,
}

/// Chrome 扩展发来的下载请求
#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub referer: Option<String>,
    #[serde(default)]
    pub cookies: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub content_length: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct TaskItem {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub state: String,
    pub progress: f64,
    pub speed_bps: f64,
    pub downloaded: u64,
    pub total: u64,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Json<Self> {
        Json(Self { ok: true, data: Some(data), error: None })
    }
    fn err(msg: &str) -> Json<Self> {
        Json(Self { ok: false, data: None, error: Some(msg.to_string()) })
    }
}

/// 构建 axum Router
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health))
        .route("/api/download", post(handle_download))
        .route("/api/tasks", get(list_tasks))
        .layer(cors)
        .with_state(state)
}

// ─── handlers ───────────────────────────────────────────────

async fn health() -> Json<ApiResponse<String>> {
    ApiResponse::ok("ok".to_string())
}

async fn handle_download(
    State(state): State<AppState>,
    Json(req): Json<DownloadRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let save_dir = get_default_download_dir();
    let url = req.url.clone();
    let filename = req.filename.clone();

    match state.scheduler.lock().await
        .submit_with_meta(url.clone(), save_dir, req.referer, req.cookies, req.user_agent)
        .await
    {
        Ok(id) => {
            println!("[http-server] accepted download: {} → {}", filename, id);
            (StatusCode::OK, ApiResponse::ok(id.to_string()))
        }
        Err(e) => {
            eprintln!("[http-server] failed to submit {}: {}", url, e);
            (StatusCode::BAD_REQUEST, ApiResponse::err(&e.to_string()))
        }
    }
}

async fn list_tasks(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<TaskItem>>> {
    let tasks = state.scheduler.lock().await.list();
    let items: Vec<TaskItem> = tasks
        .into_iter()
        .map(|(id, filename, ts, progress)| TaskItem {
            id: id.to_string(),
            filename,
            url: String::new(),
            state: match ts {
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
        .collect();
    ApiResponse::ok(items)
}

fn get_default_download_dir() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("Downloads")
}

/// 在 tokio 后台启动 HTTP Server，监听 127.0.0.1:16888
pub async fn start_server(state: AppState) {
    let app = build_router(state);

    let listener = match tokio::net::TcpListener::bind("127.0.0.1:16888").await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[http-server] WARNING: failed to bind 127.0.0.1:16888: {} — skipping", e);
            return;
        }
    };

    println!("[http-server] listening on http://127.0.0.1:16888");

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("[http-server] error: {}", e);
    }
}
