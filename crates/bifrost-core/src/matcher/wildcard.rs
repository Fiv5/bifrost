use regex::Regex;

use super::{MatchResult, Matcher};

#[derive(Debug, Clone, PartialEq)]
pub enum WildcardType {
    Prefix,
    Suffix,
    Contains,
    DomainWildcard,
    PathWildcard,
    Mixed,
}

pub struct WildcardMatcher {
    pattern: Regex,
    negated: bool,
    raw_pattern: String,
    wildcard_type: WildcardType,
}

impl WildcardMatcher {
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        let (negated, clean_pattern) = Self::parse_negation(pattern);
        let (has_protocol, pattern_without_protocol) = Self::strip_protocol(clean_pattern);
        let wildcard_type = Self::detect_type(pattern_without_protocol);
        let regex_pattern = Self::to_regex(clean_pattern, &wildcard_type, has_protocol);
        let compiled = Regex::new(&regex_pattern)?;

        Ok(Self {
            pattern: compiled,
            negated,
            raw_pattern: pattern.to_string(),
            wildcard_type,
        })
    }

    fn parse_negation(pattern: &str) -> (bool, &str) {
        if let Some(stripped) = pattern.strip_prefix('!') {
            (true, stripped)
        } else {
            (false, pattern)
        }
    }

    fn strip_protocol(pattern: &str) -> (bool, &str) {
        if let Some(stripped) = pattern.strip_prefix("http://") {
            (true, stripped)
        } else if let Some(stripped) = pattern.strip_prefix("https://") {
            (true, stripped)
        } else if pattern.starts_with('$') {
            (true, pattern)
        } else {
            (false, pattern)
        }
    }

    fn detect_type(pattern: &str) -> WildcardType {
        if pattern.starts_with('$') {
            return WildcardType::DomainWildcard;
        }

        let has_prefix_star = pattern.starts_with('*');
        let has_suffix_star = pattern.ends_with('*');
        let has_path_wildcard = pattern.contains("/*") || pattern.contains("*/");

        let inner_stars = pattern
            .trim_start_matches('*')
            .trim_end_matches('*')
            .contains('*');

        if has_path_wildcard {
            WildcardType::PathWildcard
        } else if has_prefix_star && has_suffix_star {
            WildcardType::Contains
        } else if has_prefix_star {
            WildcardType::Prefix
        } else if has_suffix_star {
            WildcardType::Suffix
        } else if inner_stars {
            WildcardType::Mixed
        } else {
            WildcardType::Suffix
        }
    }

    fn to_regex(pattern: &str, wildcard_type: &WildcardType, has_protocol: bool) -> String {
        let escaped = Self::pattern_to_regex(pattern);

        match wildcard_type {
            WildcardType::DomainWildcard => {
                let domain_pattern = escaped.replace("__DOLLAR__", "");
                format!("^https?://{}(/.*)?$", domain_pattern)
            }
            WildcardType::Prefix => {
                if has_protocol {
                    format!("^{}(/.*)?$", escaped)
                } else {
                    format!("^https?://{}(/.*)?$", escaped)
                }
            }
            WildcardType::Suffix | WildcardType::Contains | WildcardType::Mixed => {
                if has_protocol {
                    format!("^{}(/.*)?$", escaped)
                } else {
                    format!("^https?://{}(/.*)?$", escaped)
                }
            }
            WildcardType::PathWildcard => {
                if has_protocol {
                    format!("^{}$", escaped)
                } else {
                    format!("^https?://{}$", escaped)
                }
            }
        }
    }

    fn pattern_to_regex(pattern: &str) -> String {
        let mut result = String::with_capacity(pattern.len() * 2);
        let special_chars = ['.', '+', '^', '(', ')', '[', ']', '{', '}', '|', '\\'];
        let mut after_slash = false;

        for c in pattern.chars() {
            match c {
                '*' => {
                    if after_slash {
                        result.push_str(".*");
                    } else {
                        result.push_str("[^/]*");
                    }
                }
                '?' => result.push('.'),
                '$' => result.push_str("__DOLLAR__"),
                '/' => {
                    after_slash = true;
                    result.push(c);
                }
                _ if special_chars.contains(&c) => {
                    result.push('\\');
                    result.push(c);
                }
                _ => result.push(c),
            }
        }
        result
    }

    pub fn raw_pattern(&self) -> &str {
        &self.raw_pattern
    }

    pub fn wildcard_type(&self) -> &WildcardType {
        &self.wildcard_type
    }
}

