# Phase 1: Rust Download Engine 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建 IDM Master 的 Rust 下载引擎核心，支持 HTTP Range 多线程分段下载、Direct I/O 直写磁盘、滑动窗口测速，提供命令行测试入口。

**Architecture:** 6 个独立模块：task（数据结构）、speed（速度计）、writer（Direct I/O 写入）、connection（HTTP 连接池）、scheduler（任务调度器）、main（CLI入口）。模块依赖关系：`main → scheduler → {task, speed, writer, connection}`，除 scheduler 外各模块相互独立。

**Tech Stack:** Rust 1.80+, tokio, reqwest, uuid, anyhow, clap, dashmap, parking_lot, bytes, futures-util

## Global Constraints

- Windows 10/11 目标平台
- 使用 tokio 异步运行时（multi-threaded）
- Direct I/O 使用 `FILE_FLAG_NO_BUFFERING` + `FILE_FLAG_WRITE_THROUGH`，块大小 4096 字节页对齐
- 全局并发连接上限 Semaphore(32)，可配置
- 动态分段策略：`<1MB→1, 1-10MB→4, 10-100MB→8, >100MB→16`
- 所有模块通过 `engine/mod.rs` 暴露统一接口
- Phase 1 纯引擎 + CLI，不做 UI / HTTP Server / SQLite

---

## 文件结构总览

```
idm-master/
├── Cargo.toml                      # workspace root
└── crates/
    └── idm-engine/
        ├── Cargo.toml
        └── src/
            ├── main.rs             # CLI entry (clap)
            ├── lib.rs              # 库入口
            └── engine/
                ├── mod.rs          # 模块声明
                ├── task.rs         # DownloadTask, Segment, TaskState
                ├── speed.rs        # SpeedMeter + format_bytes helper
                ├── writer.rs       # AlignedBuffer + DirectIOWriter
                ├── connection.rs   # ConnectionPool + HeadInfo
                └── scheduler.rs    # DownloadScheduler 编排层
```

---

### Task 1: 项目脚手架 + 核心数据模型

**Files:** `Cargo.toml` ×2, `lib.rs`, `engine/mod.rs`, `engine/task.rs`

**Produces:** `DownloadTask`, `Segment`, `TaskState` — 后续所有模块的基础类型

- [ ] **Step 1: 创建 workspace Cargo.toml**

```toml
# idm-master/Cargo.toml
[workspace]
members = ["crates/idm-engine"]
resolver = "2"
```

- [ ] **Step 2: 创建 crate Cargo.toml**

```toml
# idm-master/crates/idm-engine/Cargo.toml
[package]
name = "idm-engine"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "idm-master"
path = "src/main.rs"

[lib]
name = "idm_engine"
path = "src/lib.rs"

[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["stream"] }
uuid = { version = "1", features = ["v4"] }
anyhow = "1"
clap = { version = "4", features = ["derive"] }
dashmap = "6"
parking_lot = "0.12"
bytes = "1"
futures-util = "0.3"

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = [
    "Win32_Storage_FileSystem",
    "Win32_System_IO",
] }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: lib.rs + mod.rs**

```rust
// idm-master/crates/idm-engine/src/lib.rs
pub mod engine;
```

```rust
// idm-master/crates/idm-engine/src/engine/mod.rs
pub mod task;
pub mod speed;
pub mod writer;
pub mod connection;
pub mod scheduler;
```

- [ ] **Step 4: 核心数据模型 task.rs**

```rust
// idm-master/crates/idm-engine/src/engine/task.rs
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    Pending,
    Running,
    Paused,
    Completed,
    Error(String),
    Cancelled,
}

pub struct Segment {
    pub start: u64,
    pub end: u64,
    pub downloaded: AtomicU64,
}

impl Segment {
    pub fn new(start: u64, end: u64) -> Self {
        Self { start, end, downloaded: AtomicU64::new(0) }
    }
    pub fn len(&self) -> u64 {
        if self.end >= self.start { self.end - self.start + 1 } else { 0 }
    }
    pub fn remaining(&self) -> u64 {
        self.len().saturating_sub(self.downloaded.load(Ordering::Relaxed))
    }
    pub fn next_range_start(&self) -> u64 {
        self.start + self.downloaded.load(Ordering::Relaxed)
    }
    pub fn add_downloaded(&self, n: u64) -> u64 {
        self.downloaded.fetch_add(n, Ordering::Relaxed) + n
    }
    pub fn is_done(&self) -> bool {
        self.downloaded.load(Ordering::Relaxed) >= self.len()
    }
}

