use super::types::*;

pub struct BifrostFileWriter;

impl BifrostFileWriter {
    fn write_header(file_type: BifrostFileType) -> String {
        format!("{:02} {}", BIFROST_FILE_VERSION, file_type)
    }

    pub fn write_rules(meta: &RuleFileMeta, rules_content: &str) -> String {
        let rule_count = rules_content
            .lines()
            .filter(|l| {
                let trimmed = l.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .count();

        let mut output = Self::write_header(BifrostFileType::Rules);
        output.push_str("\n\n");

        output.push_str("[meta]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_toml_string(&meta.name)));
        output.push_str(&format!("enabled = {}\n", meta.enabled));
        output.push_str(&format!("sort_order = {}\n", meta.sort_order));
        output.push_str(&format!("version = \"{}\"\n", meta.version));
        output.push_str(&format!("created_at = \"{}\"\n", meta.created_at));
        output.push_str(&format!("updated_at = \"{}\"\n", meta.updated_at));
        if let Some(ref desc) = meta.description {
            output.push_str(&format!("description = \"{}\"\n", escape_toml_string(desc)));
        }

        output.push_str("\n[options]\n");
        output.push_str(&format!("rule_count = {}\n", rule_count));

        output.push_str("\n---\n");
        output.push_str(rules_content);

        output
    }

    pub fn write_network(
        name: &str,
        description: Option<&str>,
        records: &[NetworkRecord],
    ) -> Result<String, serde_json::Error> {
        let mut output = Self::write_header(BifrostFileType::Network);
        output.push_str("\n\n");

        let now = chrono::Utc::now().to_rfc3339();
        output.push_str("[meta]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_toml_string(name)));
        output.push_str("version = \"1.0.0\"\n");
        output.push_str(&format!("created_at = \"{}\"\n", now));
        if let Some(desc) = description {
            output.push_str(&format!("description = \"{}\"\n", escape_toml_string(desc)));
        }

        output.push_str("\n[options]\n");
        output.push_str(&format!("count = {}\n", records.len()));
        output.push_str("include_body = true\n");
        output.push_str("include_response = true\n");

        output.push_str("\n---\n");
        output.push_str(&serde_json::to_string_pretty(records)?);

        Ok(output)
    }

    pub fn write_script(
        name: &str,
        description: Option<&str>,
        scripts: &[ScriptItem],
    ) -> Result<String, serde_json::Error> {
        let mut output = Self::write_header(BifrostFileType::Script);
        output.push_str("\n\n");

        let now = chrono::Utc::now().to_rfc3339();
        output.push_str("[meta]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_toml_string(name)));
        output.push_str("version = \"1.0.0\"\n");
        output.push_str(&format!("created_at = \"{}\"\n", now));
        if let Some(desc) = description {
            output.push_str(&format!("description = \"{}\"\n", escape_toml_string(desc)));
        }

        output.push_str("\n[options]\n");
        output.push_str(&format!("count = {}\n", scripts.len()));

        output.push_str("\n---\n");
        output.push_str(&serde_json::to_string_pretty(scripts)?);

        Ok(output)
    }

    pub fn write_values(
        name: &str,
        description: Option<&str>,
        values: &ValuesContent,
    ) -> Result<String, serde_json::Error> {
        let mut output = Self::write_header(BifrostFileType::Values);
        output.push_str("\n\n");

        let now = chrono::Utc::now().to_rfc3339();
        output.push_str("[meta]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_toml_string(name)));
        output.push_str("version = \"1.0.0\"\n");
        output.push_str(&format!("created_at = \"{}\"\n", now));
        if let Some(desc) = description {
            output.push_str(&format!("description = \"{}\"\n", escape_toml_string(desc)));
        }

        output.push_str("\n[options]\n");
        output.push_str(&format!("count = {}\n", values.len()));

        output.push_str("\n---\n");
        output.push_str(&serde_json::to_string_pretty(values)?);

        Ok(output)
    }

    pub fn write_template(
        name: &str,
        description: Option<&str>,
        template: &TemplateContent,
    ) -> Result<String, serde_json::Error> {
        let mut output = Self::write_header(BifrostFileType::Template);
        output.push_str("\n\n");

        let now = chrono::Utc::now().to_rfc3339();
        output.push_str("[meta]\n");
        output.push_str(&format!("name = \"{}\"\n", escape_toml_string(name)));
        output.push_str("version = \"1.0.0\"\n");
        output.push_str(&format!("created_at = \"{}\"\n", now));
        if let Some(desc) = description {
            output.push_str(&format!("description = \"{}\"\n", escape_toml_string(desc)));
        }

        output.push_str("\n[options]\n");
        output.push_str(&format!("request_count = {}\n", template.requests.len()));
        output.push_str(&format!("group_count = {}\n", template.groups.len()));

        output.push_str("\n---\n");
        output.push_str(&serde_json::to_string_pretty(template)?);

        Ok(output)
    }
}

fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_rules() {
        let meta = RuleFileMeta::new("test-rules".to_string());
        let content = "example.com proxy://localhost:3000";

        let output = BifrostFileWriter::write_rules(&meta, content);

        assert!(output.starts_with("01 rules"));
        assert!(output.contains("name = \"test-rules\""));
        assert!(output.contains("enabled = true"));
        assert!(output.contains("---"));
        assert!(output.contains("example.com proxy://localhost:3000"));
    }

    #[test]
    fn test_write_values() {
        let mut values = ValuesContent::new();
        values.insert("key1".to_string(), "value1".to_string());
        values.insert("key2".to_string(), "value2".to_string());

        let output = BifrostFileWriter::write_values("test-values", None, &values).unwrap();

        assert!(output.starts_with("01 values"));
        assert!(output.contains("name = \"test-values\""));
        assert!(output.contains("count = 2"));
        assert!(output.contains("---"));
    }

    #[test]
    fn test_escape_toml_string() {
        assert_eq!(escape_toml_string("hello"), "hello");
        assert_eq!(escape_toml_string("hello\"world"), "hello\\\"world");
        assert_eq!(escape_toml_string("hello\nworld"), "hello\\nworld");
    }
}
