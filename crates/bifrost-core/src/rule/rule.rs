use crate::matcher::Matcher;
use crate::protocol::Protocol;
use std::fmt;
use std::sync::Arc;

pub struct Rule {
    pub pattern: String,
    pub matcher: Arc<dyn Matcher>,
    pub protocol: Protocol,
    pub value: String,
    pub raw: String,
    pub file: Option<String>,
    pub line: Option<usize>,
}

impl Rule {
    pub fn new(
        pattern: String,
        matcher: Arc<dyn Matcher>,
        protocol: Protocol,
        value: String,
        raw: String,
    ) -> Self {
        Self {
            pattern,
            matcher,
            protocol,
            value,
            raw,
            file: None,
            line: None,
        }
    }

    pub fn with_source(mut self, file: String, line: usize) -> Self {
        self.file = Some(file);
        self.line = Some(line);
        self
    }

    pub fn priority(&self) -> i32 {
        self.matcher.priority()
    }

    pub fn is_negated(&self) -> bool {
        self.matcher.is_negated()
    }
}

impl fmt::Debug for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rule")
            .field("pattern", &self.pattern)
            .field("protocol", &self.protocol)
            .field("value", &self.value)
            .field("raw", &self.raw)
            .field("file", &self.file)
            .field("line", &self.line)
            .finish()
    }
}

impl Clone for Rule {
    fn clone(&self) -> Self {
        Self {
            pattern: self.pattern.clone(),
            matcher: Arc::clone(&self.matcher),
            protocol: self.protocol,
            value: self.value.clone(),
            raw: self.raw.clone(),
            file: self.file.clone(),
            line: self.line,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::WildcardMatcher;

    #[test]
    fn test_rule_new() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::Host,
            "127.0.0.1".to_string(),
            "*.example.com host://127.0.0.1".to_string(),
        );

        assert_eq!(rule.pattern, "*.example.com");
        assert_eq!(rule.protocol, Protocol::Host);
        assert_eq!(rule.value, "127.0.0.1");
        assert!(rule.file.is_none());
        assert!(rule.line.is_none());
    }

    #[test]
    fn test_rule_with_source() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::Host,
            "127.0.0.1".to_string(),
            "*.example.com host://127.0.0.1".to_string(),
        )
        .with_source("rules.txt".to_string(), 10);

        assert_eq!(rule.file, Some("rules.txt".to_string()));
        assert_eq!(rule.line, Some(10));
    }

    #[test]
    fn test_rule_priority() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::Host,
            "127.0.0.1".to_string(),
            "*.example.com host://127.0.0.1".to_string(),
        );

        assert_eq!(rule.priority(), 55);
    }

    #[test]
    fn test_rule_is_negated() {
        let matcher = Arc::new(WildcardMatcher::new("!*.example.com").unwrap());
        let rule = Rule::new(
            "!*.example.com".to_string(),
            matcher,
            Protocol::Ignore,
            "".to_string(),
            "!*.example.com ignore://".to_string(),
        );

        assert!(rule.is_negated());
    }

    #[test]
    fn test_rule_clone() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::Host,
            "127.0.0.1".to_string(),
            "*.example.com host://127.0.0.1".to_string(),
        )
        .with_source("rules.txt".to_string(), 10);

        let cloned = rule.clone();
        assert_eq!(cloned.pattern, rule.pattern);
        assert_eq!(cloned.protocol, rule.protocol);
        assert_eq!(cloned.value, rule.value);
        assert_eq!(cloned.file, rule.file);
        assert_eq!(cloned.line, rule.line);
    }

    #[test]
    fn test_rule_debug() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::Host,
            "127.0.0.1".to_string(),
            "*.example.com host://127.0.0.1".to_string(),
        );

        let debug_str = format!("{:?}", rule);
        assert!(debug_str.contains("Rule"));
        assert!(debug_str.contains("*.example.com"));
    }
}
