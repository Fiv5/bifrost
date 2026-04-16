use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;

pub const GITHUB_RELEASES_LATEST_URL: &str =
    "https://github.com/bifrost-proxy/bifrost/releases/latest";
pub const GITHUB_RELEASES_API_URL: &str =
    "https://api.github.com/repos/bifrost-proxy/bifrost/releases/latest";
pub const GITHUB_TAGS_API_URL: &str = "https://api.github.com/repos/bifrost-proxy/bifrost/tags";
pub const GITHUB_RELEASE_URL: &str = "https://github.com/bifrost-proxy/bifrost/releases/tag";
pub const REQUEST_TIMEOUT_SECS: u64 = 10;
pub const MAX_RETRIES: u32 = 2;
pub const RETRY_DELAY_MS: u64 = 500;
const MAX_RELEASE_HIGHLIGHTS: usize = 5;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VersionCache {
    pub latest_version: String,
    pub release_highlights: Vec<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub body: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubTag {
    pub name: String,
}

#[derive(Debug)]
pub enum FetchError {
    Network(String),
    Parse(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Network(msg) => write!(f, "{}", msg),
            FetchError::Parse(msg) => write!(f, "{}", msg),
        }
    }
}

pub fn extract_version_from_redirect_url(url: &str) -> Result<String, FetchError> {
    if let Some(idx) = url.rfind("/tag/") {
        let tag = &url[idx + 5..];
        let tag = tag.trim_end_matches('/');
        let version = tag.strip_prefix('v').unwrap_or(tag).to_string();
        if version.is_empty() {
            Err(FetchError::Parse(format!(
                "empty version tag in URL: {}",
                url
            )))
        } else {
            debug!(version = %version, url = %url, "extracted version from redirect");
            Ok(version)
        }
    } else {
        Err(FetchError::Parse(format!(
            "no /tag/ found in redirect URL: {}",
            url
        )))
    }
}

pub fn make_release_tag(version: &str) -> String {
    if version.contains('-') || version.chars().next().is_none_or(|c| c.is_ascii_digit()) {
        format!("v{}", version)
    } else {
        version.to_string()
    }
}

pub fn release_api_url_for_tag(tag: &str) -> String {
    format!(
        "https://api.github.com/repos/bifrost-proxy/bifrost/releases/tags/{}",
        tag
    )
}

pub fn strip_tag_prefix(tag: &str) -> String {
    tag.strip_prefix('v').unwrap_or(tag).to_string()
}

pub fn pick_latest_tag(tags: Vec<GitHubTag>) -> Option<String> {
    tags.into_iter()
        .map(|t| t.name)
        .filter(|name| name.starts_with('v'))
        .map(|name| name.trim_start_matches('v').to_string())
        .max_by(|a, b| compare_versions(a, b))
}

pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
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

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    compare_versions(latest, current) == std::cmp::Ordering::Greater
}

pub fn classify_ureq_error(err: &ureq::Error) -> &'static str {
    match err {
        ureq::Error::Status(status, _) => {
            if *status == 403 {
                "GitHub API rate limit exceeded"
            } else if *status == 404 {
                "GitHub API endpoint not found"
            } else {
                "HTTP error from GitHub API"
            }
        }
        ureq::Error::Transport(transport) => match transport.kind() {
            ureq::ErrorKind::Dns => "DNS resolution failed (check network connectivity)",
            ureq::ErrorKind::ConnectionFailed => {
                "connection failed (GitHub may be unreachable from your network)"
            }
            ureq::ErrorKind::Io => {
                let msg = transport.to_string().to_lowercase();
                if msg.contains("timed out") || msg.contains("timeout") {
                    "connection timed out (GitHub may be unreachable from your network)"
                } else {
                    "network I/O error"
                }
            }
            ureq::ErrorKind::ProxyConnect | ureq::ErrorKind::ProxyUnauthorized => {
                "proxy-related error"
            }
            ureq::ErrorKind::InvalidUrl | ureq::ErrorKind::UnknownScheme => "invalid URL",
            ureq::ErrorKind::TooManyRedirects => "too many redirects",
            ureq::ErrorKind::InsecureRequestHttpsOnly => "TLS/SSL configuration error",
            _ => "network error",
        },
    }
}