impl Clone for Segment {
    fn clone(&self) -> Self {
        Self {
            start: self.start,
            end: self.end,
            downloaded: AtomicU64::new(self.downloaded.load(Ordering::Relaxed)),
        }
    }
}

pub struct DownloadTask {
    pub id: Uuid,
    pub url: String,
    pub filename: String,
    pub file_path: PathBuf,
    pub total_size: u64,
    pub segments: Vec<Segment>,
}

impl DownloadTask {
    pub fn new(url: String, filename: String, file_path: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4(),
            url,
            filename,
            file_path,
            total_size: 0,
            segments: Vec::new(),
        }
    }

    pub fn segment_count_for_size(file_size: u64) -> usize {
        match file_size {
            s if s < 1024 * 1024 => 1,
            s if s < 10 * 1024 * 1024 => 4,
            s if s < 100 * 1024 * 1024 => 8,
            _ => 16,
        }
    }

    pub fn split_segments(&mut self, n: usize) {
        if self.total_size == 0 || n <= 1 {
            self.segments = vec![Segment::new(0, self.total_size.saturating_sub(1))];
            return;
        }
        let seg_size = self.total_size / n as u64;
        self.segments = (0..n).map(|i| {
            let start = i as u64 * seg_size;
            let end = if i == n - 1 { self.total_size - 1 } else { (i as u64 + 1) * seg_size - 1 };
            Segment::new(start, end)
        }).collect();
    }

    pub fn is_done(&self) -> bool {
        self.segments.iter().all(|s| s.is_done())
    }

    pub fn progress(&self) -> f64 {
        if self.total_size == 0 { return 0.0; }
        let dl: u64 = self.segments.iter().map(|s| s.downloaded.load(Ordering::Relaxed)).sum();
        dl as f64 / self.total_size as f64
    }
}
```

- [ ] **Step 5: 验证编译**

```bash
cd E:/code/IDM/idm-master && cargo check 2>&1
```

- [ ] **Step 6: 提交**

```bash
cd E:/code/IDM/idm-master && git init && git add -A && git commit -m "feat: project scaffold + core data model (task.rs)"
```

---

### Task 2: 滑动窗口速度计

**Files:** `engine/speed.rs`

**Produces:** `SpeedMeter` (5秒滑动窗口), `format_bytes_per_sec()`, `format_bytes()`

- [ ] **Step 1: 实现 speed.rs**

```rust
// idm-master/crates/idm-engine/src/engine/speed.rs
use parking_lot::RwLock;
use std::time::{Duration, Instant};

pub struct SpeedMeter {
    samples: RwLock<Vec<(Instant, u64)>>,
    window: Duration,
}

impl SpeedMeter {
    pub fn new(window: Duration) -> Self {
        Self { samples: RwLock::new(Vec::with_capacity(128)), window }
    }

    pub fn record(&self, bytes: u64) {
        let now = Instant::now();
        let mut s = self.samples.write();
        let total = s.last().map(|(_, t)| *t).unwrap_or(0) + bytes;
        s.push((now, total));
        self.prune(&mut s);
    }

    pub fn speed_bps(&self) -> f64 {
        let mut s = self.samples.write();
        self.prune(&mut s);
        if s.len() < 2 { return 0.0; }
        let first = s.first().unwrap();
        let last = s.last().unwrap();
        let dur = last.0.duration_since(first.0).as_secs_f64();
        if dur < 0.1 { return 0.0; }
        (last.1 - first.1) as f64 / dur
    }

    pub fn speed_human(&self) -> String {
        format_bytes_per_sec(self.speed_bps())
    }

    fn prune(&self, s: &mut Vec<(Instant, u64)>) {
        let cutoff = Instant::now() - self.window;
        while s.len() > 2 && s[1].0 < cutoff {
            s.remove(0);
        }
    }
}

