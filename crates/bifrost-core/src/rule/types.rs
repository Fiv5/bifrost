use crate::matcher::Matcher;
use crate::protocol::Protocol;
use crate::rule::filter::{Filter, LineProps};
use crate::rule::value_source::ValueSource;
use std::fmt;
use std::sync::Arc;

pub struct Rule {
    pub pattern: String,
    pub matcher: Arc<dyn Matcher>,
    pub protocol: Protocol,
    pub value: String,
    pub value_source: ValueSource,
    pub raw: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub line_props: LineProps,
    pub include_filters: Vec<Filter>,
    pub exclude_filters: Vec<Filter>,
}

impl Rule {
    pub fn new(
        pattern: String,
        matcher: Arc<dyn Matcher>,
        protocol: Protocol,
        value: String,
        raw: String,
    ) -> Self {
        let value_source = ValueSource::parse(&value);
        Self {
            pattern,
            matcher,
            protocol,
            value,
            value_source,
            raw,
            file: None,
            line: None,
            line_props: LineProps::default(),
            include_filters: Vec::new(),
            exclude_filters: Vec::new(),
        }
    }

    pub fn with_source(mut self, file: String, line: usize) -> Self {
        self.file = Some(file);
        self.line = Some(line);
        self
    }

    pub fn with_line_props(mut self, line_props: LineProps) -> Self {
        self.line_props = line_props;
        self
    }

    pub fn with_include_filters(mut self, filters: Vec<Filter>) -> Self {
        self.include_filters = filters;
        self
    }

    pub fn with_exclude_filters(mut self, filters: Vec<Filter>) -> Self {
        self.exclude_filters = filters;
        self
    }

    pub fn priority(&self) -> i32 {
        let base = self.matcher.priority();
        if self.line_props.important {
            base + 10000
        } else {
            base
        }
    }

    pub fn is_negated(&self) -> bool {
        self.matcher.is_negated()
    }

    pub fn is_disabled(&self) -> bool {
        self.line_props.disabled
    }
}

impl fmt::Debug for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rule")
            .field("pattern", &self.pattern)
            .field("protocol", &self.protocol)
            .field("value", &self.value)
            .field("value_source", &self.value_source)
            .field("raw", &self.raw)
            .field("file", &self.file)
            .field("line", &self.line)
            .field("line_props", &self.line_props)
            .field("include_filters", &self.include_filters)
            .field("exclude_filters", &self.exclude_filters)
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
            value_source: self.value_source.clone(),
            raw: self.raw.clone(),
            file: self.file.clone(),
            line: self.line,
            line_props: self.line_props.clone(),
            include_filters: self.include_filters.clone(),
            exclude_filters: self.exclude_filters.clone(),
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
        assert_eq!(
            rule.value_source,
            ValueSource::Inline("127.0.0.1".to_string())
        );
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
        assert_eq!(cloned.value_source, rule.value_source);
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

    #[test]
    fn test_rule_value_source_file_path() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::File,
            "/etc/mock.json".to_string(),
            "*.example.com file:///etc/mock.json".to_string(),
        );

        assert_eq!(
            rule.value_source,
            ValueSource::FilePath("/etc/mock.json".to_string())
        );
    }

    #[test]
    fn test_rule_value_source_paren_content() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::ResBody,
            "({\"ok\":true})".to_string(),
            "*.example.com resBody://({\"ok\":true})".to_string(),
        );

        assert_eq!(
            rule.value_source,
            ValueSource::ParenContent("{\"ok\":true}".to_string())
        );
    }

    #[test]
    fn test_rule_value_source_value_ref() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::ResBody,
            "{myResponse}".to_string(),
            "*.example.com resBody://{myResponse}".to_string(),
        );

        assert_eq!(
            rule.value_source,
            ValueSource::ValueRef("myResponse".to_string())
        );
    }

    #[test]
    fn test_rule_value_source_remote_url() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::File,
            "http://example.com/data.json".to_string(),
            "*.example.com file://http://example.com/data.json".to_string(),
        );

        assert_eq!(
            rule.value_source,
            ValueSource::RemoteUrl("http://example.com/data.json".to_string())
        );
    }

    #[test]
    fn test_rule_value_source_inline_params() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::ReqHeaders,
            "X-Custom=test&X-Another=value".to_string(),
            "*.example.com reqHeaders://X-Custom=test&X-Another=value".to_string(),
        );

        match &rule.value_source {
            ValueSource::InlineParams(params) => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], ("X-Custom".to_string(), "test".to_string()));
                assert_eq!(params[1], ("X-Another".to_string(), "value".to_string()));
            }
            _ => panic!("Expected InlineParams"),
        }
    }
}
