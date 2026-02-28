use bifrost_storage::data_dir;
use chrono::{DateTime, Duration, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const GITHUB_RELEASES_API_URL: &str =
    "https://api.github.com/repos/bifrost-proxy/bifrost/releases/latest";
const GITHUB_TAGS_API_URL: &str = "https://api.github.com/repos/bifrost-proxy/bifrost/tags";
const GITHUB_RELEASE_URL: &str = "https://github.com/bifrost-proxy/bifrost/releases/tag";
const CACHE_FILE_NAME: &str = "version_cache.json";
const CACHE_DURATION_HOURS: i64 = 24;
const REQUEST_TIMEOUT_SECS: u64 = 5;
const MAX_RELEASE_HIGHLIGHTS: usize = 3;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct VersionCache {
    latest_version: String,
    release_highlights: Vec<String>,
    checked_at: DateTime<Utc>,
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

fn fetch_latest_release() -> Option<(String, Vec<String>)> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent("bifrost-cli")
        .build();

    if let Ok(response) = agent.get(GITHUB_RELEASES_API_URL).call() {
        if let Ok(release) = response.into_json::<GitHubRelease>() {
            let version = release
                .tag_name
                .strip_prefix('v')
                .unwrap_or(&release.tag_name)
                .to_string();

            let highlights = parse_release_highlights(release.body.as_deref());
            return Some((version, highlights));
        }
    }

    let response = agent.get(GITHUB_TAGS_API_URL).call().ok()?;
    let tags: Vec<GitHubTag> = response.into_json().ok()?;

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

fn get_latest_version() -> Option<VersionCache> {
    if let Some(cache) = read_cache() {
        if is_cache_valid(&cache) {
            return Some(cache);
        }
    }

    let (latest, highlights) = fetch_latest_release()?;

    let cache = VersionCache {
        latest_version: latest,
        release_highlights: highlights,
        checked_at: Utc::now(),
    };
    write_cache(&cache);

    Some(cache)
}

pub fn check_and_print_update_notice() {
    let current_version = env!("CARGO_PKG_VERSION");

    let cache = match get_latest_version() {
        Some(c) => c,
        None => return,
    };

    if !is_newer_version(current_version, &cache.latest_version) {
        return;
    }

    print_update_notice(current_version, &cache);
}

fn print_update_notice(current_version: &str, cache: &VersionCache) {
    let separator = "─".repeat(64);
    let release_url = format!("{}/v{}", GITHUB_RELEASE_URL, cache.latest_version);

    println!();
    println!("{}", separator.bright_yellow());
    println!(
        "{}",
        "  🚀 A new version of bifrost is available!"
            .bright_yellow()
            .bold()
    );
    println!();
    println!(
        "     Current version: {}",
        current_version.bright_red().bold()
    );
    println!(
        "     Latest version:  {}",
        cache.latest_version.bright_green().bold()
    );

    if !cache.release_highlights.is_empty() {
        println!();
        println!("     {}", "What's new:".bright_white().bold());
        for highlight in &cache.release_highlights {
            let display = if highlight.len() > 50 {
                format!("{}...", &highlight[..47])
            } else {
                highlight.clone()
            };
            println!("       {} {}", "•".bright_cyan(), display.bright_white());
        }
        println!(
            "       {} {}",
            "→".dimmed(),
            format!("View full release notes: {}", release_url).dimmed()
        );
    }

    println!();
    println!("     {}", "To upgrade, run:".bright_white());
    println!("       {}", "bifrost upgrade".bright_cyan().bold());
    println!("{}", separator.bright_yellow());
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

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
    #[ignore = "requires network access"]
    fn test_fetch_latest_release_from_github() {
        let result = fetch_latest_release();
        assert!(result.is_some(), "Should fetch release from GitHub API");

        let (version, _highlights) = result.unwrap();
        assert!(!version.is_empty(), "Version should not be empty");
        assert!(
            !version.starts_with('v'),
            "Version should not start with 'v'"
        );

        let parts: Vec<&str> = version.split('-').next().unwrap().split('.').collect();
        assert!(parts.len() >= 2, "Version should have at least major.minor");
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
    #[ignore = "visual test - run manually to see output"]
    fn test_visual_update_notice() {
        let cache = VersionCache {
            latest_version: "1.0.0".to_string(),
            release_highlights: vec![
                "Added WebSocket support for real-time communication".to_string(),
                "Improved HTTP/2 performance by 40%".to_string(),
                "New rule engine with wildcard matching".to_string(),
            ],
            checked_at: Utc::now(),
        };

        println!("\n=== Test with highlights ===");
        print_update_notice("0.0.1-alpha", &cache);

        let cache_no_highlights = VersionCache {
            latest_version: "1.0.0".to_string(),
            release_highlights: vec![],
            checked_at: Utc::now(),
        };

        println!("\n=== Test without highlights ===");
        print_update_notice("0.0.1-alpha", &cache_no_highlights);
    }
}
