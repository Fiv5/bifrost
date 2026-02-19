use crate::error::{BifrostError, Result};
use crate::matcher::factory::parse_pattern;
use crate::protocol::Protocol;
use crate::rule::filter::{parse_filter, parse_line_props, Filter, LineProps};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

use super::types::Rule;
use super::ValueStore;

const INCLUDE_FILTER_PREFIX: &str = "includeFilter://";
const EXCLUDE_FILTER_PREFIX: &str = "excludeFilter://";
const LINE_PROPS_PREFIX: &str = "lineProps://";

type ParsedPatternResult = (
    Vec<String>,
    Vec<(Protocol, String)>,
    Vec<Filter>,
    Vec<Filter>,
    LineProps,
);

lazy_static::lazy_static! {
    static ref PROTOCOL_REGEX: Regex = Regex::new(r"^([a-zA-Z][a-zA-Z0-9\-]*)://(.*)$").unwrap();
    static ref INLINE_VALUES_REGEX: Regex = Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_.\-]*)\}").unwrap();
    static ref HOST_PORT_REGEX: Regex = Regex::new(r"^(localhost|\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}|\[[:0-9a-fA-F]+\]):(\d+)(/.*)?$").unwrap();
}

pub struct RuleParser {
    values: HashMap<String, String>,
}

impl RuleParser {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn with_values(values: HashMap<String, String>) -> Self {
        Self { values }
    }

    pub fn from_store(store: &dyn ValueStore) -> Self {
        Self {
            values: store.as_hashmap(),
        }
    }

    pub fn set_value(&mut self, key: String, value: String) {
        self.values.insert(key, value);
    }

    pub fn merge_from_store(&mut self, store: &dyn ValueStore) {
        for (k, v) in store.list() {
            self.values.entry(k).or_insert(v);
        }
    }

    pub fn values(&self) -> &HashMap<String, String> {
        &self.values
    }

    pub fn parse_line(&self, line: &str) -> Result<Vec<Rule>> {
        parse_line_with_values(line, &self.values)
    }

    pub fn parse_rules(&self, text: &str) -> Result<Vec<Rule>> {
        parse_rules_with_values(text, &self.values)
    }

    pub fn parse_rules_with_inline_values(
        &self,
        text: &str,
    ) -> Result<(Vec<Rule>, HashMap<String, String>)> {
        let mut merged_values = self.values.clone();
        let text = extract_markdown_value_blocks(text, &mut merged_values);

        let rules = parse_rules_with_values(&text, &merged_values)?;
        let inline_values: HashMap<String, String> = merged_values
            .into_iter()
            .filter(|(k, _)| !self.values.contains_key(k))
            .collect();

        Ok((rules, inline_values))
    }
}

