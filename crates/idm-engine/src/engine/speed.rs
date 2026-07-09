use parking_lot::RwLock;
use std::time::{Duration, Instant};

/// 滑动窗口速度计，记录最近 `window` 时长内的采样点，
/// 每次查询速度时清理过期数据并计算窗口内的平均速度。
pub struct SpeedMeter {
    samples: RwLock<Vec<(Instant, u64)>>,
    window: Duration,
}

impl SpeedMeter {
    pub fn new(window: Duration) -> Self {
        Self {
            samples: RwLock::new(Vec::with_capacity(128)),
            window,
        }
    }

    /// 记录一次下载增量（本次新增字节数，非累计值）
    pub fn record(&self, bytes: u64) {
        let now = Instant::now();
        let mut s = self.samples.write();
        let total = s.last().map(|(_, t)| *t).unwrap_or(0) + bytes;
        s.push((now, total));
        self.prune(&mut s);
    }

    /// 计算窗口内的平均下载速度 (bytes per second)
    pub fn speed_bps(&self) -> f64 {
        let mut s = self.samples.write();
        self.prune(&mut s);
        if s.len() < 2 {
            return 0.0;
        }
        let first = s.first().unwrap();
        let last = s.last().unwrap();
        let dur = last.0.duration_since(first.0).as_secs_f64();
        if dur < 0.1 {
            return 0.0;
        }
        (last.1 - first.1) as f64 / dur
    }

    /// 人类可读的速度字符串
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

/// 格式化 bytes/s → 人类可读
pub fn format_bytes_per_sec(bps: f64) -> String {
    if bps >= 1_073_741_824.0 {
        format!("{:.1} GB/s", bps / 1_073_741_824.0)
    } else if bps >= 1_048_576.0 {
        format!("{:.1} MB/s", bps / 1_048_576.0)
    } else if bps >= 1024.0 {
        format!("{:.1} KB/s", bps / 1024.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

/// 格式化字节数 → 人类可读
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_new_meter_zero() {
        let sm = SpeedMeter::new(Duration::from_secs(5));
        assert_eq!(sm.speed_bps(), 0.0);
    }

    #[test]
    fn test_record_and_speed() {
        let sm = SpeedMeter::new(Duration::from_secs(5));
        // 两个采样点确保速度计有足够数据计算速度
        sm.record(1_048_576);
        sleep(Duration::from_millis(100));
        sm.record(1_048_576);
        sleep(Duration::from_millis(200));
        let bps = sm.speed_bps();
        assert!(bps > 500_000.0, "expected >500KB/s, got {}", bps);
    }

    #[test]
    fn test_format_helpers() {
        assert_eq!(format_bytes_per_sec(500.0), "500 B/s");
        assert_eq!(format_bytes_per_sec(2048.0), "2.0 KB/s");
        assert_eq!(format_bytes(2048), "2.00 KB");
        assert_eq!(format_bytes(1_073_741_824), "1.00 GB");
    }
}