impl Matcher for WildcardMatcher {
    fn matches(&self, url: &str, _host: &str, _path: &str) -> MatchResult {
        let is_match = self.pattern.is_match(url);
        let effective_match = if self.negated { !is_match } else { is_match };

        if effective_match {
            MatchResult::matched()
        } else {
            MatchResult::not_matched()
        }
    }

    fn is_negated(&self) -> bool {
        self.negated
    }

    fn priority(&self) -> i32 {
        match self.wildcard_type {
            WildcardType::DomainWildcard => 50,
            WildcardType::Contains => 40,
            WildcardType::Mixed => 45,
            WildcardType::PathWildcard => 60,
            WildcardType::Prefix => 55,
            WildcardType::Suffix => 55,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_wildcard() {
        let matcher = WildcardMatcher::new("*.example.com").unwrap();
        assert_eq!(matcher.wildcard_type(), &WildcardType::Prefix);

        let result = matcher.matches("http://www.example.com", "www.example.com", "/");
        assert!(result.matched);

        let result = matcher.matches("https://api.example.com", "api.example.com", "/");
        assert!(result.matched);

        let result = matcher.matches("http://example.com", "example.com", "/");
        assert!(!result.matched);
    }

    #[test]
    fn test_prefix_wildcard_subdomain() {
        let matcher = WildcardMatcher::new("*.test.example.com").unwrap();

        let result = matcher.matches("http://api.test.example.com", "api.test.example.com", "/");
        assert!(result.matched);

        let result = matcher.matches("http://test.example.com", "test.example.com", "/");
        assert!(!result.matched);
    }

    #[test]
    fn test_suffix_wildcard() {
        let matcher = WildcardMatcher::new("example.*").unwrap();
        assert_eq!(matcher.wildcard_type(), &WildcardType::Suffix);

        let result = matcher.matches("http://example.com", "example.com", "/");
        assert!(result.matched);

        let result = matcher.matches("http://example.org", "example.org", "/");
        assert!(result.matched);

        let result = matcher.matches("http://example.co.uk", "example.co.uk", "/");
        assert!(result.matched);
    }

    #[test]
    fn test_contains_wildcard() {
        let matcher = WildcardMatcher::new("*example*").unwrap();
        assert_eq!(matcher.wildcard_type(), &WildcardType::Contains);

        let result = matcher.matches("http://www.example.com/path", "www.example.com", "/path");
        assert!(result.matched);

        let result = matcher.matches("http://myexample.org", "myexample.org", "/");
        assert!(result.matched);

        let result = matcher.matches("http://test.com", "test.com", "/");
        assert!(!result.matched);
    }

    #[test]
    fn test_domain_wildcard() {
        let matcher = WildcardMatcher::new("$example.com").unwrap();
        assert_eq!(matcher.wildcard_type(), &WildcardType::DomainWildcard);

        let result = matcher.matches("http://example.com", "example.com", "/");
        assert!(result.matched);

        let result = matcher.matches("https://example.com", "example.com", "/");
        assert!(result.matched);

        let result = matcher.matches("http://example.com/api/test", "example.com", "/api/test");
        assert!(result.matched);
    }

    #[test]
    fn test_domain_wildcard_with_star() {
        let matcher = WildcardMatcher::new("$*.example.com").unwrap();

        let result = matcher.matches("http://api.example.com/path", "api.example.com", "/path");
        assert!(result.matched);

        let result = matcher.matches("https://www.example.com", "www.example.com", "/");
        assert!(result.matched);
    }

    #[test]
    fn test_path_wildcard() {
        let matcher = WildcardMatcher::new("example.com/api/*").unwrap();
        assert_eq!(matcher.wildcard_type(), &WildcardType::PathWildcard);

        let result = matcher.matches("http://example.com/api/users", "example.com", "/api/users");
        assert!(result.matched);

        let result = matcher.matches(
            "http://example.com/api/products/123",
            "example.com",
            "/api/products/123",
        );
        assert!(result.matched);

        let result = matcher.matches("http://example.com/other", "example.com", "/other");
        assert!(!result.matched);
    }

    #[test]
    fn test_path_wildcard_nested() {
        let matcher = WildcardMatcher::new("example.com/api/*/details").unwrap();

        let result = matcher.matches(
            "http://example.com/api/users/details",
            "example.com",
            "/api/users/details",
        );
        assert!(result.matched);

        let result = matcher.matches(
            "http://example.com/api/products/details",
            "example.com",
            "/api/products/details",
        );
        assert!(result.matched);
    }

    #[test]
    fn test_negated_wildcard() {
        let matcher = WildcardMatcher::new("!*.example.com").unwrap();
        assert!(matcher.is_negated());

        let result = matcher.matches("http://www.example.com", "www.example.com", "/");
        assert!(!result.matched);

        let result = matcher.matches("http://other.com", "other.com", "/");
        assert!(result.matched);
    }

    #[test]
    fn test_negated_contains() {
        let matcher = WildcardMatcher::new("!*internal*").unwrap();
        assert!(matcher.is_negated());

        let result = matcher.matches("http://internal.company.com", "internal.company.com", "/");
        assert!(!result.matched);

        let result = matcher.matches("http://public.company.com", "public.company.com", "/");
        assert!(result.matched);
    }

    #[test]
    fn test_with_protocol_http() {
        let matcher = WildcardMatcher::new("http://*.example.com").unwrap();

        let result = matcher.matches("http://www.example.com", "www.example.com", "/");
        assert!(result.matched);
    }

    #[test]
    fn test_with_protocol_https() {
        let matcher = WildcardMatcher::new("https://*.example.com").unwrap();

        let result = matcher.matches("https://api.example.com", "api.example.com", "/");
        assert!(result.matched);
    }

    #[test]
    fn test_priority_values() {
        let domain = WildcardMatcher::new("$example.com").unwrap();
        assert_eq!(domain.priority(), 50);

        let contains = WildcardMatcher::new("*example*").unwrap();
        assert_eq!(contains.priority(), 40);

        let path = WildcardMatcher::new("example.com/*").unwrap();
        assert_eq!(path.priority(), 60);

        let prefix = WildcardMatcher::new("*.example.com").unwrap();
        assert_eq!(prefix.priority(), 55);
    }

    #[test]
    fn test_raw_pattern() {
        let pattern = "*.example.com";
        let matcher = WildcardMatcher::new(pattern).unwrap();
        assert_eq!(matcher.raw_pattern(), pattern);
    }

    #[test]
    fn test_special_chars_escaped() {
        let matcher = WildcardMatcher::new("example.com/path+test").unwrap();
        let result = matcher.matches("http://example.com/path+test", "example.com", "/path+test");
        assert!(result.matched);
    }

    #[test]
    fn test_question_mark_wildcard() {
        let matcher = WildcardMatcher::new("example?.com").unwrap();

        let result = matcher.matches("http://example1.com", "example1.com", "/");
        assert!(result.matched);

        let result = matcher.matches("http://exampleA.com", "exampleA.com", "/");
        assert!(result.matched);
    }

    #[test]
    fn test_complex_wildcard_pattern() {
        let matcher = WildcardMatcher::new("*.example.*/api/*").unwrap();

        let result = matcher.matches(
            "http://www.example.com/api/users",
            "www.example.com",
            "/api/users",
        );
        assert!(result.matched);

        let result = matcher.matches(
            "https://api.example.org/api/products",
            "api.example.org",
            "/api/products",
        );
        assert!(result.matched);
    }

    #[test]
    fn test_multiple_subdomain_levels() {
        let matcher = WildcardMatcher::new("*.*.example.com").unwrap();

        let result = matcher.matches("http://a.b.example.com", "a.b.example.com", "/");
        assert!(result.matched);
    }

    #[test]
    fn test_empty_path() {
        let matcher = WildcardMatcher::new("*.example.com").unwrap();

        let result = matcher.matches("http://www.example.com", "www.example.com", "");
        assert!(result.matched);
    }
}
