use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use bifrost_core::{BifrostError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFile {
    pub name: String,
    pub content: String,
    pub enabled: bool,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl RuleFile {
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            enabled: true,
            metadata: HashMap::new(),
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

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
        self.base_dir.join(format!("{}.json", name))
    }

    pub fn load(&self, name: &str) -> Result<RuleFile> {
        let path = self.rule_path(name);
        if !path.exists() {
            return Err(BifrostError::NotFound(format!("Rule '{}' not found", name)));
        }
        let content = fs::read_to_string(&path)?;
        serde_json::from_str(&content)
            .map_err(|e| BifrostError::Parse(format!("Failed to parse rule file: {}", e)))
    }

    pub fn save(&self, rule: &RuleFile) -> Result<()> {
        let path = self.rule_path(&rule.name);
        let content = serde_json::to_string_pretty(rule)
            .map_err(|e| BifrostError::Config(format!("Failed to serialize rule: {}", e)))?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.rule_path(name);
        if !path.exists() {
            return Err(BifrostError::NotFound(format!("Rule '{}' not found", name)));
        }
        fs::remove_file(&path)?;
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
        self.save(&rule)?;
        self.delete(old)?;
        Ok(())
    }

    pub fn exists(&self, name: &str) -> bool {
        self.rule_path(name).exists()
    }

    pub fn load_all(&self) -> Result<Vec<RuleFile>> {
        let names = self.list()?;
        let mut rules = Vec::new();
        for name in names {
            rules.push(self.load(&name)?);
        }
        Ok(rules)
    }

    pub fn load_enabled(&self) -> Result<Vec<RuleFile>> {
        let rules = self.load_all()?;
        Ok(rules.into_iter().filter(|r| r.enabled).collect())
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let mut rule = self.load(name)?;
        rule.enabled = enabled;
        self.save(&rule)
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
}
