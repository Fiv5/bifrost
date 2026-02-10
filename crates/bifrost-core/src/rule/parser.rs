use crate::error::{BifrostError, Result};
use crate::matcher::factory::parse_pattern;
use crate::protocol::Protocol;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

use super::types::Rule;

lazy_static::lazy_static! {
    static ref PROTOCOL_REGEX: Regex = Regex::new(r"^([a-zA-Z][a-zA-Z0-9\-]*)://(.*)$").unwrap();
    static ref INLINE_VALUES_REGEX: Regex = Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_.\-]*)\}").unwrap();
    static ref HOST_PORT_REGEX: Regex = Regex::new(r"^([a-zA-Z0-9][-a-zA-Z0-9.]*|\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}|\[[:0-9a-fA-F]+\]):(\d+)(/.*)?$").unwrap();
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

    pub fn set_value(&mut self, key: String, value: String) {
        self.values.insert(key, value);
    }

    pub fn parse_line(&self, line: &str) -> Result<Vec<Rule>> {
        parse_line_with_values(line, &self.values)
    }

    pub fn parse_rules(&self, text: &str) -> Result<Vec<Rule>> {
        parse_rules_with_values(text, &self.values)
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

    let (pattern, protocol_values) = extract_pattern_and_protocols(&parts)?;

    if protocol_values.is_empty() {
        return Err(BifrostError::Parse(format!(
            "No protocol found in rule: {}",
            line
        )));
    }

    let matcher = parse_pattern(&pattern)
        .map_err(|e| BifrostError::Parse(format!("Invalid pattern '{}': {}", pattern, e)))?;
    let matcher = Arc::from(matcher);

    let mut rules = Vec::new();
    for (protocol, value) in protocol_values {
        let rule = Rule::new(
            pattern.clone(),
            Arc::clone(&matcher),
            protocol,
            value,
            line.clone(),
        );
        rules.push(rule);
    }

    Ok(rules)
}

fn parse_rules_with_values(text: &str, values: &HashMap<String, String>) -> Result<Vec<Rule>> {
    let mut rules = Vec::new();
    let mut current_line = String::new();
    let mut start_line_num = 1;

    for (line_num, line) in text.lines().enumerate() {
        let line_num = line_num + 1;
        let trimmed = line.trim();

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
            let parsed = parse_line_with_values(&current_line, values)?;
            for mut rule in parsed {
                rule.line = Some(start_line_num);
                rules.push(rule);
            }
            current_line.clear();
        } else {
            let parsed = parse_line_with_values(trimmed, values)?;
            for mut rule in parsed {
                rule.line = Some(line_num);
                rules.push(rule);
            }
        }
    }

    if !current_line.is_empty() {
        let parsed = parse_line_with_values(&current_line, values)?;
        for mut rule in parsed {
            rule.line = Some(start_line_num);
            rules.push(rule);
        }
    }

    Ok(rules)
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
                result = result.replacen(full_match, value, 1);
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

fn extract_pattern_and_protocols(parts: &[String]) -> Result<(String, Vec<(Protocol, String)>)> {
    if parts.is_empty() {
        return Err(BifrostError::Parse("Empty rule".to_string()));
    }

    let mut pattern_idx = None;
    let mut protocol_values = Vec::new();

    for (idx, part) in parts.iter().enumerate() {
        if let Some(caps) = PROTOCOL_REGEX.captures(part) {
            let proto_name = caps.get(1).unwrap().as_str();
            let raw_value = caps.get(2).unwrap().as_str();
            let value = if raw_value.starts_with('`') && raw_value.ends_with('`') {
                raw_value[1..raw_value.len() - 1].to_string()
            } else {
                raw_value.to_string()
            };

            if let Some(protocol) = Protocol::parse(proto_name) {
                protocol_values.push((protocol, value));
            } else {
                let resolved = Protocol::resolve_alias(proto_name);
                if let Some(protocol) = Protocol::parse(resolved) {
                    protocol_values.push((protocol, value));
                } else if pattern_idx.is_none() {
                    pattern_idx = Some(idx);
                }
            }
        } else if HOST_PORT_REGEX.is_match(part) {
            protocol_values.push((Protocol::Host, part.clone()));
        } else if pattern_idx.is_none() {
            pattern_idx = Some(idx);
        }
    }

    let pattern = match pattern_idx {
        Some(idx) => parts[idx].clone(),
        None => return Err(BifrostError::Parse("No pattern found in rule".to_string())),
    };

    Ok((pattern, protocol_values))
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
        assert!(HOST_PORT_REGEX.is_match("my-server.local:3000"));
        assert!(HOST_PORT_REGEX.is_match("[::1]:8080"));
        assert!(HOST_PORT_REGEX.is_match("[::1]:8080/api"));
        assert!(!HOST_PORT_REGEX.is_match("example.com"));
        assert!(!HOST_PORT_REGEX.is_match("host://127.0.0.1:3000"));
        assert!(!HOST_PORT_REGEX.is_match("*.example.com"));
    }
}
