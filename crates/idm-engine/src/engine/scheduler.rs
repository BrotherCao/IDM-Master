use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use parking_lot::RwLock;
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::connection::ConnectionPool;
use super::speed::SpeedMeter;
use super::task::{DownloadTask, Segment, TaskState};

/// 进度事件，由 CLI / UI 消费
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub task_id: Uuid,
    pub filename: String,
    pub progress: f64,
    pub speed_bps: f64,
    pub downloaded: u64,
    pub total: u64,
    pub state: TaskState,
}

struct InnerTask {
    task: DownloadTask,
    speed_meter: SpeedMeter,
    abort: Arc<std::sync::atomic::AtomicBool>,
    paused: Arc<std::sync::atomic::AtomicBool>,
}

/// 下载任务调度器。
/// 负责：接收任务 → HEAD 探测 → 分段 → 并发下载 → 进度上报。
pub struct DownloadScheduler {
    pool: ConnectionPool,
    semaphore: Arc<Semaphore>,
    handles: Arc<DashMap<Uuid, InnerTask>>,
    on_progress: Arc<RwLock<Option<Box<dyn Fn(ProgressEvent) + Send + Sync>>>>,
}

impl DownloadScheduler {
    pub fn new(pool: ConnectionPool) -> Self {
        let sem = pool.semaphore();
        Self {
            pool,
            semaphore: sem,
            handles: Arc::new(DashMap::new()),
            on_progress: Arc::new(RwLock::new(None)),
        }
    }

    /// 设置进度回调
    pub fn on_progress<F: Fn(ProgressEvent) + Send + Sync + 'static>(&self, f: F) {
        *self.on_progress.write() = Some(Box::new(f));
    }

    /// 提交下载任务
    pub async fn submit(&self, url: String, save_dir: PathBuf) -> Result<Uuid, anyhow::Error> {
        let head = self.pool.fetch_head(&url).await?;
        let filename = head.filename.unwrap_or_else(|| "download.bin".to_owned());
        let file_path = save_dir.join(&filename);

        let mut task = DownloadTask::new(url.clone(), filename.clone(), file_path);
        task.total_size = head.content_length;
        let n = DownloadTask::segment_count_for_size(head.content_length);
        task.split_segments(n);

        let id = task.id;
        let abort = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let paused = Arc::new(std::sync::atomic::AtomicBool::new(false));

        self.handles.insert(id, InnerTask {
            task,
            speed_meter: SpeedMeter::new(Duration::from_secs(5)),
            abort: Arc::clone(&abort),
            paused: Arc::clone(&paused),
        });

        let client = self.pool.client().clone();
        let sem = Arc::clone(&self.semaphore);
        let handles = Arc::clone(&self.handles);
        let on_prog = Arc::clone(&self.on_progress);

        tokio::spawn(async move {
            let result = run_segments(
            client.clone(), Arc::clone(&sem), Arc::clone(&handles), Arc::clone(&on_prog),
            id, &url, Arc::clone(&abort), Arc::clone(&paused)
        ).await;
            match result {
                Ok(_) => emit_state(&handles, &on_prog, id, TaskState::Completed),
                Err(e) => emit_state(&handles, &on_prog, id, TaskState::Error(e.to_string())),
            }
        });

        Ok(id)
    }

    pub fn pause(&self, id: Uuid) {
        if let Some(h) = self.handles.get(&id) { h.paused.store(true, std::sync::atomic::Ordering::SeqCst); }
    }
    pub fn resume(&self, id: Uuid) {
        if let Some(h) = self.handles.get(&id) { h.paused.store(false, std::sync::atomic::Ordering::SeqCst); }
    }
    pub fn cancel(&self, id: Uuid) {
        if let Some(h) = self.handles.get(&id) { h.abort.store(true, std::sync::atomic::Ordering::SeqCst); }
    }

    pub fn list(&self) -> Vec<(Uuid, String, TaskState, f64)> {
        self.handles.iter().map(|e| {
            let t = &e.value().task;
            let state = if e.value().abort.load(std::sync::atomic::Ordering::Relaxed) { TaskState::Cancelled }
            else if e.value().paused.load(std::sync::atomic::Ordering::Relaxed) { TaskState::Paused }
            else if t.is_done() { TaskState::Completed }
            else { TaskState::Running };
            (e.key().clone(), t.filename.clone(), state, t.progress())
        }).collect()
    }
}