pub fn fetch_with_retry(
    agent: &ureq::Agent,
    url: &str,
) -> Result<ureq::Response, Box<ureq::Error>> {
    let mut last_err = None;
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            debug!(
                attempt = attempt + 1,
                max = MAX_RETRIES + 1,
                url,
                "retrying request"
            );
            std::thread::sleep(std::time::Duration::from_millis(
                RETRY_DELAY_MS * (attempt as u64),
            ));
        }
        match agent.get(url).call() {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                let reason = classify_ureq_error(&e);
                debug!(
                    url,
                    attempt = attempt + 1,
                    error = %e,
                    reason,
                    "request failed"
                );
                last_err = Some(Box::new(e));
            }
        }
    }
    Err(last_err.unwrap())
}

pub fn fetch_version_via_redirect_sync() -> Result<String, FetchError> {
    let no_redirect_agent = crate::direct_ureq_agent_builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent("bifrost-cli")
        .redirects(0)
        .build();

    debug!(
        "fetching latest version via redirect from {}",
        GITHUB_RELEASES_LATEST_URL
    );

    match no_redirect_agent.get(GITHUB_RELEASES_LATEST_URL).call() {
        Ok(resp) => {
            let url = resp.get_url().to_string();
            extract_version_from_redirect_url(&url)
        }
        Err(ureq::Error::Status(301 | 302 | 303 | 307 | 308, resp)) => {
            if let Some(location) = resp.header("location") {
                extract_version_from_redirect_url(location)
            } else {
                Err(FetchError::Parse(
                    "redirect response missing Location header".to_string(),
                ))
            }
        }
        Err(e) => {
            let reason = classify_ureq_error(&e);
            Err(FetchError::Network(format!("{}: {}", reason, e)))
        }
    }
}

pub fn fetch_release_body_for_version_sync(agent: &ureq::Agent, version: &str) -> Vec<String> {
    let tag = make_release_tag(version);
    let url = release_api_url_for_tag(&tag);

    debug!(url = %url, "fetching release body for highlights");

    match fetch_with_retry(agent, &url) {
        Ok(response) => match response.into_json::<GitHubRelease>() {
            Ok(release) => parse_release_highlights(release.body.as_deref()),
            Err(e) => {
                debug!(error = %e, "failed to parse release body");
                Vec::new()
            }
        },
        Err(e) => {
            debug!(error = %e, "failed to fetch release body (non-critical)");
            Vec::new()
        }
    }
}

pub fn fetch_latest_release_sync() -> Result<(String, Vec<String>), FetchError> {
    let agent = crate::direct_ureq_agent_builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent("bifrost-cli")
        .build();

    match fetch_version_via_redirect_sync() {
        Ok(version) => {
            let highlights = fetch_release_body_for_version_sync(&agent, &version);
            return Ok((version, highlights));
        }
        Err(e) => {
            debug!(error = %e, "redirect-based version detection failed, falling back to GitHub API");
        }
    }

    match fetch_with_retry(&agent, GITHUB_RELEASES_API_URL) {
        Ok(response) => match response.into_json::<GitHubRelease>() {
            Ok(release) => {
                let version = strip_tag_prefix(&release.tag_name);
                let highlights = parse_release_highlights(release.body.as_deref());
                return Ok((version, highlights));
            }
            Err(e) => {
                debug!(error = %e, "failed to parse GitHub release JSON, falling back to tags API");
            }
        },
        Err(e) => {
            let reason = classify_ureq_error(&e);
            debug!(
                error = %e,
                reason,
                "releases API failed, falling back to tags API"
            );
        }
    }

    let response = fetch_with_retry(&agent, GITHUB_TAGS_API_URL).map_err(|e| {
        let reason = classify_ureq_error(&e);
        FetchError::Network(format!("{}: {}", reason, e))
    })?;

    let tags: Vec<GitHubTag> = response
        .into_json()
        .map_err(|e| FetchError::Parse(format!("failed to parse tags response: {}", e)))?;

    let version = pick_latest_tag(tags)
        .ok_or_else(|| FetchError::Parse("no valid version tags found".to_string()))?;

    Ok((version, Vec::new()))
}