pub fn format_bytes_per_sec(bps: f64) -> String {
    if bps >= 1_073_741_824.0 { format!("{:.1} GB/s", bps / 1_073_741_824.0) }
    else if bps >= 1_048_576.0 { format!("{:.1} MB/s", bps / 1_048_576.0) }
    else if bps >= 1024.0 { format!("{:.1} KB/s", bps / 1024.0) }
    else { format!("{:.0} B/s", bps) }
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 { format!("{:.2} GB", bytes as f64 / 1_073_741_824.0) }
    else if bytes >= 1_048_576 { format!("{:.2} MB", bytes as f64 / 1_048_576.0) }
    else if bytes >= 1024 { format!("{:.2} KB", bytes as f64 / 1024.0) }
    else { format!("{} B", bytes) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_new_meter_is_zero() {
        let sm = SpeedMeter::new(Duration::from_secs(5));
        assert_eq!(sm.speed_bps(), 0.0);
    }

    #[test]
    fn test_record_and_speed() {
        let sm = SpeedMeter::new(Duration::from_secs(5));
        sm.record(1_048_576); // 1MB
        sleep(Duration::from_millis(300));
        let bps = sm.speed_bps();
        assert!(bps > 500_000.0, "expected > 500KB/s, got {}", bps);
    }

    #[test]
    fn test_format_human() {
        assert_eq!(format_bytes_per_sec(500.0), "500 B/s");
        assert_eq!(format_bytes_per_sec(2048.0), "2.0 KB/s");
        assert_eq!(format_bytes(2048), "2.00 KB");
    }
}
```

- [ ] **Step 2: 运行测试**

```bash
cd E:/code/IDM/idm-master && cargo test -p idm-engine 2>&1
```

预期输出：3 个测试全部通过。

- [ ] **Step 3: 提交**

```bash
cd E:/code/IDM/idm-master && git add -A && git commit -m "feat: sliding-window speed meter + format helpers (speed.rs)"
```

---

### Task 3: Direct I/O 文件写入器

**Files:** `engine/writer.rs`

**Produces:** `AlignedBuffer` (页对齐缓冲区), `AlignedBufferPool` (缓冲区对象池), `DirectIOWriter` (FILE_FLAG_NO_BUFFERING 写入)

- [ ] **Step 1: 实现 writer.rs**

```rust
// idm-master/crates/idm-engine/src/engine/writer.rs
use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::Arc;
use parking_lot::Mutex;

const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;

/// 页对齐（4096 字节）的内存缓冲区。
/// 满足 Windows Direct I/O 的扇区对齐要求（4096 是 512 的超集）。
pub struct AlignedBuffer {
    data: Vec<u8>,
    len: usize,
}

impl AlignedBuffer {
    pub fn new(capacity: usize) -> Self {
        let aligned_cap = (capacity.max(1) + 4095) / 4096 * 4096;
        Self { data: vec![0u8; aligned_cap], len: 0 }
    }
    pub fn as_mut_slice(&mut self) -> &mut [u8] { &mut self.data }
    pub fn as_slice(&self) -> &[u8] { &self.data[..self.len] }
    pub fn set_len(&mut self, len: usize) { self.len = len; }
    pub fn capacity(&self) -> usize { self.data.len() }
    pub fn is_full(&self) -> bool { self.len >= self.data.len() }
    pub fn remaining(&self) -> usize { self.data.len() - self.len }
    pub fn clear(&mut self) { self.len = 0; }
}

/// 缓冲区对象池，减少频繁分配。
pub struct AlignedBufferPool {
    pool: Arc<Mutex<Vec<AlignedBuffer>>>,
    buf_size: usize,
}

impl AlignedBufferPool {
    pub fn new(buf_size: usize) -> Self {
        Self { pool: Arc::new(Mutex::new(Vec::new())), buf_size }
    }
    pub fn acquire(&self) -> AlignedBuffer {
        self.pool.lock().pop().unwrap_or_else(|| AlignedBuffer::new(self.buf_size))
    }
    pub fn release(&self, mut buf: AlignedBuffer) {
        buf.clear();
        let mut pool = self.pool.lock();
        if pool.len() < 16 { pool.push(buf); }
    }
}

/// Direct I/O 文件写入器 — 跳过 OS 文件缓存，直接写入磁盘。
pub struct DirectIOWriter {
    file: File,
    written: u64,
}

impl DirectIOWriter {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .write(true).create(true).truncate(false)
            .custom_flags(FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH)
            .open(path)?;
        Ok(Self { file, written: 0 })
    }

    /// 在指定偏移写入页对齐数据。调用方负责 offset 和数据长度对齐。
    pub fn write_at(&mut self, offset: u64, data: &[u8]) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(data)?;
        self.written += data.len() as u64;
        Ok(())
    }

    /// 截断文件到实际大小并落盘
    pub fn finalize(mut self) -> io::Result<()> {
        self.file.set_len(self.written)?;
        self.file.flush()?;
        Ok(())
    }

    pub fn written(&self) -> u64 { self.written }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_aligned_buffer() {
        let mut buf = AlignedBuffer::new(512 * 1024);
        assert_eq!(buf.capacity(), 512 * 1024);
        assert_eq!(buf.as_slice().len(), 0);
        buf.as_mut_slice()[..5].copy_from_slice(b"hello");
        buf.set_len(5);
        assert_eq!(buf.as_slice(), b"hello");
    }

    #[test]
    fn test_buffer_pool() {
        let pool = AlignedBufferPool::new(4096);
        let buf = pool.acquire();
        assert_eq!(buf.capacity(), 4096);
        pool.release(buf);
        let buf2 = pool.acquire();
        assert_eq!(buf2.capacity(), 4096);
    }

    #[test]
    fn test_direct_io_open_and_finalize() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("out.bin");
        let writer = DirectIOWriter::open(&path).unwrap();
        writer.finalize().unwrap();
        assert!(path.exists());
    }
}
```

- [ ] **Step 2: 运行测试**

```bash
cd E:/code/IDM/idm-master && cargo test -p idm-engine 2>&1
```

预期输出：共 6 个测试全部通过（speed 3 + writer 3）。

- [ ] **Step 3: 提交**

```bash
cd E:/code/IDM/idm-master && git add -A && git commit -m "feat: aligned buffer + Direct I/O file writer (writer.rs)"
```

---

### Task 4: HTTP 连接池

**Files:** `engine/connection.rs`

**Produces:** `HeadInfo`, `ConnectionPool::new()`, `fetch_head()`, `fetch_range()`

- [ ] **Step 1: 实现 connection.rs**

```rust
// idm-master/crates/idm-engine/src/engine/connection.rs
use reqwest::header::{CONTENT_LENGTH, CONTENT_DISPOSITION};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

