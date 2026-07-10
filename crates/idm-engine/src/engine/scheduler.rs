use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use parking_lot::RwLock;
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::classify;
use super::connection::ConnectionPool;
use super::db::Database;
use super::rules::{RuleEngine, SiteRule};
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

/// 下载任务调度器
pub struct DownloadScheduler {
    pool: ConnectionPool,
    semaphore: Arc<Semaphore>,
    handles: Arc<DashMap<Uuid, InnerTask>>,
    on_progress: Arc<RwLock<Option<Box<dyn Fn(ProgressEvent) + Send + Sync>>>>,
    db: Option<Arc<Database>>,
    classify_enabled: std::sync::atomic::AtomicBool,
    rules: parking_lot::Mutex<RuleEngine>,
}

impl DownloadScheduler {
    pub fn new(pool: ConnectionPool) -> Self {
        let sem = pool.semaphore();
        Self {
            pool,
            semaphore: sem,
            handles: Arc::new(DashMap::new()),
            on_progress: Arc::new(RwLock::new(None)),
            db: None,
            classify_enabled: std::sync::atomic::AtomicBool::new(true),
            rules: parking_lot::Mutex::new(RuleEngine::new()),
        }
    }

    /// 创建带数据库持久化的调度器
    pub fn with_db(pool: ConnectionPool, db: Arc<Database>) -> Self {
        let sem = pool.semaphore();
        Self {
            pool,
            semaphore: sem,
            handles: Arc::new(DashMap::new()),
            on_progress: Arc::new(RwLock::new(None)),
            db: Some(db),
            classify_enabled: std::sync::atomic::AtomicBool::new(true),
            rules: parking_lot::Mutex::new(RuleEngine::new()),
        }
    }

    /// 启用/禁用自动文件分类
    pub fn set_classify_enabled(&self, enabled: bool) {
        self.classify_enabled.store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    /// 获取分类状态
    pub fn classify_enabled(&self) -> bool {
        self.classify_enabled.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 添加站点规则
    pub fn add_rule(&self, rule: SiteRule) {
        self.rules.lock().add(rule);
    }

    /// 删除站点规则
    pub fn remove_rule(&self, id: &str) {
        self.rules.lock().remove(id);
    }

    /// 列出所有站点规则
    pub fn list_rules(&self) -> Vec<SiteRule> {
        self.rules.lock().list().to_vec()
    }

    /// 根据 URL 解析保存目录（应用站点规则 + 分类）
    pub fn resolve_save_dir(&self, url: &str, base_dir: PathBuf) -> PathBuf {
        let dir = self.rules.lock().resolve_save_dir(url, base_dir);
        if self.classify_enabled() {
            // 需要文件名才能分类 — 这里先返回规则解析的目录
            // 分类在 submit_inner 中进一步处理
        }
        dir
    }

    pub fn on_progress<F: Fn(ProgressEvent) + Send + Sync + 'static>(&self, f: F) {
        *self.on_progress.write() = Some(Box::new(f));
    }

    /// 提交下载任务：HEAD 探测 → 创建文件 → 分段 → 并发下载并写盘 → 完成
    pub async fn submit(&self, url: String, save_dir: PathBuf) -> Result<Uuid, anyhow::Error> {
        self.submit_inner(url, save_dir, None, None, None, None).await
    }

    /// 提交下载（含元数据）：由 Chrome 扩展调用，带有文件名、referer、cookies
    pub async fn submit_with_meta(
        &self,
        url: String,
        save_dir: PathBuf,
        _referer: Option<String>,
        _cookies: Option<String>,
        _user_agent: Option<String>,
    ) -> Result<Uuid, anyhow::Error> {
        // 使用 HEAD 获取文件名，Chrome 扩展可能不传
        self.submit_inner(url, save_dir, None, _referer, _cookies, _user_agent).await
    }

    /// 内部统一入口
    async fn submit_inner(
        &self,
        url: String,
        save_dir: PathBuf,
        filename_hint: Option<String>,
        _referer: Option<String>,
        _cookies: Option<String>,
        _user_agent: Option<String>,
    ) -> Result<Uuid, anyhow::Error> {
        let head = self.pool.fetch_head(&url).await?;
        let filename = filename_hint
            .filter(|f| !f.is_empty())
            .or(head.filename)
            .unwrap_or_else(|| "download.bin".to_owned());

        // 应用站点规则解析保存目录
        let save_dir = self.rules.lock().resolve_save_dir(&url, save_dir);

        // 下载分类：按文件类型自动分到子目录
        let save_dir = if self.classify_enabled() {
            let cat = classify::classify_filename(&filename);
            save_dir.join(cat.dir_name_en())
        } else {
            save_dir
        };
        let file_path = save_dir.join(&filename);

        // 确保保存目录存在
        fs::create_dir_all(&save_dir)?;

        let mut task = DownloadTask::new(url.clone(), filename.clone(), file_path.clone());
        task.total_size = head.content_length;
        let n = DownloadTask::segment_count_for_size(head.content_length);
        task.split_segments(n);

        let id = task.id;

        // 持久化到 SQLite
        if let Some(ref db) = self.db {
            let _ = db.save_task(id, &url, &filename,
                &file_path.to_string_lossy(),
                head.content_length, 0, "running");
        }

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
                id, &url, Arc::clone(&abort), Arc::clone(&paused), file_path,
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

/// 核心下载循环：分段并发下载 + 写入磁盘
async fn run_segments(
    client: reqwest::Client,
    sem: Arc<Semaphore>,
    handles: Arc<DashMap<Uuid, InnerTask>>,
    on_prog: Arc<RwLock<Option<Box<dyn Fn(ProgressEvent) + Send + Sync>>>>,
    id: Uuid,
    url: &str,
    abort: Arc<std::sync::atomic::AtomicBool>,
    paused: Arc<std::sync::atomic::AtomicBool>,
    file_path: PathBuf,
) -> Result<(), anyhow::Error> {
    let segments: Vec<Arc<Segment>> = {
        handles.get(&id)
            .ok_or_else(|| anyhow::anyhow!("task not found"))?
            .task.segments.clone()
    };

    // 创建/打开文件，用 Arc<Mutex<File>> 在多分段间共享
    let file = File::create(&file_path)?;
    let shared_file = Arc::new(std::sync::Mutex::new(file));

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
        let shared_file = Arc::clone(&shared_file);

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
            let mut write_offset = start;

            while let Some(chunk) = stream.next().await {
                if abort.load(std::sync::atomic::Ordering::Relaxed) { break; }
                wait_if_paused(&abort, &paused).await;
                let chunk = chunk?;

                // 写入文件 — 短暂持锁
                {
                    let mut f = shared_file.lock().unwrap();
                    f.seek(SeekFrom::Start(write_offset))?;
                    f.write_all(&chunk)?;
                }

                write_offset += chunk.len() as u64;
                seg.add_downloaded(chunk.len() as u64);

                if let Some(h) = handles.get(&id) {
                    h.speed_meter.record(chunk.len() as u64);
                    fire_progress(&on_prog, &h.task, h.speed_meter.speed_bps());
                }
            }
            Ok::<_, anyhow::Error>(())
        });
    }

    // 等待所有分段完成
    while let Some(r) = join_set.join_next().await {
        r??;
    }

    // 截断文件到正确大小（如果有预分配的话）
    if let Some(h) = handles.get(&id) {
        let total = h.task.total_size;
        if total > 0 {
            let f = shared_file.lock().unwrap();
            f.set_len(total)?;
        }
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