/// 核心下载循环：为每个分段启动一个协程，并发下载。
async fn run_segments(
    client: reqwest::Client,
    sem: Arc<Semaphore>,
    handles: Arc<DashMap<Uuid, InnerTask>>,
    on_prog: Arc<RwLock<Option<Box<dyn Fn(ProgressEvent) + Send + Sync>>>>,
    id: Uuid,
    url: &str,
    abort: Arc<std::sync::atomic::AtomicBool>,
    paused: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), anyhow::Error> {
    let segments: Vec<Arc<Segment>> = {
        handles.get(&id)
            .ok_or_else(|| anyhow::anyhow!("task not found"))?
            .task.segments.clone()
    };

    let mut join_set = tokio::task::JoinSet::new();

    for seg in segments {
        let seg = Arc::clone(&seg);
        let client = client.clone();
        let sem = Arc::clone(&sem);
        let handles = Arc::clone(&handles);
        let on_prog = Arc::clone(&on_prog);
        let abort = Arc::clone(&abort);
        let paused = Arc::clone(&paused);
        let url = url.to_owned();

        join_set.spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            if abort.load(std::sync::atomic::Ordering::Relaxed) { return Ok(()); }
            wait_if_paused(&abort, &paused).await;
            if abort.load(std::sync::atomic::Ordering::Relaxed) { return Ok(()); }

            let start = seg.next_range_start();
            let end = seg.end;

            let resp = client.get(&url)
                .header(reqwest::header::RANGE, format!("bytes={}-{}", start, end))
                .send().await?;

            use futures_util::StreamExt;
            let mut stream = resp.bytes_stream();
            while let Some(chunk) = stream.next().await {
                if abort.load(std::sync::atomic::Ordering::Relaxed) { break; }
                wait_if_paused(&abort, &paused).await;
                let chunk = chunk?;
                seg.add_downloaded(chunk.len() as u64);
                if let Some(h) = handles.get(&id) {
                    h.speed_meter.record(chunk.len() as u64);
                    fire_progress(&on_prog, &h.task, h.speed_meter.speed_bps());
                }
            }
            Ok::<_, anyhow::Error>(())
        });
    }

    while let Some(r) = join_set.join_next().await {
        r??;
    }
    Ok(())
}

async fn wait_if_paused(
    abort: &std::sync::atomic::AtomicBool,
    paused: &std::sync::atomic::AtomicBool,
) {
    while paused.load(std::sync::atomic::Ordering::Relaxed)
        && !abort.load(std::sync::atomic::Ordering::Relaxed)
    {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

fn fire_progress(
    on_prog: &Arc<RwLock<Option<Box<dyn Fn(ProgressEvent) + Send + Sync>>>>,
    task: &DownloadTask,
    speed: f64,
) {
    if let Some(cb) = on_prog.read().as_ref() {
        let dl: u64 = task.segments.iter()
            .map(|s| s.downloaded.load(std::sync::atomic::Ordering::Relaxed))
            .sum();
        cb(ProgressEvent {
            task_id: task.id,
            filename: task.filename.clone(),
            progress: task.progress(),
            speed_bps: speed,
            downloaded: dl,
            total: task.total_size,
            state: TaskState::Running,
        });
    }
}

fn emit_state(
    handles: &DashMap<Uuid, InnerTask>,
    on_prog: &Arc<RwLock<Option<Box<dyn Fn(ProgressEvent) + Send + Sync>>>>,
    id: Uuid,
    state: TaskState,
) {
    if let Some(h) = handles.get(&id) {
        if let Some(cb) = on_prog.read().as_ref() {
            let dl: u64 = h.task.segments.iter()
                .map(|s| s.downloaded.load(std::sync::atomic::Ordering::Relaxed))
                .sum();
            cb(ProgressEvent {
                task_id: id,
                filename: h.task.filename.clone(),
                progress: h.task.progress(),
                speed_bps: 0.0,
                downloaded: dl,
                total: h.task.total_size,
                state,
            });
        }
    }
}
