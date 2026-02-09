use regex::Regex;
use std::collections::HashMap;

lazy_static::lazy_static! {
    static ref CAPTURE_VAR_REGEX: Regex = Regex::new(r"\$(\d+)").unwrap();
    static ref NAMED_VAR_REGEX: Regex = Regex::new(r"\$\{([^}]+)\}").unwrap();
    static ref INLINE_FILE_REGEX: Regex = Regex::new(r"\{([^}]+\.json)\}").unwrap();
}

#[derive(Debug, Clone)]
pub enum TemplateValue {
    Capture(usize),
    Named(String),
    InlineFile(String),
    Literal(String),
}

pub struct TemplateEngine;

impl TemplateEngine {
    pub fn expand(
        template: &str,
        captures: Option<&[String]>,
        values: &HashMap<String, String>,
    ) -> String {
        let mut result = template.to_string();

        result = Self::expand_captures(&result, captures);
        result = Self::expand_named_vars(&result, values);
        result = Self::expand_inline_files(&result, values);

        result
    }

    fn expand_captures(template: &str, captures: Option<&[String]>) -> String {
        if captures.is_none() {
            return template.to_string();
        }

        let caps = captures.unwrap();
        let mut result = template.to_string();

        for cap in CAPTURE_VAR_REGEX.captures_iter(template) {
            let full_match = cap.get(0).unwrap().as_str();
            let index: usize = cap.get(1).unwrap().as_str().parse().unwrap_or(0);

            if index > 0 && index <= caps.len() {
                result = result.replace(full_match, &caps[index - 1]);
            }
        }

        result
    }

    fn expand_named_vars(template: &str, values: &HashMap<String, String>) -> String {
        let mut result = template.to_string();

        for cap in NAMED_VAR_REGEX.captures_iter(template) {
            let full_match = cap.get(0).unwrap().as_str();
            let var_name = cap.get(1).unwrap().as_str();

            if let Some(value) = values.get(var_name) {
                result = result.replace(full_match, value);
            }
        }

        result
    }

    fn expand_inline_files(template: &str, values: &HashMap<String, String>) -> String {
        let mut result = template.to_string();

        for cap in INLINE_FILE_REGEX.captures_iter(template) {
            let full_match = cap.get(0).unwrap().as_str();
            let file_key = cap.get(1).unwrap().as_str();

            if let Some(value) = values.get(file_key) {
                result = result.replace(full_match, value);
            }
        }

        result
    }

    pub fn parse_template(template: &str) -> Vec<TemplateValue> {
        let mut parts = Vec::new();
        let mut last_end = 0;
        let mut matches: Vec<(usize, usize, TemplateValue)> = Vec::new();

        for cap in CAPTURE_VAR_REGEX.captures_iter(template) {
            let m = cap.get(0).unwrap();
            let index: usize = cap.get(1).unwrap().as_str().parse().unwrap_or(0);
            matches.push((m.start(), m.end(), TemplateValue::Capture(index)));
        }

        for cap in NAMED_VAR_REGEX.captures_iter(template) {
            let m = cap.get(0).unwrap();
            let name = cap.get(1).unwrap().as_str().to_string();
            matches.push((m.start(), m.end(), TemplateValue::Named(name)));
        }

        for cap in INLINE_FILE_REGEX.captures_iter(template) {
            let m = cap.get(0).unwrap();
            let file = cap.get(1).unwrap().as_str().to_string();
            matches.push((m.start(), m.end(), TemplateValue::InlineFile(file)));
        }

        matches.sort_by_key(|(start, _, _)| *start);

        for (start, end, value) in matches {
            if start > last_end {
                parts.push(TemplateValue::Literal(template[last_end..start].to_string()));
            }
            parts.push(value);
            last_end = end;
        }

        if last_end < template.len() {
            parts.push(TemplateValue::Literal(template[last_end..].to_string()));
        }

        parts
    }

    pub fn has_variables(template: &str) -> bool {
        CAPTURE_VAR_REGEX.is_match(template)
            || NAMED_VAR_REGEX.is_match(template)
            || INLINE_FILE_REGEX.is_match(template)
    }

    pub fn extract_variable_names(template: &str) -> Vec<String> {
        let mut names = Vec::new();

        for cap in NAMED_VAR_REGEX.captures_iter(template) {
            names.push(cap.get(1).unwrap().as_str().to_string());
        }

        for cap in INLINE_FILE_REGEX.captures_iter(template) {
            names.push(cap.get(1).unwrap().as_str().to_string());
        }

        names
    }

