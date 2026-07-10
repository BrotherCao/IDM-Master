/// 站点规则引擎：按域名匹配自定义下载配置
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::Url;

/// 一条站点规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteRule {
    pub id: String,
    /// 域名匹配模式（例: "*.example.com", "video.example.com"）
    pub domain_pattern: String,
    /// 自定义保存路径（可选，绝对路径）
    pub save_path: Option<String>,
    /// 最大并发连接数（0 = 使用默认值）
    pub max_connections: usize,
    /// 是否启用
    pub enabled: bool,
}

impl SiteRule {
    pub fn new(domain_pattern: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            domain_pattern: domain_pattern.to_string(),
            save_path: None,
            max_connections: 0,
            enabled: true,
        }
    }

    /// 检查域名是否匹配此规则
    pub fn matches(&self, host: &str) -> bool {
        let pattern = self.domain_pattern.trim().to_lowercase();
        let host = host.trim().to_lowercase();

        // 精确匹配
        if pattern == host {
            return true;
        }

        // *.example.com 通配符
        if pattern.starts_with("*.") {
            let suffix = &pattern[1..]; // .example.com
            return host.ends_with(suffix) && host != &suffix[1..];
        }

        false
    }
}

/// 规则引擎：存储和匹配站点规则
pub struct RuleEngine {
    rules: Vec<SiteRule>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn load(&mut self, rules: Vec<SiteRule>) {
        self.rules = rules;
    }

    pub fn add(&mut self, rule: SiteRule) {
        self.rules.push(rule);
    }

    pub fn remove(&mut self, id: &str) {
        self.rules.retain(|r| r.id != id);
    }

    pub fn list(&self) -> &[SiteRule] {
        &self.rules
    }

    /// 根据 URL 匹配规则，返回第一个匹配的
    pub fn find_match(&self, url_str: &str) -> Option<&SiteRule> {
        let host = match Url::parse(url_str) {
            Ok(u) => u.host_str()?.to_string(),
            Err(_) => return None,
        };

        self.rules
            .iter()
            .filter(|r| r.enabled)
            .find(|r| r.matches(&host))
    }

    /// 获取匹配的保存路径（如果有）
    pub fn resolve_save_dir(
        &self,
        url_str: &str,
        default_dir: PathBuf,
    ) -> PathBuf {
        if let Some(rule) = self.find_match(url_str) {
            if let Some(ref path) = rule.save_path {
                let p = PathBuf::from(path);
                if p.is_absolute() && p.exists() {
                    return p;
                }
            }
        }
        default_dir
    }

    /// 获取匹配的连接数限制（0 = 使用默认值）
    pub fn resolve_max_conns(&self, url_str: &str, default_conns: usize) -> usize {
        self.find_match(url_str)
            .map(|r| r.max_connections)
            .filter(|&c| c > 0)
            .unwrap_or(default_conns)
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let rule = SiteRule::new("example.com");
        assert!(rule.matches("example.com"));
        assert!(!rule.matches("www.example.com"));
        assert!(!rule.matches("other.com"));
    }

    #[test]
    fn test_wildcard_match() {
        let rule = SiteRule::new("*.example.com");
        assert!(rule.matches("www.example.com"));
        assert!(rule.matches("video.example.com"));
        assert!(rule.matches("a.b.example.com"));
        assert!(!rule.matches("example.com"));
        assert!(!rule.matches("other.com"));
    }

    #[test]
    fn test_engine_match_url() {
        let mut engine = RuleEngine::new();
        let mut rule = SiteRule::new("*.example.com");
        rule.enabled = true;
        rule.save_path = Some("D:\\Downloads\\Example".to_string());
        engine.add(rule);

        let result = engine.find_match("https://video.example.com/path/file.mp4");
        assert!(result.is_some());
        assert_eq!(result.unwrap().save_path.as_deref(), Some("D:\\Downloads\\Example"));
    }

    #[test]
    fn test_engine_no_match() {
        let engine = RuleEngine::new();
        assert!(engine.find_match("https://other.com/file.zip").is_none());
    }

    #[test]
    fn test_resolve_save_dir_default() {
        let engine = RuleEngine::new();
        let default = PathBuf::from("C:\\Downloads");
        let result = engine.resolve_save_dir("https://example.com/file.zip", default.clone());
        assert_eq!(result, default);
    }
}