impl Default for RuleParser {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_line(line: &str) -> Result<Vec<Rule>> {
    parse_line_with_values(line, &HashMap::new())
}

pub fn parse_rules(text: &str) -> Result<Vec<Rule>> {
    parse_rules_with_values(text, &HashMap::new())
}

fn parse_line_with_values(line: &str, values: &HashMap<String, String>) -> Result<Vec<Rule>> {
    let line = line.trim();

    if line.is_empty() || line.starts_with('#') {
        return Ok(vec![]);
    }

    let line = expand_inline_values(line, values);

    let parts = split_rule_parts(&line);
    if parts.is_empty() {
        return Ok(vec![]);
    }

    let (patterns, protocol_values, include_filters, exclude_filters, line_props) =
        extract_pattern_and_protocols(&parts)?;

    if protocol_values.is_empty() {
        return Err(BifrostError::Parse(format!(
            "No protocol found in rule: {}",
            line
        )));
    }

    let mut rules = Vec::new();
    for pattern in patterns {
        let matcher = parse_pattern(&pattern)
            .map_err(|e| BifrostError::Parse(format!("Invalid pattern '{}': {}", pattern, e)))?;
        let matcher = Arc::from(matcher);

        for (protocol, value) in &protocol_values {
            let rule = Rule::new(
                pattern.clone(),
                Arc::clone(&matcher),
                *protocol,
                value.clone(),
                line.clone(),
            )
            .with_line_props(line_props.clone())
            .with_include_filters(include_filters.clone())
            .with_exclude_filters(exclude_filters.clone());
            rules.push(rule);
        }
    }

    Ok(rules)
}

fn parse_rules_with_values(text: &str, values: &HashMap<String, String>) -> Result<Vec<Rule>> {
    let mut merged_values = values.clone();
    let text = extract_markdown_value_blocks(text, &mut merged_values);

    let mut rules = Vec::new();
    let mut current_line = String::new();
    let mut start_line_num = 1;
    let mut in_line_block = false;
    let mut line_block_content = String::new();
    let mut line_block_start = 1;

    for (line_num, line) in text.lines().enumerate() {
        let line_num = line_num + 1;
        let trimmed = line.trim();

        if in_line_block {
            if trimmed == "`" {
                in_line_block = false;
                let block_line = line_block_content.trim().replace('\n', " ");
                let parsed = parse_line_with_values(&block_line, &merged_values)?;
                for mut rule in parsed {
                    rule.line = Some(line_block_start);
                    rules.push(rule);
                }
                line_block_content.clear();
            } else {
                line_block_content.push_str(trimmed);
                line_block_content.push('\n');
            }
            continue;
        }

        if trimmed.starts_with("line`") {
            in_line_block = true;
            line_block_start = line_num;
            let after_marker = trimmed.strip_prefix("line`").unwrap_or("");
            if !after_marker.is_empty() {
                line_block_content.push_str(after_marker);
                line_block_content.push('\n');
            }
            continue;
        }

        if let Some(stripped) = trimmed.strip_suffix('\\') {
            if current_line.is_empty() {
                start_line_num = line_num;
            }
            current_line.push_str(stripped);
            current_line.push(' ');
            continue;
        }

        if !current_line.is_empty() {
            current_line.push_str(trimmed);
            let parsed = parse_line_with_values(&current_line, &merged_values)?;
            for mut rule in parsed {
                rule.line = Some(start_line_num);
                rules.push(rule);
            }
            current_line.clear();
        } else {
            let parsed = parse_line_with_values(trimmed, &merged_values)?;
            for mut rule in parsed {
                rule.line = Some(line_num);
                rules.push(rule);
            }
        }
    }

    if !current_line.is_empty() {
        let parsed = parse_line_with_values(&current_line, &merged_values)?;
        for mut rule in parsed {
            rule.line = Some(start_line_num);
            rules.push(rule);
        }
    }

    if in_line_block && !line_block_content.is_empty() {
        let block_line = line_block_content.trim().replace('\n', " ");
        let parsed = parse_line_with_values(&block_line, &merged_values)?;
        for mut rule in parsed {
            rule.line = Some(line_block_start);
            rules.push(rule);
        }
    }

    Ok(rules)
}

fn extract_markdown_value_blocks(text: &str, values: &mut HashMap<String, String>) -> String {
    if !text.contains("```") {
        return text.to_string();
    }

    let mut result = String::new();
    let mut chars = text.chars().peekable();
    let mut line_start = true;

    while let Some(c) = chars.next() {
        if line_start && c == '`' {
            let mut backtick_count = 1;
            while chars.peek() == Some(&'`') {
                chars.next();
                backtick_count += 1;
            }

            if backtick_count >= 3 {
                while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                    chars.next();
                }

                let mut key = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch == '\n' || ch == '\r' {
                        break;
                    }
                    if ch.is_whitespace() {
                        break;
                    }
                    key.push(chars.next().unwrap());
                }

                while let Some(&ch) = chars.peek() {
                    if ch == '\n' || ch == '\r' {
                        break;
                    }
                    chars.next();
                }
                if chars.peek() == Some(&'\r') {
                    chars.next();
                }
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }

                let mut content = String::new();
                let closing_pattern: String = "`".repeat(backtick_count);

                loop {
                    let mut line = String::new();
                    let mut at_eol = false;

                    while let Some(&ch) = chars.peek() {
                        if ch == '\n' || ch == '\r' {
                            at_eol = true;
                            break;
                        }
                        line.push(chars.next().unwrap());
                    }

                    let trimmed = line.trim();
                    if trimmed == closing_pattern || trimmed.starts_with(&closing_pattern) {
                        if chars.peek() == Some(&'\r') {
                            chars.next();
                        }
                        if chars.peek() == Some(&'\n') {
                            chars.next();
                        }
                        break;
                    }

                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(&line);

                    if at_eol {
                        if chars.peek() == Some(&'\r') {
                            chars.next();
                        }
                        if chars.peek() == Some(&'\n') {
                            chars.next();
                        }
                    }

                    if chars.peek().is_none() {
                        break;
                    }
                }

                if !key.is_empty() {
                    values.insert(key, content);
                }

                line_start = true;
                continue;
            } else {
                for _ in 0..backtick_count {
                    result.push('`');
                }
            }
        }

        result.push(c);
        line_start = c == '\n';
    }

    result
}

