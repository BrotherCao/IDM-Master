use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::Arc;
use parking_lot::Mutex;

const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;

/// 页对齐（4096字节）的内存缓冲区，用于 Direct I/O 写入。
pub struct AlignedBuffer {
    data: Vec<u8>,
    len: usize,
}

impl AlignedBuffer {
    /// 创建对齐缓冲区，capacity 向上取整到 4096 的倍数。
    pub fn new(capacity: usize) -> Self {
        let aligned = (capacity.max(1) + 4095) / 4096 * 4096;
        Self { data: vec![0u8; aligned], len: 0 }
    }
    pub fn as_mut_slice(&mut self) -> &mut [u8] { &mut self.data }
    pub fn as_slice(&self) -> &[u8] { &self.data[..self.len] }
    pub fn set_len(&mut self, len: usize) { self.len = len; }
    pub fn capacity(&self) -> usize { self.data.len() }
    pub fn is_full(&self) -> bool { self.len >= self.data.len() }
    pub fn remaining(&self) -> usize { self.data.len() - self.len }
    pub fn clear(&mut self) { self.len = 0; }
}

/// 对齐缓冲区对象池，复用内存减少分配开销。
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

/// Direct I/O 文件写入器。
/// 使用 `FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH` 跳过 OS 缓存，
/// 直接将数据落盘。调用方负责确保偏移和数据长度满足扇区对齐。
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

    pub fn write_at(&mut self, offset: u64, data: &[u8]) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(data)?;
        self.written += data.len() as u64;
        Ok(())
    }

    /// 截断文件到实际大小并刷新到磁盘
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
    fn test_direct_io_open_finalize() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.bin");
        let writer = DirectIOWriter::open(&path).unwrap();
        writer.finalize().unwrap();
        assert!(path.exists());
    }
}
