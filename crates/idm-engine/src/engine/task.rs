use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

/// 下载任务状态
#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    Pending,
    Running,
    Paused,
    Completed,
    Error(String),
    Cancelled,
}

/// 一个下载分段，包含文件字节偏移和已下载进度。
/// 使用 `Arc<Segment>` 在多个并发下载协程间共享进度计数器。
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

/// 下载任务。segments 使用 `Arc<Segment>` 以支持并发协程间共享进度。
pub struct DownloadTask {
    pub id: Uuid,
    pub url: String,
    pub filename: String,
    pub file_path: PathBuf,
    pub total_size: u64,
    pub segments: Vec<Arc<Segment>>,
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
            self.segments = vec![Arc::new(Segment::new(0, self.total_size.saturating_sub(1)))];
            return;
        }
        let seg_size = self.total_size / n as u64;
        self.segments = (0..n)
            .map(|i| {
                let start = i as u64 * seg_size;
                let end = if i == n - 1 {
                    self.total_size - 1
                } else {
                    (i as u64 + 1) * seg_size - 1
                };
                Arc::new(Segment::new(start, end))
            })
            .collect();
    }

    pub fn is_done(&self) -> bool {
        self.segments.iter().all(|s| s.is_done())
    }

    pub fn progress(&self) -> f64 {
        if self.total_size == 0 { return 0.0; }
        let dl: u64 = self.segments.iter()
            .map(|s| s.downloaded.load(Ordering::Relaxed))
            .sum();
        dl as f64 / self.total_size as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_segment_count() {
        assert_eq!(DownloadTask::segment_count_for_size(500_000), 1);
        assert_eq!(DownloadTask::segment_count_for_size(5_000_000), 4);
        assert_eq!(DownloadTask::segment_count_for_size(50_000_000), 8);
        assert_eq!(DownloadTask::segment_count_for_size(200_000_000), 16);
    }

    #[test]
    fn test_split_segments() {
        let mut task = DownloadTask::new(
            "http://example.com/f".into(), "f.bin".into(),
            Path::new("out").join("f.bin"),
        );
        task.total_size = 1024 * 1024;
        task.split_segments(4);
        assert_eq!(task.segments.len(), 4);
        assert_eq!(task.segments[0].start, 0);
        assert_eq!(task.segments[0].end, 262143);
        assert_eq!(task.segments[3].start, 786432);
        assert_eq!(task.segments[3].end, 1048575);
    }

    #[test]
    fn test_segment_progress() {
        let seg = Segment::new(0, 999);
        assert_eq!(seg.len(), 1000);
        assert_eq!(seg.remaining(), 1000);
        seg.add_downloaded(500);
        assert_eq!(seg.remaining(), 500);
        assert!(!seg.is_done());
        seg.add_downloaded(500);
        assert!(seg.is_done());
    }

    #[test]
    fn test_task_progress() {
        let mut task = DownloadTask::new("url".into(), "f".into(), PathBuf::from("f"));
        task.total_size = 1000;
        task.split_segments(2);
        task.segments[0].add_downloaded(500);
        task.segments[1].add_downloaded(250);
        assert!((task.progress() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_arc_segment_shared() {
        // Arc<Segment> 使得 clone（Arc::clone）后共享同一个进度计数器
        let seg = Arc::new(Segment::new(0, 999));
        let seg2 = Arc::clone(&seg);
        seg.add_downloaded(500);
        seg2.add_downloaded(250);
        // 共享同一个 AtomicU64，累加 = 750
        assert_eq!(seg.downloaded.load(Ordering::Relaxed), 750);
        assert_eq!(seg2.downloaded.load(Ordering::Relaxed), 750);
    }
}
