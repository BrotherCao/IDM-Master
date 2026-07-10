use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Arc;
use parking_lot::Mutex;
use uuid::Uuid;

/// 数据库封装，提供任务持久化和配置存储。
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// 打开（或创建）数据库文件，运行迁移。
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let db = Self { conn: Arc::new(Mutex::new(conn)) };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                filename TEXT NOT NULL,
                file_path TEXT NOT NULL,
                total_size INTEGER NOT NULL DEFAULT 0,
                downloaded INTEGER NOT NULL DEFAULT 0,
                state TEXT NOT NULL DEFAULT 'pending',
                error_message TEXT,
                created_at INTEGER NOT NULL,
                completed_at INTEGER,
                referer TEXT,
                cookies TEXT
            );

            CREATE TABLE IF NOT EXISTS segments (
                task_id TEXT NOT NULL,
                segment_index INTEGER NOT NULL,
                start_byte INTEGER NOT NULL,
                end_byte INTEGER NOT NULL,
                downloaded INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (task_id, segment_index),
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );"
        )?;
        Ok(())
    }

    /// 保存任务（新建或更新）
    pub fn save_task(
        &self,
        id: Uuid,
        url: &str,
        filename: &str,
        file_path: &str,
        total_size: u64,
        downloaded: u64,
        state: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO tasks (id, url, filename, file_path, total_size, downloaded, state, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch())
             ON CONFLICT(id) DO UPDATE SET downloaded=?6, state=?7",
            params![id.to_string(), url, filename, file_path, total_size, downloaded, state],
        )?;
        Ok(())
    }

    /// 保存分段进度
    pub fn save_segment(
        &self,
        task_id: Uuid,
        index: usize,
        start: u64,
        end: u64,
        downloaded: u64,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO segments (task_id, segment_index, start_byte, end_byte, downloaded)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(task_id, segment_index) DO UPDATE SET downloaded=?5",
            params![task_id.to_string(), index, start, end, downloaded],
        )?;
        Ok(())
    }

    /// 读取设置
    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        ).ok()
    }

    /// 写入设置
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=?2",
            params![key, value],
        )?;
        Ok(())
    }
}