#[derive(Debug, Clone)]
pub struct HeadInfo {
    pub content_length: u64,
    pub supports_range: bool,
    pub filename: Option<String>,
}

pub struct ConnectionPool {
    client: Client,
    semaphore: Arc<Semaphore>,
}

impl ConnectionPool {
    pub fn new(max_conns: usize) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(8)
            .user_agent("IDM-Master/1.0")
            .build()
            .expect("Failed to build HTTP client");
        Self { client, semaphore: Arc::new(Semaphore::new(max_conns)) }
    }

    pub fn semaphore(&self) -> Arc<Semaphore> {
        Arc::clone(&self.semaphore)
    }

    pub fn client(&self) -> &Client { &self.client }

    /// HEAD 请求获取文件元信息
    pub async fn fetch_head(&self, url: &str) -> Result<HeadInfo, anyhow::Error> {
        let resp = self.client.head(url).send().await?;
        let headers = resp.headers();

        let content_length = headers
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let supports_range = headers
            .get("accept-ranges")
            .map(|v| v.as_bytes() == b"bytes")
            .unwrap_or(false);

        let filename = extract_filename(&resp);

        Ok(HeadInfo { content_length, supports_range, filename })
    }

    /// 下载指定 Range
    pub async fn fetch_range(
        &self, url: &str, start: u64, end: Option<u64>,
    ) -> Result<reqwest::Response, anyhow::Error> {
        let range = match end {
            Some(e) => format!("bytes={}-{}", start, e),
            None => format!("bytes={}-", start),
        };
        let resp = self.client.get(url)
            .header(reqwest::header::RANGE, &range)
            .send().await?;
        let status = resp.status();
        if status != reqwest::StatusCode::PARTIAL_CONTENT && status != reqwest::StatusCode::OK {
            anyhow::bail!("Unexpected status for range request: {}", status);
        }
        Ok(resp)
    }
}

