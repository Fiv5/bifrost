use crate::rule::types::Rule;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParseErrorSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableInfo {
    pub name: String,
    pub source: String,
    pub defined_at: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFix {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<(usize, usize)>,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseError {
    pub line: usize,
    pub start_column: usize,
    pub end_column: usize,
    pub message: String,
    pub severity: ParseErrorSeverity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_info: Option<VariableInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fixes: Vec<CodeFix>,
}

impl ParseError {
    pub fn new(line: usize, content: &str, message: impl Into<String>) -> Self {
        let content_len = content.len().max(1);
        Self {
            line,
            start_column: 1,
            end_column: content_len,
            message: message.into(),
            severity: ParseErrorSeverity::Error,
            suggestion: None,
            code: None,
            related_info: None,
            fixes: Vec::new(),
        }
    }

    pub fn with_range(
        line: usize,
        start_column: usize,
        end_column: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            line,
            start_column,
            end_column,
            message: message.into(),
            severity: ParseErrorSeverity::Error,
            suggestion: None,
            code: None,
            related_info: None,
            fixes: Vec::new(),
        }
    }

    pub fn with_severity(mut self, severity: ParseErrorSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_related_info(mut self, info: VariableInfo) -> Self {
        self.related_info = Some(info);
        self
    }

    pub fn with_fix(mut self, fix: CodeFix) -> Self {
        self.fixes.push(fix);
        self
    }

    pub fn with_fixes(mut self, fixes: Vec<CodeFix>) -> Self {
        self.fixes = fixes;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    pub rules: Vec<Rule>,
    pub errors: Vec<ParseError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptReference {
    pub name: String,
    pub script_type: String,
    pub line: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub rule_count: usize,
    pub errors: Vec<ParseError>,
    pub warnings: Vec<ParseError>,
    pub defined_variables: Vec<VariableInfo>,
    #[serde(default)]
    pub script_references: Vec<ScriptReference>,
}
