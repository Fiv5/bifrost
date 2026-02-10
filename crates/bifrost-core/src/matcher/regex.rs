use regex::Regex;

use super::{MatchResult, Matcher};

#[derive(Debug, Clone, Default)]
pub struct RegexFlags {
    pub case_insensitive: bool,
    pub unicode: bool,
}

pub struct RegexMatcher {
    pattern: Regex,
    negated: bool,
    raw_pattern: String,
    flags: RegexFlags,
}

impl RegexMatcher {
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        let (negated, actual_pattern, flags) = Self::parse_pattern(pattern);

        let regex_pattern = if flags.case_insensitive {
            format!("(?i){}", actual_pattern)
        } else {
            actual_pattern.to_string()
        };

        let compiled = Regex::new(&regex_pattern)?;

        Ok(Self {
            pattern: compiled,
            negated,
            raw_pattern: pattern.to_string(),
            flags,
        })
    }

    fn parse_pattern(pattern: &str) -> (bool, &str, RegexFlags) {
        let mut input = pattern;
        let mut negated = false;

        if input.starts_with('!') {
            negated = true;
            input = &input[1..];
        }

        if input.starts_with('/') && input.len() > 1 {
            if let Some((inner, flags_str)) = Self::parse_with_flags(input) {
                let flags = Self::parse_flags(flags_str);
                return (negated, inner, flags);
            }
        }

        (negated, input, RegexFlags::default())
    }

    fn parse_with_flags(input: &str) -> Option<(&str, &str)> {
        if !input.starts_with('/') {
            return None;
        }

        let content = &input[1..];
        if let Some(last_slash) = content.rfind('/') {
            let inner = &content[..last_slash];
            let flags = &content[last_slash + 1..];
            Some((inner, flags))
        } else {
            None
        }
    }

    fn parse_flags(flags_str: &str) -> RegexFlags {
        let mut flags = RegexFlags::default();
        for c in flags_str.chars() {
            match c {
                'i' => flags.case_insensitive = true,
                'u' => flags.unicode = true,
                _ => {}
            }
        }
        flags
    }

    pub fn flags(&self) -> &RegexFlags {
        &self.flags
    }

    pub fn raw_pattern(&self) -> &str {
        &self.raw_pattern
    }

    pub fn captures(&self, text: &str) -> Option<Vec<String>> {
        self.pattern.captures(text).map(|caps| {
            caps.iter()
                .skip(1)
                .filter_map(|m| m.map(|m| m.as_str().to_string()))
                .collect()
        })
    }
}

impl Matcher for RegexMatcher {
    fn matches(&self, url: &str, _host: &str, _path: &str) -> MatchResult {
        let is_match = self.pattern.is_match(url);
        let effective_match = if self.negated { !is_match } else { is_match };

        if effective_match {
            if let Some(caps) = self.captures(url) {
                if !caps.is_empty() {
                    return MatchResult::matched_with_captures(caps);
                }
            }
            MatchResult::matched()
        } else {
            MatchResult::not_matched()
        }
    }

    fn is_negated(&self) -> bool {
        self.negated
    }

