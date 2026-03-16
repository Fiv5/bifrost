use std::fs;
use std::path::PathBuf;

use bifrost_core::bifrost_file::{
    BifrostFileParser, BifrostFileWriter, RuleFileMeta as BifrostRuleFileMeta,
    RuleFileOptions as BifrostRuleFileOptions,
};
use bifrost_core::{BifrostError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFile {
    pub name: String,
    pub content: String,
    pub enabled: bool,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_timestamp")]
    pub created_at: String,
    #[serde(default = "default_timestamp")]
    pub updated_at: String,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

impl RuleFile {
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            name: name.into(),
            content: content.into(),
            enabled: true,
            sort_order: 0,
            description: None,
            version: "1.0.0".to_string(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_sort_order(mut self, sort_order: i32) -> Self {
        self.sort_order = sort_order;
        self
    }

    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    fn to_bifrost_meta(&self) -> BifrostRuleFileMeta {
        BifrostRuleFileMeta {
            name: self.name.clone(),
            enabled: self.enabled,
            sort_order: self.sort_order,
            version: self.version.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            description: self.description.clone(),
        }
    }

    fn from_bifrost(meta: BifrostRuleFileMeta, content: String) -> Self {
        Self {
            name: meta.name,
            content,
            enabled: meta.enabled,
            sort_order: meta.sort_order,
            description: meta.description,
            version: meta.version,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleSummary {
    pub name: String,
    pub enabled: bool,
    pub sort_order: i32,
    pub rule_count: usize,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&RuleFile> for RuleSummary {
    fn from(rule: &RuleFile) -> Self {
        let rule_count = rule
            .content
            .lines()
            .filter(|l| {
                let trimmed = l.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .count();

        Self {
            name: rule.name.clone(),
            enabled: rule.enabled,
            sort_order: rule.sort_order,
            rule_count,
            description: rule.description.clone(),
            created_at: rule.created_at.clone(),
            updated_at: rule.updated_at.clone(),
        }
    }
}

#[derive(Deserialize)]
struct RuleSummaryWrapper {
    meta: BifrostRuleFileMeta,
    #[serde(default)]
    options: BifrostRuleFileOptions,
}

#[derive(Clone)]
pub struct RulesStorage {
    base_dir: PathBuf,
}

impl RulesStorage {
    pub fn new() -> Result<Self> {
        let base_dir = crate::data_dir().join("rules");
        Self::with_dir(base_dir)
    }

    pub fn with_dir(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Self { base_dir: dir })
    }

    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }

    fn rule_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(format!("{}.bifrost", name))
    }

    fn legacy_rule_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(format!("{}.json", name))
    }

    pub fn load(&self, name: &str) -> Result<RuleFile> {
        let bifrost_path = self.rule_path(name);
        let legacy_path = self.legacy_rule_path(name);

        if bifrost_path.exists() {
            let content = fs::read_to_string(&bifrost_path)?;
            let file = BifrostFileParser::parse_rules(&content)
                .map_err(|e| BifrostError::Parse(format!("Failed to parse rule file: {}", e)))?;
            Ok(RuleFile::from_bifrost(file.meta, file.content))
        } else if legacy_path.exists() {
            #[derive(Deserialize)]
            struct LegacyRuleFile {
                name: String,
                content: String,
                enabled: bool,
            }
            let content = fs::read_to_string(&legacy_path)?;
            let legacy: LegacyRuleFile = serde_json::from_str(&content).map_err(|e| {
                BifrostError::Parse(format!("Failed to parse legacy rule file: {}", e))
            })?;

            let rule = RuleFile::new(legacy.name, legacy.content).with_enabled(legacy.enabled);
            self.save(&rule)?;
            fs::remove_file(&legacy_path)?;
            Ok(rule)
        } else {
            Err(BifrostError::NotFound(format!("Rule '{}' not found", name)))
        }
    }

    pub fn save(&self, rule: &RuleFile) -> Result<()> {
        let path = self.rule_path(&rule.name);
        let meta = rule.to_bifrost_meta();
        let content = BifrostFileWriter::write_rules(&meta, &rule.content);
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str());
            if ext == Some("bifrost") || ext == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if !names.contains(&stem.to_string()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let bifrost_path = self.rule_path(name);
        let legacy_path = self.legacy_rule_path(name);

        let exists = bifrost_path.exists() || legacy_path.exists();
        if !exists {
            return Err(BifrostError::NotFound(format!("Rule '{}' not found", name)));
        }

        if bifrost_path.exists() {
            fs::remove_file(&bifrost_path)?;
        }
        if legacy_path.exists() {
            fs::remove_file(&legacy_path)?;
        }
        Ok(())
    }

    pub fn rename(&self, old: &str, new: &str) -> Result<()> {
        if !self.exists(old) {
            return Err(BifrostError::NotFound(format!("Rule '{}' not found", old)));
        }
        if self.exists(new) {
            return Err(BifrostError::Config(format!(
                "Rule '{}' already exists",
                new
            )));
        }

        let mut rule = self.load(old)?;
        rule.name = new.to_string();
        rule.touch();
        self.save(&rule)?;
        self.delete(old)?;
        Ok(())
    }

    pub fn exists(&self, name: &str) -> bool {
        self.rule_path(name).exists() || self.legacy_rule_path(name).exists()
    }

    pub fn load_all(&self) -> Result<Vec<RuleFile>> {
        let names = self.list()?;
        let mut rules = Vec::new();
        for name in names {
            match self.load(&name) {
                Ok(rule) => rules.push(rule),
                Err(e) => {
                    tracing::warn!(name = %name, error = %e, "Failed to load rule file, skipping");
                }
            }
        }
        rules.sort_by_key(|r| r.sort_order);
        Ok(rules)
    }

    pub fn load_enabled(&self) -> Result<Vec<RuleFile>> {
        let rules = self.load_all()?;
        Ok(rules.into_iter().filter(|r| r.enabled).collect())
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let mut rule = self.load(name)?;
        rule.enabled = enabled;
        rule.touch();
        self.save(&rule)
    }

    pub fn set_sort_order(&self, name: &str, sort_order: i32) -> Result<()> {
        let mut rule = self.load(name)?;
        rule.sort_order = sort_order;
        rule.touch();
        self.save(&rule)
    }

    pub fn update_content(&self, name: &str, content: String) -> Result<()> {
        let mut rule = self.load(name)?;
        rule.content = content;
        rule.touch();
        self.save(&rule)
    }

    pub fn list_summaries(&self) -> Result<Vec<RuleSummary>> {
        let names = self.list()?;
        let mut summaries = Vec::new();

        for name in names {
            match self.load_summary(&name) {
                Ok(summary) => summaries.push(summary),
                Err(e) => {
                    tracing::warn!(name = %name, error = %e, "Failed to load rule summary, skipping");
                }
            }
        }

        summaries.sort_by_key(|r| r.sort_order);
        Ok(summaries)
    }

    pub fn reorder(&self, order: &[String]) -> Result<()> {
        for (i, name) in order.iter().enumerate() {
            if self.exists(name) {
                self.set_sort_order(name, i as i32)?;
            }
        }
        Ok(())
    }
}

impl RulesStorage {
    fn load_summary(&self, name: &str) -> Result<RuleSummary> {
        let bifrost_path = self.rule_path(name);
        let legacy_path = self.legacy_rule_path(name);

        if bifrost_path.exists() {
            let content = fs::read_to_string(&bifrost_path)?;
            let raw = BifrostFileParser::parse_raw(&content)
                .map_err(|e| BifrostError::Parse(format!("Failed to parse rule file: {}", e)))?;

            let parsed: RuleSummaryWrapper = toml::from_str(&raw.meta_raw)
                .map_err(|e| BifrostError::Parse(format!("Failed to parse rule meta: {}", e)))?;

            Ok(RuleSummary {
                name: parsed.meta.name,
                enabled: parsed.meta.enabled,
                sort_order: parsed.meta.sort_order,
                rule_count: parsed.options.rule_count,
                description: parsed.meta.description,
                created_at: parsed.meta.created_at,
                updated_at: parsed.meta.updated_at,
            })
        } else if legacy_path.exists() {
            #[derive(Deserialize)]
            struct LegacyRuleFile {
                name: String,
                content: String,
                enabled: bool,
            }

            let content = fs::read_to_string(&legacy_path)?;
            let legacy: LegacyRuleFile = serde_json::from_str(&content).map_err(|e| {
                BifrostError::Parse(format!("Failed to parse legacy rule file: {}", e))
            })?;

            Ok(RuleSummary::from(
                &RuleFile::new(legacy.name, legacy.content).with_enabled(legacy.enabled),
            ))
        } else {
            Err(BifrostError::NotFound(format!("Rule '{}' not found", name)))
        }
    }
}

impl Default for RulesStorage {
    fn default() -> Self {
        Self::new().expect("Failed to create default RulesStorage")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, RulesStorage) {
        let temp_dir = TempDir::new().unwrap();
        let storage = RulesStorage::with_dir(temp_dir.path().to_path_buf()).unwrap();
        (temp_dir, storage)
    }

    #[test]
    fn test_save_and_load() {
        let (_temp_dir, storage) = setup();
        let rule = RuleFile::new("test", "example.com resHeaders://x-test=1");
        storage.save(&rule).unwrap();

        let loaded = storage.load("test").unwrap();
        assert_eq!(loaded.name, "test");
        assert_eq!(loaded.content, "example.com resHeaders://x-test=1");
        assert!(loaded.enabled);
    }

    #[test]
    fn test_save_creates_bifrost_file() {
        let (temp_dir, storage) = setup();
        let rule = RuleFile::new("test", "example.com proxy://localhost");
        storage.save(&rule).unwrap();

        let file_path = temp_dir.path().join("test.bifrost");
        assert!(file_path.exists());

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.starts_with("01 rules"));
        assert!(content.contains("[meta]"));
        assert!(content.contains("name = \"test\""));
        assert!(content.contains("---"));
    }

    #[test]
    fn test_load_not_found() {
        let (_temp_dir, storage) = setup();
        let result = storage.load("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list() {
        let (_temp_dir, storage) = setup();
        storage.save(&RuleFile::new("rule1", "content1")).unwrap();
        storage.save(&RuleFile::new("rule2", "content2")).unwrap();
        storage.save(&RuleFile::new("rule3", "content3")).unwrap();

        let names = storage.list().unwrap();
        assert_eq!(names, vec!["rule1", "rule2", "rule3"]);
    }

    #[test]
    fn test_list_empty() {
        let (_temp_dir, storage) = setup();
        let names = storage.list().unwrap();
        assert!(names.is_empty());
    }

    #[test]
    fn test_delete() {
        let (_temp_dir, storage) = setup();
        storage.save(&RuleFile::new("test", "content")).unwrap();
        assert!(storage.exists("test"));

        storage.delete("test").unwrap();
        assert!(!storage.exists("test"));
    }

    #[test]
    fn test_delete_not_found() {
        let (_temp_dir, storage) = setup();
        let result = storage.delete("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_rename() {
        let (_temp_dir, storage) = setup();
        storage.save(&RuleFile::new("old_name", "content")).unwrap();

        storage.rename("old_name", "new_name").unwrap();

        assert!(!storage.exists("old_name"));
        assert!(storage.exists("new_name"));
        let rule = storage.load("new_name").unwrap();
        assert_eq!(rule.name, "new_name");
        assert_eq!(rule.content, "content");
    }

    #[test]
    fn test_rename_not_found() {
        let (_temp_dir, storage) = setup();
        let result = storage.rename("nonexistent", "new_name");
        assert!(result.is_err());
    }

    #[test]
    fn test_rename_target_exists() {
        let (_temp_dir, storage) = setup();
        storage
            .save(&RuleFile::new("old_name", "content1"))
            .unwrap();
        storage
            .save(&RuleFile::new("new_name", "content2"))
            .unwrap();

        let result = storage.rename("old_name", "new_name");
        assert!(result.is_err());
    }

    #[test]
    fn test_exists() {
        let (_temp_dir, storage) = setup();
        assert!(!storage.exists("test"));

        storage.save(&RuleFile::new("test", "content")).unwrap();
        assert!(storage.exists("test"));
    }

    #[test]
    fn test_set_enabled() {
        let (_temp_dir, storage) = setup();
        storage.save(&RuleFile::new("test", "content")).unwrap();

        storage.set_enabled("test", false).unwrap();
        let rule = storage.load("test").unwrap();
        assert!(!rule.enabled);

        storage.set_enabled("test", true).unwrap();
        let rule = storage.load("test").unwrap();
        assert!(rule.enabled);
    }

    #[test]
    fn test_load_enabled() {
        let (_temp_dir, storage) = setup();
        storage.save(&RuleFile::new("rule1", "content1")).unwrap();
        storage
            .save(&RuleFile::new("rule2", "content2").with_enabled(false))
            .unwrap();
        storage.save(&RuleFile::new("rule3", "content3")).unwrap();

        let enabled = storage.load_enabled().unwrap();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.iter().all(|r| r.enabled));
    }

    #[test]
    fn test_overwrite() {
        let (_temp_dir, storage) = setup();
        storage.save(&RuleFile::new("test", "content1")).unwrap();
        storage.save(&RuleFile::new("test", "content2")).unwrap();

        let loaded = storage.load("test").unwrap();
        assert_eq!(loaded.content, "content2");
    }

    #[test]
    fn test_sort_order() {
        let (_temp_dir, storage) = setup();
        storage
            .save(&RuleFile::new("rule1", "c1").with_sort_order(2))
            .unwrap();
        storage
            .save(&RuleFile::new("rule2", "c2").with_sort_order(0))
            .unwrap();
        storage
            .save(&RuleFile::new("rule3", "c3").with_sort_order(1))
            .unwrap();

        let rules = storage.load_all().unwrap();
        assert_eq!(rules[0].name, "rule2");
        assert_eq!(rules[1].name, "rule3");
        assert_eq!(rules[2].name, "rule1");
    }

    #[test]
    fn test_reorder() {
        let (_temp_dir, storage) = setup();
        storage.save(&RuleFile::new("a", "ca")).unwrap();
        storage.save(&RuleFile::new("b", "cb")).unwrap();
        storage.save(&RuleFile::new("c", "cc")).unwrap();

        storage
            .reorder(&["c".to_string(), "a".to_string(), "b".to_string()])
            .unwrap();

        let rules = storage.load_all().unwrap();
        assert_eq!(rules[0].name, "c");
        assert_eq!(rules[1].name, "a");
        assert_eq!(rules[2].name, "b");
    }

    #[test]
    fn test_list_summaries() {
        let (_temp_dir, storage) = setup();
        storage
            .save(
                &RuleFile::new("test", "rule1\nrule2\n# comment\n")
                    .with_description(Some("Test rules".to_string())),
            )
            .unwrap();

        let summaries = storage.list_summaries().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "test");
        assert_eq!(summaries[0].rule_count, 2);
        assert_eq!(summaries[0].description, Some("Test rules".to_string()));
    }
}