pub async fn fetch_version_via_redirect_async() -> Option<String> {
    debug!(
        "fetching latest version via redirect from {}",
        GITHUB_RELEASES_LATEST_URL
    );

    let no_redirect_client = crate::direct_reqwest_client_builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent("bifrost-admin")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;

    let resp = no_redirect_client
        .get(GITHUB_RELEASES_LATEST_URL)
        .send()
        .await
        .ok()?;

    if resp.status().is_redirection() {
        if let Some(location) = resp.headers().get(reqwest::header::LOCATION) {
            let url = location.to_str().ok()?;
            return extract_version_from_redirect_url(url).ok();
        }
    }

    let follow_client = crate::direct_reqwest_client_builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent("bifrost-admin")
        .build()
        .ok()?;

    let resp = follow_client
        .get(GITHUB_RELEASES_LATEST_URL)
        .send()
        .await
        .ok()?;
    let final_url = resp.url().to_string();
    extract_version_from_redirect_url(&final_url).ok()
}

pub async fn fetch_release_body_for_version_async(
    client: &reqwest::Client,
    version: &str,
) -> Vec<String> {
    let tag = make_release_tag(version);
    let url = release_api_url_for_tag(&tag);

    debug!(url = %url, "fetching release body for highlights");

    match client.get(&url).send().await {
        Ok(response) => match response.json::<GitHubRelease>().await {
            Ok(release) => parse_release_highlights(release.body.as_deref()),
            Err(e) => {
                debug!(error = %e, "failed to parse release body");
                Vec::new()
            }
        },
        Err(e) => {
            debug!(error = %e, "failed to fetch release body (non-critical)");
            Vec::new()
        }
    }
}

pub async fn fetch_latest_release_async() -> Option<(String, Vec<String>)> {
    let client = crate::direct_reqwest_client_builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent("bifrost-admin")
        .build()
        .ok()?;

    if let Some(version) = fetch_version_via_redirect_async().await {
        let highlights = fetch_release_body_for_version_async(&client, &version).await;
        return Some((version, highlights));
    }

    debug!("redirect-based version detection failed, falling back to GitHub API");

    if let Ok(response) = client.get(GITHUB_RELEASES_API_URL).send().await {
        if let Ok(release) = response.json::<GitHubRelease>().await {
            let version = strip_tag_prefix(&release.tag_name);
            let highlights = parse_release_highlights(release.body.as_deref());
            return Some((version, highlights));
        }
    }

    let response = client.get(GITHUB_TAGS_API_URL).send().await.ok()?;
    let tags: Vec<GitHubTag> = response.json().await.ok()?;

    let version = pick_latest_tag(tags)?;

    Some((version, Vec::new()))
}

