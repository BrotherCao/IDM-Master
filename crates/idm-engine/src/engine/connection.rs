use reqwest::header::{CONTENT_LENGTH, CONTENT_DISPOSITION};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// HEAD 请求返回的文件元信息
#[derive(Debug, Clone)]
pub struct HeadInfo {
    pub content_length: u64,
    pub supports_range: bool,
    pub filename: Option<String>,
}

/// HTTP 连接池，复用内置 reqwest::Client（自带连接池），外加全局并发信号量。
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

    pub fn semaphore(&self) -> Arc<Semaphore> { Arc::clone(&self.semaphore) }
    pub fn client(&self) -> &Client { &self.client }

    /// HEAD 请求获取文件大小、是否支持 Range、文件名
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

    /// 下载指定 Range，返回 Response 供流式读取
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
        if status != reqwest::StatusCode::PARTIAL_CONTENT
            && status != reqwest::StatusCode::OK
        {
            anyhow::bail!("Unexpected HTTP status for range: {}", status);
        }
        Ok(resp)
    }
}

/// 从响应头提取文件名（Content-Disposition 优先，其次 URL path）
fn extract_filename(resp: &reqwest::Response) -> Option<String> {
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
    resp.url().path().rsplit('/').next()
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