fn expand_inline_values(line: &str, values: &HashMap<String, String>) -> String {
    let mut result = line.to_string();
    let max_iterations = 10;

    for _ in 0..max_iterations {
        let mut changed = false;
        let current = result.clone();

        for caps in INLINE_VALUES_REGEX.captures_iter(&current) {
            let full_match = caps.get(0).unwrap().as_str();
            let match_start = caps.get(0).unwrap().start();

            if match_start > 0 && current.as_bytes()[match_start - 1] == b'$' {
                continue;
            }

            let key = caps.get(1).unwrap().as_str();

            if let Some(value) = values.get(key) {
                if value.contains('\n') || value.contains('\r') {
                    continue;
                }
                let replacement = if value.contains(' ') || value.contains('\t') {
                    format!("`{}`", value)
                } else {
                    value.clone()
                };
                result = result.replacen(full_match, &replacement, 1);
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    result
}

fn split_rule_parts(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_regex = false;
    let mut in_backtick = false;
    let mut brace_depth = 0;
    let mut paren_depth = 0;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '`' if !in_regex => {
                in_backtick = !in_backtick;
                current.push(c);
            }
            '/' if !in_regex
                && !in_backtick
                && current.is_empty()
                && brace_depth == 0
                && paren_depth == 0 =>
            {
                in_regex = true;
                current.push(c);
            }
            '/' if in_regex => {
                current.push(c);
                if let Some(&next) = chars.peek() {
                    if next == 'i' {
                        current.push(chars.next().unwrap());
                    }
                }
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
                in_regex = false;
            }
            '{' if !in_regex && !in_backtick => {
                brace_depth += 1;
                current.push(c);
            }
            '}' if !in_regex && !in_backtick && brace_depth > 0 => {
                brace_depth -= 1;
                current.push(c);
            }
            '(' if !in_regex && !in_backtick => {
                paren_depth += 1;
                current.push(c);
            }
            ')' if !in_regex && !in_backtick && paren_depth > 0 => {
                paren_depth -= 1;
                current.push(c);
            }
            ' ' | '\t' if !in_regex && !in_backtick && brace_depth == 0 && paren_depth == 0 => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn is_target_address(value: &str) -> bool {
    let host_part = if let Some(idx) = value.find('/') {
        &value[..idx]
    } else {
        value
    };
    let host_without_port = if let Some(idx) = host_part.rfind(':') {
        &host_part[..idx]
    } else {
        host_part
    };
    host_without_port == "localhost"
        || host_without_port.starts_with("127.")
        || host_without_port
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
        || host_without_port.starts_with('[')
}

fn extract_pattern_and_protocols(parts: &[String]) -> Result<ParsedPatternResult> {
    if parts.is_empty() {
        return Err(BifrostError::Parse("Empty rule".to_string()));
    }

    let mut patterns = Vec::new();
    let mut protocol_values = Vec::new();
    let mut include_filters = Vec::new();
    let mut exclude_filters = Vec::new();
    let mut line_props = LineProps::default();

    for part in parts.iter() {
        if let Some(stripped) = part.strip_prefix(INCLUDE_FILTER_PREFIX) {
            let filter_value = strip_backticks(stripped);
            if let Some(filter) = parse_filter(&filter_value) {
                include_filters.push(filter);
            }
            continue;
        }

        if let Some(stripped) = part.strip_prefix(EXCLUDE_FILTER_PREFIX) {
            let filter_value = strip_backticks(stripped);
            if let Some(filter) = parse_filter(&filter_value) {
                exclude_filters.push(filter);
            }
            continue;
        }

        if let Some(stripped) = part.strip_prefix(LINE_PROPS_PREFIX) {
            let props_value = strip_backticks(stripped);
            line_props = parse_line_props(&props_value);
            continue;
        }

        if let Some(caps) = PROTOCOL_REGEX.captures(part) {
            let proto_name = caps.get(1).unwrap().as_str();
            let raw_value = caps.get(2).unwrap().as_str();
            let value = if raw_value.starts_with('`') && raw_value.ends_with('`') {
                raw_value[1..raw_value.len() - 1].to_string()
            } else {
                raw_value.to_string()
            };

            if let Some(protocol) = Protocol::parse(proto_name) {
                if (protocol == Protocol::Http
                    || protocol == Protocol::Https
                    || protocol == Protocol::Ws
                    || protocol == Protocol::Wss)
                    && !is_target_address(&value)
                {
                    let reconstructed_url = format!("{}://{}", proto_name.to_lowercase(), value);
                    if patterns.iter().any(|p: &String| {
                        let pattern_url = if p.starts_with("http://")
                            || p.starts_with("https://")
                            || p.starts_with("ws://")
                            || p.starts_with("wss://")
                        {
                            p.clone()
                        } else {
                            format!("{}://{}", proto_name.to_lowercase(), p)
                        };
                        pattern_url == reconstructed_url
                    }) {
                        protocol_values.push((Protocol::Ignore, String::new()));
                    } else {
                        patterns.push(part.clone());
                    }
                } else {
                    protocol_values.push((protocol, value));
                }
            } else {
                let resolved = Protocol::resolve_alias(proto_name);
                if let Some(protocol) = Protocol::parse(resolved) {
                    protocol_values.push((protocol, value));
                } else {
                    patterns.push(part.clone());
                }
            }
        } else if HOST_PORT_REGEX.is_match(part) {
            protocol_values.push((Protocol::Host, part.clone()));
        } else {
            patterns.push(part.clone());
        }
    }

    if patterns.is_empty() {
        return Err(BifrostError::Parse("No pattern found in rule".to_string()));
    }

    Ok((
        patterns,
        protocol_values,
        include_filters,
        exclude_filters,
        line_props,
    ))
}

fn strip_backticks(s: &str) -> String {
    if s.starts_with('`') && s.ends_with('`') && s.len() >= 2 {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let rules = parse_line("example.com host://127.0.0.1").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "example.com");
        assert_eq!(rules[0].protocol, Protocol::Host);
        assert_eq!(rules[0].value, "127.0.0.1");
    }

    #[test]
    fn test_parse_multi_protocol_rule() {
        let rules = parse_line("example.com host://127.0.0.1 reqHeaders://{test=1}").unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].protocol, Protocol::Host);
        assert_eq!(rules[1].protocol, Protocol::ReqHeaders);
    }

    #[test]
    fn test_parse_comment_line() {
        let rules = parse_line("# this is a comment").unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_parse_empty_line() {
        let rules = parse_line("").unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_parse_whitespace_line() {
        let rules = parse_line("   \t  ").unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_parse_wildcard_pattern() {
        let rules = parse_line("*.example.com host://127.0.0.1").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "*.example.com");
    }

    #[test]
    fn test_parse_regex_pattern() {
        let rules = parse_line("/example\\.com/ host://127.0.0.1").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "/example\\.com/");
    }

    #[test]
    fn test_parse_regex_pattern_case_insensitive() {
        let rules = parse_line("/example\\.com/i host://127.0.0.1").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "/example\\.com/i");
    }

    #[test]
    fn test_parse_rules_multiline() {
        let text = r#"
# Comment line
example.com host://127.0.0.1
*.api.com proxy://proxy.local:8080

192.168.1.1 ignore://
"#;
        let rules = parse_rules(text).unwrap();
        assert_eq!(rules.len(), 3);
    }

    #[test]
    fn test_parse_rules_continuation() {
        let text = r#"example.com \
host://127.0.0.1 \
reqHeaders://{test=1}"#;
        let rules = parse_rules(text).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].line, Some(1));
        assert_eq!(rules[1].line, Some(1));
    }

    #[test]
    fn test_parse_with_inline_values() {
        let mut values = HashMap::new();
        values.insert("config.json".to_string(), "value_from_json".to_string());

        let parser = RuleParser::with_values(values);
        let result = parser.parse_line("example.com host://{config.json}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_split_rule_parts() {
        let parts = split_rule_parts("example.com host://127.0.0.1");
        assert_eq!(parts, vec!["example.com", "host://127.0.0.1"]);
    }

    #[test]
    fn test_split_rule_parts_with_regex() {
        let parts = split_rule_parts("/test\\.com/ host://127.0.0.1");
        assert_eq!(parts, vec!["/test\\.com/", "host://127.0.0.1"]);
    }

    #[test]
    fn test_split_rule_parts_with_regex_case_insensitive() {
        let parts = split_rule_parts("/test\\.com/i host://127.0.0.1");
        assert_eq!(parts, vec!["/test\\.com/i", "host://127.0.0.1"]);
    }

    #[test]
    fn test_protocol_alias_resolution() {
        let rules = parse_line("example.com hosts://127.0.0.1").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].protocol, Protocol::Host);
    }

    #[test]
    fn test_parse_ip_pattern() {
        let rules = parse_line("192.168.1.1 ignore://").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "192.168.1.1");
    }

    #[test]
    fn test_parse_identical_url_rule() {
        let rules =
            parse_line("https://example.com/api/ https://example.com/api/").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "https://example.com/api/");
        assert_eq!(rules[0].protocol, Protocol::Ignore);
    }

    #[test]
    fn test_parse_identical_url_rule_without_trailing_slash() {
        let rules =
            parse_line("https://example.com/api https://example.com/api").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "https://example.com/api");
        assert_eq!(rules[0].protocol, Protocol::Ignore);
    }

    #[test]
    fn test_parse_cidr_pattern() {
        let rules = parse_line("192.168.0.0/16 proxy://proxy.local:8080").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "192.168.0.0/16");
    }

    #[test]
    fn test_parse_invalid_rule_no_protocol() {
        let result = parse_line("example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_rule_parser_set_value() {
        let mut parser = RuleParser::new();
        parser.set_value("key".to_string(), "value".to_string());
        assert_eq!(parser.values.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_expand_inline_values() {
        let mut values = HashMap::new();
        values.insert("data.json".to_string(), "expanded_value".to_string());

        let result = expand_inline_values("test {data.json} end", &values);
        assert_eq!(result, "test expanded_value end");
    }

    #[test]
    fn test_expand_inline_values_no_match() {
        let values = HashMap::new();
        let result = expand_inline_values("test {unknown.json} end", &values);
        assert_eq!(result, "test {unknown.json} end");
    }

    #[test]
    fn test_parse_negated_pattern() {
        let rules = parse_line("!*.example.com ignore://").unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].is_negated());
    }

    #[test]
    fn test_parse_multiple_protocols() {
        let rules =
            parse_line("example.com host://127.0.0.1 proxy://proxy:8080 reqDelay://1000").unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].protocol, Protocol::Host);
        assert_eq!(rules[1].protocol, Protocol::Proxy);
        assert_eq!(rules[2].protocol, Protocol::ReqDelay);
    }

    #[test]
    fn test_parse_rules_with_line_numbers() {
        let text = "line1.com host://1\nline2.com host://2\nline3.com host://3";
        let rules = parse_rules(text).unwrap();
        assert_eq!(rules[0].line, Some(1));
        assert_eq!(rules[1].line, Some(2));
        assert_eq!(rules[2].line, Some(3));
    }

    #[test]
    fn test_rule_parser_default() {
        let parser = RuleParser::default();
        assert!(parser.values.is_empty());
    }

    #[test]
    fn test_expand_inline_values_varname() {
        let mut values = HashMap::new();
        values.insert("myResponse".to_string(), r#"{"ok":true}"#.to_string());

        let result = expand_inline_values("test {myResponse} end", &values);
        assert_eq!(result, r#"test {"ok":true} end"#);
    }

    #[test]
    fn test_expand_inline_values_nested() {
        let mut values = HashMap::new();
        values.insert("inner".to_string(), "resolved_inner".to_string());
        values.insert("outer".to_string(), "prefix_{inner}_suffix".to_string());

        let result = expand_inline_values("{outer}", &values);
        assert_eq!(result, "prefix_resolved_inner_suffix");
    }

    #[test]
    fn test_expand_inline_values_preserve_template_vars() {
        let mut values = HashMap::new();
        values.insert(
            "response".to_string(),
            r#"{"url":"${url}","time":${now}}"#.to_string(),
        );

        let result = expand_inline_values("{response}", &values);
        assert_eq!(result, r#"{"url":"${url}","time":${now}}"#);
    }

    #[test]
    fn test_expand_inline_values_skip_template_syntax() {
        let values = HashMap::new();
        let result = expand_inline_values("${host} and {varName}", &values);
        assert_eq!(result, "${host} and {varName}");
    }

    #[test]
    fn test_expand_inline_values_multiple_same_var() {
        let mut values = HashMap::new();
        values.insert("val".to_string(), "X".to_string());

        let result = expand_inline_values("{val}-{val}-{val}", &values);
        assert_eq!(result, "X-X-X");
    }

    #[test]
    fn test_expand_inline_values_deep_nested() {
        let mut values = HashMap::new();
        values.insert("a".to_string(), "{b}".to_string());
        values.insert("b".to_string(), "{c}".to_string());
        values.insert("c".to_string(), "final".to_string());

        let result = expand_inline_values("{a}", &values);
        assert_eq!(result, "final");
    }

    #[test]
    fn test_parse_with_varname_values() {
        let mut values = HashMap::new();
        values.insert("myHost".to_string(), "127.0.0.1:8080".to_string());

        let parser = RuleParser::with_values(values);
        let rules = parser.parse_line("example.com host://{myHost}").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].value, "127.0.0.1:8080");
    }

    #[test]
    fn test_parse_with_nested_template() {
        let mut values = HashMap::new();
        values.insert(
            "mockBody".to_string(),
            r#"{"host":"${hostname}","path":"${path}"}"#.to_string(),
        );

        let parser = RuleParser::with_values(values);
        let rules = parser
            .parse_line("example.com resBody://{mockBody}")
            .unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].value, r#"{"host":"${hostname}","path":"${path}"}"#);
    }

    #[test]
    fn test_parse_bare_host_port() {
        let rules = parse_line("example.com 127.0.0.1:3000").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "example.com");
        assert_eq!(rules[0].protocol, Protocol::Host);
        assert_eq!(rules[0].value, "127.0.0.1:3000");
    }

    #[test]
    fn test_parse_bare_host_port_with_path() {
        let rules = parse_line("example.com 127.0.0.1:3000/api").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "example.com");
        assert_eq!(rules[0].protocol, Protocol::Host);
        assert_eq!(rules[0].value, "127.0.0.1:3000/api");
    }

    #[test]
    fn test_parse_bare_host_port_with_deep_path() {
        let rules = parse_line("example.com localhost:8080/api/v1/users").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "example.com");
        assert_eq!(rules[0].protocol, Protocol::Host);
        assert_eq!(rules[0].value, "localhost:8080/api/v1/users");
    }

    #[test]
    fn test_parse_bare_host_port_with_other_protocols() {
        let rules = parse_line("example.com 127.0.0.1:3000/api reqHeaders://{x-test=1}").unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].protocol, Protocol::Host);
        assert_eq!(rules[0].value, "127.0.0.1:3000/api");
        assert_eq!(rules[1].protocol, Protocol::ReqHeaders);
    }

    #[test]
    fn test_host_port_regex() {
        assert!(HOST_PORT_REGEX.is_match("127.0.0.1:3000"));
        assert!(HOST_PORT_REGEX.is_match("127.0.0.1:3000/api"));
        assert!(HOST_PORT_REGEX.is_match("localhost:8080"));
        assert!(HOST_PORT_REGEX.is_match("localhost:8080/path/to/api"));
        assert!(HOST_PORT_REGEX.is_match("[::1]:8080"));
        assert!(HOST_PORT_REGEX.is_match("[::1]:8080/api"));
        assert!(!HOST_PORT_REGEX.is_match("example.com"));
        assert!(!HOST_PORT_REGEX.is_match("host://127.0.0.1:3000"));
        assert!(!HOST_PORT_REGEX.is_match("*.example.com"));
        assert!(
            !HOST_PORT_REGEX.is_match("my-server.local:3000"),
            "Domain names with port should be treated as patterns, not host targets"
        );
        assert!(
            !HOST_PORT_REGEX.is_match("portmatch.local:8080"),
            "Domain names with port should be treated as patterns, not host targets"
        );
    }

    #[test]
    fn test_parse_domain_with_port_as_pattern() {
        let rules = parse_line("portmatch.local:8080 http://127.0.0.1:3000").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "portmatch.local:8080");
        assert_eq!(rules[0].protocol, Protocol::Http);
        assert_eq!(rules[0].value, "127.0.0.1:3000");
    }

    #[test]
    fn test_parse_full_url_as_pattern() {
        let rules =
            parse_line("https://full-match.local:443/api/v1 http://127.0.0.1:3000").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "https://full-match.local:443/api/v1");
        assert_eq!(rules[0].protocol, Protocol::Http);
        assert_eq!(rules[0].value, "127.0.0.1:3000");
    }

    #[test]
    fn test_parse_include_filter() {
        let rules = parse_line("example.com host://127.0.0.1 includeFilter://m:GET").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].include_filters.len(), 1);
        assert!(rules[0].exclude_filters.is_empty());
    }

    #[test]
    fn test_parse_exclude_filter() {
        let rules = parse_line("example.com host://127.0.0.1 excludeFilter:///admin/").unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].include_filters.is_empty());
        assert_eq!(rules[0].exclude_filters.len(), 1);
    }

    #[test]
    fn test_parse_multiple_filters() {
        let rules = parse_line(
            "example.com host://127.0.0.1 includeFilter://m:GET includeFilter:///api/ excludeFilter:///admin/",
        )
        .unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].include_filters.len(), 2);
        assert_eq!(rules[0].exclude_filters.len(), 1);
    }

    #[test]
    fn test_parse_line_props_important() {
        let rules = parse_line("example.com host://127.0.0.1 lineProps://important").unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].line_props.important);
        assert!(!rules[0].line_props.disabled);
    }

    #[test]
    fn test_parse_line_props_disabled() {
        let rules = parse_line("example.com host://127.0.0.1 lineProps://disabled").unwrap();
        assert_eq!(rules.len(), 1);
        assert!(!rules[0].line_props.important);
        assert!(rules[0].line_props.disabled);
    }

    #[test]
    fn test_parse_line_props_multiple() {
        let rules =
            parse_line("example.com host://127.0.0.1 lineProps://important,disabled").unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].line_props.important);
        assert!(rules[0].line_props.disabled);
    }

    #[test]
    fn test_important_priority() {
        let rules1 = parse_line("example.com host://127.0.0.1").unwrap();
        let rules2 = parse_line("example.com host://127.0.0.1 lineProps://important").unwrap();

        assert!(rules2[0].priority() > rules1[0].priority());
        assert!(rules2[0].priority() >= 10000);
    }

    #[test]
    fn test_parse_filters_with_backticks() {
        let rules =
            parse_line("example.com host://127.0.0.1 includeFilter://`m:GET,POST`").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].include_filters.len(), 1);
    }

    #[test]
    fn test_parse_line_block_basic() {
        let text = r#"line`
host://127.0.0.1
example.com
`"#;
        let rules = parse_rules(text).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "example.com");
        assert_eq!(rules[0].protocol, Protocol::Host);
    }

    #[test]
    fn test_parse_line_block_with_filters() {
        let text = r#"line`
host://127.0.0.1
example.com
includeFilter://m:GET
excludeFilter:///admin/
`"#;
        let rules = parse_rules(text).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "example.com");
        assert_eq!(rules[0].include_filters.len(), 1);
        assert_eq!(rules[0].exclude_filters.len(), 1);
    }

    #[test]
    fn test_parse_line_block_with_line_props() {
        let text = r#"line`
host://127.0.0.1
example.com
lineProps://important
`"#;
        let rules = parse_rules(text).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].line_props.important);
    }

    #[test]
    fn test_parse_combined_filters_and_props() {
        let rules = parse_line(
            "example.com host://127.0.0.1 includeFilter://m:GET excludeFilter:///admin/ lineProps://important",
        )
        .unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].include_filters.len(), 1);
        assert_eq!(rules[0].exclude_filters.len(), 1);
        assert!(rules[0].line_props.important);
    }

    #[test]
    fn test_is_disabled() {
        let rules1 = parse_line("example.com host://127.0.0.1").unwrap();
        let rules2 = parse_line("example.com host://127.0.0.1 lineProps://disabled").unwrap();

        assert!(!rules1[0].is_disabled());
        assert!(rules2[0].is_disabled());
    }

    #[test]
    fn test_extract_markdown_value_blocks() {
        let mut values = HashMap::new();
        let text = r#"
# Comment
example.com resBody://{my_response}

``` my_response
{"status":"ok"}
```
"#;
        let result = extract_markdown_value_blocks(text, &mut values);
        assert_eq!(
            values.get("my_response"),
            Some(&r#"{"status":"ok"}"#.to_string())
        );
        assert!(!result.contains("```"));
    }

    #[test]
    fn test_extract_markdown_value_blocks_multiple() {
        let mut values = HashMap::new();
        let text = r#"
example.com resBody://{response1}
another.com resBody://{response2}

``` response1
content1
```

``` response2
content2
```
"#;
        let result = extract_markdown_value_blocks(text, &mut values);
        assert_eq!(values.get("response1"), Some(&"content1".to_string()));
        assert_eq!(values.get("response2"), Some(&"content2".to_string()));
        assert!(!result.contains("```"));
    }

    #[test]
    fn test_extract_markdown_value_blocks_multiline() {
        let mut values = HashMap::new();
        let text = r#"
example.com resBody://{json_data}

``` json_data
{
  "name": "test",
  "value": 123
}
```
"#;
        let result = extract_markdown_value_blocks(text, &mut values);
        let expected = r#"{
  "name": "test",
  "value": 123
}"#;
        assert_eq!(values.get("json_data"), Some(&expected.to_string()));
        assert!(!result.contains("```"));
    }

    #[test]
    fn test_parse_rules_with_markdown_value_blocks() {
        let text = r#"
example.com resBody://{custom_response}

``` custom_response
{"custom":"response"}
```
"#;
        let rules = parse_rules(text).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "example.com");
        assert_eq!(rules[0].value, r#"{"custom":"response"}"#);
    }

    #[test]
    fn test_parse_rules_with_multiple_markdown_blocks() {
        let text = r#"
test1.local resBody://{body1}
test2.local resBody://{body2}

``` body1
first content
```

``` body2
second content
```
"#;
        let rules = parse_rules(text).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].value, "first content");
        assert_eq!(rules[1].value, "second content");
    }

    #[test]
    fn test_extract_markdown_blocks_overwrite_existing() {
        let mut values = HashMap::new();
        values.insert("existing".to_string(), "original".to_string());

        let text = r#"
``` existing
new_value
```
"#;
        extract_markdown_value_blocks(text, &mut values);
        assert_eq!(values.get("existing"), Some(&"new_value".to_string()));
    }

    #[test]
    fn test_parse_paren_content_with_space() {
        let rules = parse_line("example.com reqHeaders://(X-Custom: value)").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].protocol, Protocol::ReqHeaders);
        assert_eq!(rules[0].value, "(X-Custom: value)");
    }

    #[test]
    fn test_parse_multiple_protocols_with_paren_content() {
        let rules =
            parse_line("*.api.test tlsIntercept:// reqHeaders://(X-Intercepted: true)").unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].protocol, Protocol::TlsIntercept);
        assert_eq!(rules[1].protocol, Protocol::ReqHeaders);
        assert_eq!(rules[1].value, "(X-Intercepted: true)");
    }

    #[test]
    fn test_split_rule_parts_preserves_paren_content() {
        let parts = split_rule_parts("example.com reqHeaders://(X-Header: with space)");
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "example.com");
        assert_eq!(parts[1], "reqHeaders://(X-Header: with space)");
    }

    #[test]
    fn test_split_rule_parts_multiple_protocols_with_paren() {
        let parts = split_rule_parts("*.test tlsIntercept:// reqHeaders://(Auth: Bearer token)");
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "*.test");
        assert_eq!(parts[1], "tlsIntercept://");
        assert_eq!(parts[2], "reqHeaders://(Auth: Bearer token)");
    }
}
