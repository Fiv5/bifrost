use bifrost_storage::data_dir;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::sync::RwLock;

const GITHUB_RELEASES_API_URL: &str =
    "https://api.github.com/repos/bifrost-proxy/bifrost/releases/latest";
const GITHUB_TAGS_API_URL: &str = "https://api.github.com/repos/bifrost-proxy/bifrost/tags";
const GITHUB_RELEASE_URL: &str = "https://github.com/bifrost-proxy/bifrost/releases/tag";
const CACHE_FILE_NAME: &str = "version_cache.json";
const CACHE_DURATION_HOURS: i64 = 1;
const REQUEST_TIMEOUT_SECS: u64 = 10;
const MAX_RELEASE_HIGHLIGHTS: usize = 5;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VersionCache {
    pub latest_version: String,
    pub release_highlights: Vec<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCheckResponse {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_highlights: Vec<String>,
    pub release_url: Option<String>,
    pub checked_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubTag {
    name: String,
}

pub struct VersionChecker {
    cache: RwLock<Option<VersionCache>>,
}

pub type SharedVersionChecker = Arc<VersionChecker>;

impl VersionChecker {
    pub fn new() -> Self {
        let cache = read_cache();
        Self {
            cache: RwLock::new(cache),
        }
    }

    pub async fn check(&self, force_refresh: bool) -> VersionCheckResponse {
        let current_version = env!("CARGO_PKG_VERSION").to_string();

        if !force_refresh {
            let cache = self.cache.read().await;
            if let Some(ref c) = *cache {
                if is_cache_valid(c) {
                    return self.build_response(&current_version, Some(c.clone()));
                }
            }
        }

        match fetch_latest_release().await {
            Some((latest, highlights)) => {
                let cache = VersionCache {
                    latest_version: latest,
                    release_highlights: highlights,
                    checked_at: Utc::now(),
                };
                write_cache(&cache);

                {
                    let mut cached = self.cache.write().await;
                    *cached = Some(cache.clone());
                }

                self.build_response(&current_version, Some(cache))
            }
            None => {
                let cache = self.cache.read().await;
                self.build_response(&current_version, cache.clone())
            }
        }
    }

    fn build_response(
        &self,
        current_version: &str,
        cache: Option<VersionCache>,
    ) -> VersionCheckResponse {
        match cache {
            Some(c) => {
                let has_update = is_newer_version(current_version, &c.latest_version);
                VersionCheckResponse {
                    has_update,
                    current_version: current_version.to_string(),
                    latest_version: Some(c.latest_version.clone()),
                    release_highlights: c.release_highlights.clone(),
                    release_url: Some(format!("{}/v{}", GITHUB_RELEASE_URL, c.latest_version)),
                    checked_at: Some(c.checked_at.to_rfc3339()),
                }
            }
            None => VersionCheckResponse {
                has_update: false,
                current_version: current_version.to_string(),
                latest_version: None,
                release_highlights: vec![],
                release_url: None,
                checked_at: None,
            },
        }
    }
}

impl Default for VersionChecker {
    fn default() -> Self {
        Self::new()
    }
}

fn cache_file_path() -> PathBuf {
    data_dir().join(CACHE_FILE_NAME)
}

fn read_cache() -> Option<VersionCache> {
    let path = cache_file_path();
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_cache(cache: &VersionCache) {
    let path = cache_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(cache) {
        let _ = fs::write(&path, content);
    }
}

fn is_cache_valid(cache: &VersionCache) -> bool {
    let now = Utc::now();
    let cache_age = now.signed_duration_since(cache.checked_at);
    cache_age < Duration::hours(CACHE_DURATION_HOURS)
}

async fn fetch_latest_release() -> Option<(String, Vec<String>)> {
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent("bifrost-admin")
        .build()
        .ok()?;

    if let Ok(response) = client.get(GITHUB_RELEASES_API_URL).send().await {
        if let Ok(release) = response.json::<GitHubRelease>().await {
            let version = release
                .tag_name
                .strip_prefix('v')
                .unwrap_or(&release.tag_name)
                .to_string();
            let highlights = parse_release_highlights(release.body.as_deref());
            return Some((version, highlights));
        }
    }

    let response = client.get(GITHUB_TAGS_API_URL).send().await.ok()?;
    let tags: Vec<GitHubTag> = response.json().await.ok()?;

    let version = tags
        .into_iter()
        .map(|t| t.name)
        .filter(|name| name.starts_with('v'))
        .map(|name| name.trim_start_matches('v').to_string())
        .max_by(|a, b| compare_versions(a, b))?;

    Some((version, Vec::new()))
}

fn parse_release_highlights(body: Option<&str>) -> Vec<String> {
    let body = match body {
        Some(b) if !b.trim().is_empty() => b,
        _ => return Vec::new(),
    };

    let mut highlights = Vec::new();

    if let Some(start) = body.find("## ✨ Highlights") {
        let section = &body[start..];
        let end = section[1..]
            .find("\n## ")
            .map(|i| i + 1)
            .unwrap_or(section.len());
        let section = &section[..end];

        for line in section.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("## ") {
                break;
            }
            let cleaned = line
                .trim_start_matches("- ")
                .trim_start_matches("* ")
                .trim_start_matches("• ")
                .trim();
            if !cleaned.is_empty() {
                highlights.push(cleaned.to_string());
                if highlights.len() >= MAX_RELEASE_HIGHLIGHTS {
                    return highlights;
                }
            }
        }
    }

    if highlights.is_empty() {
        if let Some(start) = body.find("### 🚀 Features") {
            let section = &body[start..];
            let end = section[1..]
                .find("\n### ")
                .or_else(|| section[1..].find("\n## "))
                .map(|i| i + 1)
                .unwrap_or(section.len());
            let section = &section[..end];

            for line in section.lines().skip(1) {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line.starts_with("### ") || line.starts_with("## ") {
                    break;
                }
                if line.starts_with("- ") || line.starts_with("* ") {
                    let cleaned = line
                        .trim_start_matches("- ")
                        .trim_start_matches("* ")
                        .trim();
                    if let Some(msg) = extract_commit_message(cleaned) {
                        highlights.push(msg);
                        if highlights.len() >= MAX_RELEASE_HIGHLIGHTS {
                            return highlights;
                        }
                    }
                }
            }
        }
    }

    if highlights.is_empty() {
        highlights = fallback_extract_lines(body);
    }

    highlights
}

fn fallback_extract_lines(body: &str) -> Vec<String> {
    const FALLBACK_LINES: usize = 5;

    let mut lines = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if line.starts_with("**Full Changelog**")
            || line.starts_with("---")
            || line.starts_with("## 📥")
            || line.starts_with("| ")
            || line.starts_with("```")
        {
            continue;
        }

        let cleaned = line
            .trim_start_matches("- ")
            .trim_start_matches("* ")
            .trim_start_matches("• ")
            .trim();

        if !cleaned.is_empty() && cleaned.len() > 5 {
            let display = if let Some(idx) = cleaned.rfind(" (") {
                if cleaned.ends_with(')') && cleaned.len() - idx < 15 {
                    cleaned[..idx].trim().to_string()
                } else {
                    cleaned.to_string()
                }
            } else {
                cleaned.to_string()
            };

            if !display.is_empty() {
                lines.push(display);
                if lines.len() >= FALLBACK_LINES {
                    break;
                }
            }
        }
    }
    lines
}

fn extract_commit_message(line: &str) -> Option<String> {
    let cleaned = if let Some(idx) = line.rfind(" (") {
        if line.ends_with(')') {
            line[..idx].trim()
        } else {
            line
        }
    } else {
        line
    };

    let cleaned = cleaned
        .trim_start_matches("feat: ")
        .trim_start_matches("feat(")
        .split(')')
        .next_back()
        .unwrap_or(cleaned)
        .trim_start_matches(": ")
        .trim();

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse_version = |s: &str| -> (u32, u32, u32, String) {
        let (version_part, prerelease) = if let Some(idx) = s.find('-') {
            (&s[..idx], s[idx + 1..].to_string())
        } else {
            (s, String::new())
        };

        let parts: Vec<u32> = version_part
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();

        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
            prerelease,
        )
    };

    let (a_major, a_minor, a_patch, a_pre) = parse_version(a);
    let (b_major, b_minor, b_patch, b_pre) = parse_version(b);

    match (a_major, a_minor, a_patch).cmp(&(b_major, b_minor, b_patch)) {
        std::cmp::Ordering::Equal => match (a_pre.is_empty(), b_pre.is_empty()) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => a_pre.cmp(&b_pre),
        },
        other => other,
    }
}

fn is_newer_version(current: &str, latest: &str) -> bool {
    compare_versions(latest, current) == std::cmp::Ordering::Greater
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("1.0.0", "0.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("0.9.9", "1.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "1.0.0-alpha"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0-alpha", "1.0.0"), Ordering::Less);
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.0.1", "1.0.0"));
        assert!(is_newer_version("0.0.1-alpha", "0.0.1"));
        assert!(!is_newer_version("1.0.0", "0.0.1"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_parse_release_highlights() {
        let body = r#"## ✨ Highlights

- Added new feature A
- Improved performance by 50%
- Fixed critical bug

## What's Changed
"#;
        let highlights = parse_release_highlights(Some(body));
        assert_eq!(highlights.len(), 3);
        assert_eq!(highlights[0], "Added new feature A");
    }
}