    fn priority(&self) -> i32 {
        80
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_regex() {
        let matcher = RegexMatcher::new("/example\\.com/").unwrap();
        let result = matcher.matches("http://example.com/path", "example.com", "/path");
        assert!(result.matched);
        assert!(!matcher.is_negated());
    }

    #[test]
    fn test_regex_no_match() {
        let matcher = RegexMatcher::new("/example\\.com/").unwrap();
        let result = matcher.matches("http://other.com/path", "other.com", "/path");
        assert!(!result.matched);
    }

    #[test]
    fn test_case_insensitive_regex() {
        let matcher = RegexMatcher::new("/EXAMPLE\\.COM/i").unwrap();
        let result = matcher.matches("http://example.com/path", "example.com", "/path");
        assert!(result.matched);
    }

    #[test]
    fn test_case_sensitive_regex_no_match() {
        let matcher = RegexMatcher::new("/EXAMPLE\\.COM/").unwrap();
        let result = matcher.matches("http://example.com/path", "example.com", "/path");
        assert!(!result.matched);
    }

    #[test]
    fn test_negated_regex() {
        let matcher = RegexMatcher::new("!/example\\.com/").unwrap();
        assert!(matcher.is_negated());

        let result = matcher.matches("http://example.com/path", "example.com", "/path");
        assert!(!result.matched);

        let result = matcher.matches("http://other.com/path", "other.com", "/path");
        assert!(result.matched);
    }

    #[test]
    fn test_negated_case_insensitive() {
        let matcher = RegexMatcher::new("!/EXAMPLE\\.COM/i").unwrap();
        assert!(matcher.is_negated());

        let result = matcher.matches("http://example.com/path", "example.com", "/path");
        assert!(!result.matched);
    }

    #[test]
    fn test_capture_groups() {
        let matcher = RegexMatcher::new("/https?://([^/]+)(/.*)$/").unwrap();
        let result = matcher.matches("https://example.com/api/users", "example.com", "/api/users");
        assert!(result.matched);
        assert!(result.captures.is_some());
        let captures = result.captures.unwrap();
        assert_eq!(captures.len(), 2);
        assert_eq!(captures[0], "example.com");
        assert_eq!(captures[1], "/api/users");
    }

    #[test]
    fn test_capture_groups_variable_substitution() {
        let matcher = RegexMatcher::new("/api/v(\\d+)/users/(\\d+)/").unwrap();
        let result = matcher.matches(
            "http://example.com/api/v2/users/123/profile",
            "example.com",
            "/api/v2/users/123/profile",
        );
        assert!(result.matched);
        assert!(result.captures.is_some());
        let captures = result.captures.unwrap();
        assert_eq!(captures[0], "2");
        assert_eq!(captures[1], "123");
    }

    #[test]
    fn test_complex_url_pattern() {
        let matcher = RegexMatcher::new("/^https?://.*\\.google\\.com/").unwrap();

        let result = matcher.matches("https://www.google.com/search", "www.google.com", "/search");
        assert!(result.matched);

        let result = matcher.matches("http://mail.google.com/inbox", "mail.google.com", "/inbox");
        assert!(result.matched);

        let result = matcher.matches("https://google.com/search", "google.com", "/search");
        assert!(!result.matched);
    }

    #[test]
    fn test_priority() {
        let matcher = RegexMatcher::new("/test/").unwrap();
        assert_eq!(matcher.priority(), 80);
    }

    #[test]
    fn test_raw_pattern() {
        let pattern = "/example\\.com/i";
        let matcher = RegexMatcher::new(pattern).unwrap();
        assert_eq!(matcher.raw_pattern(), pattern);
    }

    #[test]
    fn test_pattern_without_delimiters() {
        let matcher = RegexMatcher::new("example\\.com").unwrap();
        let result = matcher.matches("http://example.com/path", "example.com", "/path");
        assert!(result.matched);
    }

    #[test]
    fn test_invalid_regex() {
        let result = RegexMatcher::new("/[invalid/");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_captures() {
        let matcher = RegexMatcher::new("/example\\.com/").unwrap();
        let result = matcher.matches("http://example.com/path", "example.com", "/path");
        assert!(result.matched);
        assert!(result.captures.is_none());
    }

    #[test]
    fn test_path_only_match() {
        let matcher = RegexMatcher::new("/\\/api\\/v\\d+\\//").unwrap();
        let result = matcher.matches(
            "http://example.com/api/v1/users",
            "example.com",
            "/api/v1/users",
        );
        assert!(result.matched);
    }

    #[test]
    fn test_unicode_flag() {
        let matcher = RegexMatcher::new("/测试/u").unwrap();
        assert!(matcher.flags().unicode);
        assert!(!matcher.flags().case_insensitive);

        let result = matcher.matches("http://example.com/测试/path", "example.com", "/测试/path");
        assert!(result.matched);
    }

    #[test]
    fn test_unicode_case_insensitive_flag() {
        let matcher = RegexMatcher::new("/ТЕСТ/iu").unwrap();
        assert!(matcher.flags().unicode);
        assert!(matcher.flags().case_insensitive);

        let result = matcher.matches("http://example.com/тест/path", "example.com", "/тест/path");
        assert!(result.matched);
    }

    #[test]
    fn test_unicode_flag_order_ui() {
        let matcher = RegexMatcher::new("/test/ui").unwrap();
        assert!(matcher.flags().unicode);
        assert!(matcher.flags().case_insensitive);
    }

    #[test]
    fn test_unicode_emoji_match() {
        let matcher = RegexMatcher::new("/emoji-🎉/u").unwrap();
        let result = matcher.matches(
            "http://example.com/emoji-🎉/test",
            "example.com",
            "/emoji-🎉/test",
        );
        assert!(result.matched);
    }

    #[test]
    fn test_no_flags() {
        let matcher = RegexMatcher::new("/test/").unwrap();
        assert!(!matcher.flags().unicode);
        assert!(!matcher.flags().case_insensitive);
    }
}