    pub fn extract_capture_indices(template: &str) -> Vec<usize> {
        let mut indices = Vec::new();

        for cap in CAPTURE_VAR_REGEX.captures_iter(template) {
            if let Ok(idx) = cap.get(1).unwrap().as_str().parse::<usize>() {
                indices.push(idx);
            }
        }

        indices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_no_variables() {
        let result = TemplateEngine::expand("simple text", None, &HashMap::new());
        assert_eq!(result, "simple text");
    }

    #[test]
    fn test_expand_capture_variables() {
        let captures = vec!["value1".to_string(), "value2".to_string()];
        let result = TemplateEngine::expand("prefix $1 middle $2 suffix", Some(&captures), &HashMap::new());
        assert_eq!(result, "prefix value1 middle value2 suffix");
    }

    #[test]
    fn test_expand_capture_out_of_range() {
        let captures = vec!["value1".to_string()];
        let result = TemplateEngine::expand("$1 and $5", Some(&captures), &HashMap::new());
        assert_eq!(result, "value1 and $5");
    }

    #[test]
    fn test_expand_named_variables() {
        let mut values = HashMap::new();
        values.insert("host".to_string(), "127.0.0.1".to_string());
        values.insert("port".to_string(), "8080".to_string());

        let result = TemplateEngine::expand("${host}:${port}", None, &values);
        assert_eq!(result, "127.0.0.1:8080");
    }

    #[test]
    fn test_expand_named_unknown_variable() {
        let values = HashMap::new();
        let result = TemplateEngine::expand("${unknown}", None, &values);
        assert_eq!(result, "${unknown}");
    }

    #[test]
    fn test_expand_inline_files() {
        let mut values = HashMap::new();
        values.insert("config.json".to_string(), "file_content".to_string());

        let result = TemplateEngine::expand("data: {config.json}", None, &values);
        assert_eq!(result, "data: file_content");
    }

    #[test]
    fn test_expand_mixed_variables() {
        let captures = vec!["captured".to_string()];
        let mut values = HashMap::new();
        values.insert("named".to_string(), "named_value".to_string());
        values.insert("file.json".to_string(), "file_value".to_string());

        let result = TemplateEngine::expand(
            "$1 ${named} {file.json}",
            Some(&captures),
            &values,
        );
        assert_eq!(result, "captured named_value file_value");
    }

    #[test]
    fn test_expand_captures_none() {
        let result = TemplateEngine::expand("$1 $2", None, &HashMap::new());
        assert_eq!(result, "$1 $2");
    }

    #[test]
    fn test_parse_template_literal_only() {
        let parts = TemplateEngine::parse_template("simple text");
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], TemplateValue::Literal(s) if s == "simple text"));
    }

    #[test]
    fn test_parse_template_capture() {
        let parts = TemplateEngine::parse_template("prefix $1 suffix");
        assert_eq!(parts.len(), 3);
        assert!(matches!(&parts[0], TemplateValue::Literal(s) if s == "prefix "));
        assert!(matches!(&parts[1], TemplateValue::Capture(1)));
        assert!(matches!(&parts[2], TemplateValue::Literal(s) if s == " suffix"));
    }

    #[test]
    fn test_parse_template_named() {
        let parts = TemplateEngine::parse_template("${name}");
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], TemplateValue::Named(s) if s == "name"));
    }

    #[test]
    fn test_parse_template_inline_file() {
        let parts = TemplateEngine::parse_template("{data.json}");
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], TemplateValue::InlineFile(s) if s == "data.json"));
    }

    #[test]
    fn test_has_variables_true() {
        assert!(TemplateEngine::has_variables("$1"));
        assert!(TemplateEngine::has_variables("${name}"));
        assert!(TemplateEngine::has_variables("{file.json}"));
    }

    #[test]
    fn test_has_variables_false() {
        assert!(!TemplateEngine::has_variables("simple text"));
        assert!(!TemplateEngine::has_variables("no variables here"));
    }

    #[test]
    fn test_extract_variable_names() {
        let names = TemplateEngine::extract_variable_names("${host}:${port} {config.json}");
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"host".to_string()));
        assert!(names.contains(&"port".to_string()));
        assert!(names.contains(&"config.json".to_string()));
    }

    #[test]
    fn test_extract_capture_indices() {
        let indices = TemplateEngine::extract_capture_indices("$1 $2 $3");
        assert_eq!(indices, vec![1, 2, 3]);
    }

    #[test]
    fn test_expand_url_template() {
        let captures = vec!["v2".to_string(), "123".to_string()];
        let result = TemplateEngine::expand(
            "http://api.local/api/$1/users/$2",
            Some(&captures),
            &HashMap::new(),
        );
        assert_eq!(result, "http://api.local/api/v2/users/123");
    }

    #[test]
    fn test_expand_complex_template() {
        let captures = vec!["example.com".to_string()];
        let mut values = HashMap::new();
        values.insert("target".to_string(), "127.0.0.1".to_string());

        let result = TemplateEngine::expand(
            "Host: $1, Target: ${target}",
            Some(&captures),
            &values,
        );
        assert_eq!(result, "Host: example.com, Target: 127.0.0.1");
    }

    #[test]
    fn test_expand_multiple_same_variable() {
        let captures = vec!["value".to_string()];
        let result = TemplateEngine::expand("$1 $1 $1", Some(&captures), &HashMap::new());
        assert_eq!(result, "value value value");
    }

    #[test]
    fn test_expand_zero_index_capture() {
        let captures = vec!["value1".to_string()];
        let result = TemplateEngine::expand("$0 $1", Some(&captures), &HashMap::new());
        assert_eq!(result, "$0 value1");
    }

    #[test]
    fn test_template_value_debug() {
        let capture = TemplateValue::Capture(1);
        let debug_str = format!("{:?}", capture);
        assert!(debug_str.contains("Capture"));
    }

    #[test]
    fn test_template_value_clone() {
        let original = TemplateValue::Named("test".to_string());
        let cloned = original.clone();
        assert!(matches!(cloned, TemplateValue::Named(s) if s == "test"));
    }
}