fn extract_filename(resp: &reqwest::Response) -> Option<String> {
    // Content-Disposition header 优先
    if let Some(cd) = resp.headers().get(CONTENT_DISPOSITION) {
        if let Ok(s) = cd.to_str() {
            for part in s.split(';') {
                let t = part.trim();
                if let Some(name) = t.strip_prefix("filename=") {
                    let name = name.trim_matches('"').trim_matches('\'');
                    if !name.is_empty() { return Some(name.to_string()); }
                }
                if let Some(name) = t.strip_prefix("filename*=UTF-8''") {
                    let name = name.trim_matches('"');
                    if !name.is_empty() { return Some(name.to_string()); }
                }
            }
        }
    }
    // Fallback: URL 路径最后一段
    let path = resp.url().path();
    path.rsplit('/').next()
        .filter(|&s| !s.is_empty())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_pool() {
        let pool = ConnectionPool::new(8);
        assert!(pool.client().get("http://httpbin.org/get").build().is_ok());
    }

    #[test]
    fn test_semaphore_permits() {
        let pool = ConnectionPool::new(16);
        assert_eq!(pool.semaphore().available_permits(), 16);
    }
}
```

- [ ] **Step 2: 运行测试**

```bash
cd E:/code/IDM/idm-master && cargo test -p idm-engine 2>&1
```

预期：共 8 个测试通过（speed 3 + writer 3 + connection 2）。

- [ ] **Step 3: 提交**

```bash
cd E:/code/IDM/idm-master && git add -A && git commit -m "feat: HTTP connection pool with HEAD + Range (connection.rs)"
```

---

### Task 5: 任务调度器

**Files:** `engine/scheduler.rs`

**Produces:** `DownloadScheduler` — 组合 connection + writer + speed，提供 submit/pause/resume/cancel

- [ ] **Step 1: 实现 scheduler.rs**

```rust
// idm-master/crates/idm-engine/src/engine/scheduler.rs
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

/// 进度事件 — 由 CLI / UI 消费
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

    pub fn on_progress<F: Fn(ProgressEvent) + Send + Sync + 'static>(&self, f: F) {
        *self.on_progress.write() = Some(Box::new(f));
    }

    /// 提交下载：HEAD → 创建文件 → 分段 → 启动 N 个并发下载协程
    pub async fn submit(&self, url: String, save_dir: PathBuf) -> Result<Uuid, anyhow::Error> {
        let head = self.pool.fetch_head(&url).await?;
        let filename = head.filename.unwrap_or_else(|| "download.bin".to_owned());
        let file_path = save_dir.join(&filename);

        let mut task = DownloadTask::new(url.clone(), filename.clone(), file_path.clone());
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
            if let Err(e) = execute_download(id, &url, &client, &sem, &handles, &on_prog, &abort, &paused).await {
                emit_state(&handles, &on_prog, id, TaskState::Error(e.to_string()));
            } else {
                emit_state(&handles, &on_prog, id, TaskState::Completed);
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

/// 下载主循环
async fn execute_download(
    id: Uuid, url: &str,
    client: &reqwest::Client,
    sem: &Arc<Semaphore>,
    handles: &DashMap<Uuid, InnerTask>,
    on_prog: &Arc<RwLock<Option<Box<dyn Fn(ProgressEvent) + Send + Sync>>>>,
    abort: &std::sync::atomic::AtomicBool,
    paused: &std::sync::atomic::AtomicBool,
) -> Result<(), anyhow::Error> {
    let segments: Vec<Segment> = {
        let h = handles.get(&id).ok_or_else(|| anyhow::anyhow!("task not found"))?;
        h.task.segments.clone()
    };

    let mut joins = tokio::task::JoinSet::new();

    for seg in &segments {
        let seg = seg.clone();
        let url = url.to_owned();
        let client = client.clone();
        let sem = Arc::clone(sem);
        let handles = Arc::clone(handles);
        let on_prog = Arc::clone(on_prog);
        let abort = Arc::clone(abort);
        let paused = Arc::clone(paused);

        joins.spawn(async move {
            let _p = sem.acquire().await.unwrap();
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
                    emit_running(&on_prog, &h.task, h.speed_meter.speed_bps());
                }
            }
            Ok::<_, anyhow::Error>(())
        });
    }

    while let Some(r) = joins.join_next().await {
        r??;
    }

    // TODO: Phase 2 写入文件
    Ok(())
}