pub fn parse_release_highlights(body: Option<&str>) -> Vec<String> {
    let body = match body {
        Some(b) if !b.trim().is_empty() => b,
        _ => return Vec::new(),
    };

    let mut highlights = Vec::new();

    let normalize = |s: &str| -> String {
        let mapped: String = s
            .chars()
            .filter(|c| !c.is_control())
            .map(|c| match c {
                '\u{2018}' | '\u{2019}' => '\'',
                _ => c,
            })
            .collect();
        mapped
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace() || *c == '\'')
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    };

    let lines_iter = body.lines().enumerate().peekable();
    for (idx, line) in lines_iter {
        let l = line.trim();
        if l.starts_with("## ") {
            let title = normalize(l.trim_start_matches("## ").trim());
            if title.contains("highlights")
                || title.contains("what's new")
                || title.contains("whats new")
                || title.contains("what\u{2019}s new")
            {
                let mut j = idx + 1;
                while let Some(next_line) = body.lines().nth(j) {
                    let nl = next_line.trim();
                    if nl.starts_with("## ") {
                        break;
                    }
                    if !nl.is_empty() {
                        let cleaned = nl
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
                    j += 1;
                }
            }
        }
    }

    if highlights.is_empty() {
        let mut k = 0usize;
        while k < body.lines().count() {
            let ln = body.lines().nth(k).unwrap().trim();
            if ln.starts_with("### ") {
                let title = normalize(ln.trim_start_matches("### ").trim());
                if title.contains("features")
                    || title.contains("new features")
                    || title.contains("improvements")
                    || title.contains("enhancements")
                {
                    let mut t = k + 1;
                    while let Some(nl) = body.lines().nth(t) {
                        let nlt = nl.trim();
                        if nlt.starts_with("### ") || nlt.starts_with("## ") {
                            break;
                        }
                        if nlt.starts_with("- ") || nlt.starts_with("* ") || nlt.starts_with("• ")
                        {
                            let cleaned = nlt
                                .trim_start_matches("- ")
                                .trim_start_matches("* ")
                                .trim_start_matches("• ")
                                .trim();
                            if let Some(msg) = extract_commit_message(cleaned) {
                                highlights.push(msg);
                                if highlights.len() >= MAX_RELEASE_HIGHLIGHTS {
                                    return highlights;
                                }
                            }
                        }
                        t += 1;
                    }
                }
            }
            k += 1;
        }
    }

    if highlights.is_empty() {
        let mut total_count = 0usize;
        let mut k = 0usize;
        while k < body.lines().count() {
            let ln = body.lines().nth(k).unwrap().trim();
            if ln.starts_with("### ") {
                let mut t = k + 1;
                while let Some(nl) = body.lines().nth(t) {
                    let nlt = nl.trim();
                    if nlt.starts_with("### ") || nlt.starts_with("## ") {
                        break;
                    }
                    if nlt.starts_with("- ") || nlt.starts_with("* ") || nlt.starts_with("• ") {
                        let cleaned = nlt
                            .trim_start_matches("- ")
                            .trim_start_matches("* ")
                            .trim_start_matches("• ")
                            .trim();
                        if let Some(msg) = extract_any_commit_message(cleaned) {
                            total_count += 1;
                            if highlights.len() < MAX_RELEASE_HIGHLIGHTS {
                                highlights.push(msg);
                            }
                        }
                    }
                    t += 1;
                }
            }
            k += 1;
        }
        if total_count > MAX_RELEASE_HIGHLIGHTS {
            highlights.push(format!(
                "... and {} more",
                total_count - MAX_RELEASE_HIGHLIGHTS
            ));
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

pub fn extract_commit_message(line: &str) -> Option<String> {
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

pub fn extract_any_commit_message(line: &str) -> Option<String> {
    let cleaned = if let Some(idx) = line.rfind(" (") {
        if line.ends_with(')') {
            line[..idx].trim()
        } else {
            line
        }
    } else {
        line
    };

    let prefixes = [
        "feat: ",
        "fix: ",
        "chore: ",
        "ci: ",
        "docs: ",
        "refactor: ",
        "test: ",
        "perf: ",
        "style: ",
        "build: ",
    ];

    let mut result = cleaned;
    for prefix in prefixes {
        if let Some(rest) = result.strip_prefix(prefix) {
            result = rest;
            break;
        }
    }

    let scoped_prefixes = [
        "feat(",
        "fix(",
        "chore(",
        "ci(",
        "docs(",
        "refactor(",
        "test(",
        "perf(",
        "style(",
        "build(",
    ];
    for prefix in scoped_prefixes {
        if result.starts_with(prefix) {
            if let Some(idx) = result.find("): ") {
                result = &result[idx + 3..];
            }
            break;
        }
    }

    let result = result.trim();
    if result.is_empty() {
        None
    } else {
        Some(result.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version_from_redirect_url() {
        assert_eq!(
            extract_version_from_redirect_url(
                "https://github.com/bifrost-proxy/bifrost/releases/tag/v0.0.53-beta"
            )
            .unwrap(),
            "0.0.53-beta"
        );
        assert_eq!(
            extract_version_from_redirect_url(
                "https://github.com/bifrost-proxy/bifrost/releases/tag/v1.0.0"
            )
            .unwrap(),
            "1.0.0"
        );
        assert!(extract_version_from_redirect_url("https://github.com/").is_err());
    }

    #[test]
    fn test_make_release_tag() {
        assert_eq!(make_release_tag("0.0.53-beta"), "v0.0.53-beta");
        assert_eq!(make_release_tag("1.0.0"), "v1.0.0");
    }

    #[test]
    fn test_compare_versions_basic() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("1.0.0", "0.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("0.9.9", "1.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_minor() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("0.2.0", "0.1.0"), Ordering::Greater);
        assert_eq!(compare_versions("0.1.0", "0.2.0"), Ordering::Less);
        assert_eq!(compare_versions("0.1.5", "0.1.5"), Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_patch() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("0.0.2", "0.0.1"), Ordering::Greater);
        assert_eq!(compare_versions("0.0.1", "0.0.2"), Ordering::Less);
    }

    #[test]
    fn test_compare_versions_prerelease() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("1.0.0", "1.0.0-alpha"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0-alpha", "1.0.0"), Ordering::Less);
        assert_eq!(
            compare_versions("1.0.0-alpha", "1.0.0-alpha"),
            Ordering::Equal
        );
        assert_eq!(
            compare_versions("1.0.0-beta", "1.0.0-alpha"),
            Ordering::Greater
        );
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.0.1", "1.0.0"));
        assert!(is_newer_version("0.0.1-alpha", "0.0.1"));
        assert!(is_newer_version("0.0.1-alpha", "0.0.2-alpha"));
        assert!(is_newer_version("0.0.1-alpha", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "0.0.1"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("0.0.1", "0.0.1-alpha"));
    }

    #[test]
    fn test_parse_release_highlights_from_highlights_section() {
        let body = r#"## ✨ Highlights

- Added new feature A
- Improved performance by 50%
- Fixed critical bug

## What's Changed

### 🚀 Features
- feat: something else
"#;
        let highlights = parse_release_highlights(Some(body));
        assert_eq!(highlights.len(), 3);
        assert_eq!(highlights[0], "Added new feature A");
        assert_eq!(highlights[1], "Improved performance by 50%");
        assert_eq!(highlights[2], "Fixed critical bug");
    }

    #[test]
    fn test_parse_release_highlights_from_features_section() {
        let body = r#"## What's Changed

### 🚀 Features
- feat: add proxy support (abc123)
- feat(cli): improve startup time (def456)
- feat: enable caching (ghi789)

### 🐛 Bug Fixes
- fix: resolve memory leak
"#;
        let highlights = parse_release_highlights(Some(body));
        assert_eq!(highlights.len(), 3);
        assert_eq!(highlights[0], "add proxy support");
        assert_eq!(highlights[1], "improve startup time");
        assert_eq!(highlights[2], "enable caching");
    }

    #[test]
    fn test_parse_release_highlights_empty() {
        assert!(parse_release_highlights(None).is_empty());
        assert!(parse_release_highlights(Some("")).is_empty());
        assert!(parse_release_highlights(Some("   ")).is_empty());
    }

    #[test]
    fn test_parse_release_highlights_fallback() {
        let body = r#"Some random release notes
without proper structure

- First change item (abc123)
- Second change item (def456)
- Third change here
- Fourth one
- Fifth item too
- Sixth should not appear

**Full Changelog**: https://example.com
"#;
        let highlights = parse_release_highlights(Some(body));
        assert_eq!(highlights.len(), 5);
        assert_eq!(highlights[0], "Some random release notes");
        assert_eq!(highlights[1], "without proper structure");
        assert_eq!(highlights[2], "First change item");
        assert_eq!(highlights[3], "Second change item");
        assert_eq!(highlights[4], "Third change here");
    }

    #[test]
    fn test_parse_release_highlights_plain_highlights() {
        let body = r#"## Highlights

- New rule engine
- Faster startup
- Better logs

## What's Changed
- other stuff
"#;
        let highlights = parse_release_highlights(Some(body));
        assert_eq!(highlights.len(), 3);
        assert_eq!(highlights[0], "New rule engine");
        assert_eq!(highlights[1], "Faster startup");
        assert_eq!(highlights[2], "Better logs");
    }

    #[test]
    fn test_parse_release_highlights_whats_new_curly_apostrophe() {
        let body = "## What\u{2019}s New\n\n- A\n- B\n- C\n";
        let highlights = parse_release_highlights(Some(body));
        assert_eq!(highlights.len(), 3);
        assert_eq!(highlights[0], "A");
        assert_eq!(highlights[1], "B");
        assert_eq!(highlights[2], "C");
    }

    #[test]
    fn test_parse_release_highlights_features_no_emoji() {
        let body = r#"## What's Changed

### Features
- feat: alpha (x1)
- feat(cli): bravo (x2)
- feat: charlie

### Bug Fixes
- nope
"#;
        let highlights = parse_release_highlights(Some(body));
        assert_eq!(highlights.len(), 3);
        assert_eq!(highlights[0], "alpha");
        assert_eq!(highlights[1], "bravo");
        assert_eq!(highlights[2], "charlie");
    }

    #[test]
    fn test_extract_commit_message() {
        assert_eq!(
            extract_commit_message("feat: add new feature (abc123)"),
            Some("add new feature".to_string())
        );
        assert_eq!(
            extract_commit_message("feat(scope): do something (xyz)"),
            Some("do something".to_string())
        );
        assert_eq!(
            extract_commit_message("simple message"),
            Some("simple message".to_string())
        );
    }

    #[test]
    fn test_extract_any_commit_message() {
        assert_eq!(
            extract_any_commit_message("fix(tls): 更新依赖版本并重构证书生成逻辑 (544f003)"),
            Some("更新依赖版本并重构证书生成逻辑".to_string())
        );
        assert_eq!(
            extract_any_commit_message("chore: bump version to 0.0.4-alpha (7c12d34)"),
            Some("bump version to 0.0.4-alpha".to_string())
        );
        assert_eq!(
            extract_any_commit_message("ci(workflow): 改进 Homebrew 公式更新流程 (e2c148a)"),
            Some("改进 Homebrew 公式更新流程".to_string())
        );
        assert_eq!(
            extract_any_commit_message("simple message"),
            Some("simple message".to_string())
        );
    }

    #[test]
    fn test_parse_release_highlights_bugfixes_only() {
        let body = "## What's Changed\n\n### 🐛 Bug Fixes\n- fix(tls): 更新依赖版本并重构证书生成逻辑 (544f003)\n\n### 📝 Other Changes\n- chore: bump version to 0.0.4-alpha (7c12d34)\n- ci(workflow): 改进 Homebrew 公式更新流程 (e2c148a)\n- ci(workflows): 添加对 Windows ARM64 架构的支持 (abe47fa)\n\n**Full Changelog**: https://github.com/bifrost-proxy/bifrost/compare/v0.0.3-alpha...v0.0.4-alpha\n";
        let highlights = parse_release_highlights(Some(body));
        assert_eq!(
            highlights.len(),
            4,
            "Should extract all 4 items from bug fixes and other changes sections"
        );
        assert!(highlights[0].contains("更新依赖版本"));
        assert!(highlights[1].contains("bump version"));
        assert!(highlights[2].contains("改进 Homebrew"));
        assert!(highlights[3].contains("Windows ARM64"));
    }

    #[test]
    fn test_parse_release_highlights_truncation() {
        let body = "## What's Changed\n\n### 🐛 Bug Fixes\n- fix: item 1 (abc)\n- fix: item 2 (abc)\n- fix: item 3 (abc)\n- fix: item 4 (abc)\n- fix: item 5 (abc)\n- fix: item 6 (abc)\n- fix: item 7 (abc)\n";
        let highlights = parse_release_highlights(Some(body));
        assert_eq!(highlights.len(), 6, "Should show top 5 + '... and N more'");
        assert!(highlights[5].contains("... and 2 more"));
    }
}