async fn wait_if_paused(abort: &std::sync::atomic::AtomicBool, paused: &std::sync::atomic::AtomicBool) {
    while paused.load(std::sync::atomic::Ordering::Relaxed)
        && !abort.load(std::sync::atomic::Ordering::Relaxed)
    {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

fn emit_running(
    on_prog: &Arc<RwLock<Option<Box<dyn Fn(ProgressEvent) + Send + Sync>>>>,
    task: &DownloadTask,
    speed: f64,
) {
    if let Some(cb) = on_prog.read().as_ref() {
        let dl: u64 = task.segments.iter().map(|s| s.downloaded.load(std::sync::atomic::Ordering::Relaxed)).sum();
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
    id: Uuid, state: TaskState,
) {
    if let Some(h) = handles.get(&id) {
        if let Some(cb) = on_prog.read().as_ref() {
            let dl: u64 = h.task.segments.iter().map(|s| s.downloaded.load(std::sync::atomic::Ordering::Relaxed)).sum();
            cb(ProgressEvent {
                task_id: id, filename: h.task.filename.clone(),
                progress: h.task.progress(), speed_bps: 0.0,
                downloaded: dl, total: h.task.total_size, state,
            });
        }
    }
}
```

- [ ] **Step 2: 验证编译**

```bash
cd E:/code/IDM/idm-master && cargo check 2>&1
```

- [ ] **Step 3: 提交**

```bash
cd E:/code/IDM/idm-master && git add -A && git commit -m "feat: download scheduler with concurrent segments (scheduler.rs)"
```

---

### Task 6: CLI 入口

**Files:** `main.rs`

- [ ] **Step 1: 实现 CLI**

```rust
// idm-master/crates/idm-engine/src/main.rs
use std::path::PathBuf;
use clap::Parser;
use idm_engine::engine::connection::ConnectionPool;
use idm_engine::engine::scheduler::DownloadScheduler;
use idm_engine::engine::task::TaskState;

#[derive(Parser)]
#[command(name = "idm-master", about = "High Performance Download Manager")]
struct Cli {
    #[arg(short, long)] url: String,
    #[arg(short, long, default_value = ".")] output: PathBuf,
    #[arg(short = 'c', long, default_value = "32")] connections: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    println!("IDM Master v0.1.0\nURL: {}\nSave dir: {}\nConns: {}\n---",
        cli.url, cli.output.display(), cli.connections);

    let pool = ConnectionPool::new(cli.connections);
    let scheduler = DownloadScheduler::new(pool);

    scheduler.on_progress(|ev| {
        use idm_engine::engine::speed;
        print!("\r  {}  {:.0}%  {}  {}/{}",
            ev.filename,
            ev.progress * 100.0,
            speed::format_bytes_per_sec(ev.speed_bps),
            speed::format_bytes(ev.downloaded),
            speed::format_bytes(ev.total));
    });

    let task_id = scheduler.submit(cli.url, cli.output).await?;
    println!("Task: {}\n", task_id);

    loop {
        let tasks = scheduler.list();
        if let Some((_, _, state, _)) = tasks.iter().find(|(id, ..)| *id == task_id) {
            match state {
                TaskState::Completed => { println!("\nDone."); break; }
                TaskState::Error(e)  => { println!("\nError: {}", e); break; }
                TaskState::Cancelled => { println!("\nCancelled."); break; }
                _ => {}
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    Ok(())
}
```

- [ ] **Step 2: 构建 + 测试**

```bash
cd E:/code/IDM/idm-master && cargo build --release 2>&1
```

然后用一个真实的 HTTP 文件测试：

```bash
./target/release/idm-master.exe -u "http://speedtest.tele2.net/10MB.zip" -o ./downloads -c 8
```

- [ ] **Step 3: 最终提交**

```bash
cd E:/code/IDM/idm-master && git add -A && git commit -m "feat: CLI download entry (main.rs)"
```

---

## 已知改进项 (Phase 2+)

1. scheduler 目前只记录 segments.downloaded 计数，未真正将数据写入文件 — Phase 2 集成 DirectIOWriter
2. AlignedBuffer 的 512KB 攒批逻辑需在 scheduler 中实现
3. 暂停信号改用 `tokio::sync::Notify` 替代轮询
4. 断点续传信息需落 SQLite
5. HEAD 重定向后的 URL 更新
