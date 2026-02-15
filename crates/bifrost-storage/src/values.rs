use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use bifrost_core::{BifrostError, Result, ValueStore};

#[derive(Clone)]
pub struct ValuesStorage {
    base_dir: PathBuf,
    cache: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ValueEntry {
    pub name: String,
    pub value: String,
}

impl ValuesStorage {
    pub fn new() -> Result<Self> {
        let base_dir = crate::data_dir().join("values");
        Self::with_dir(base_dir)
    }

    pub fn with_dir(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        let mut storage = Self {
            base_dir: dir,
            cache: HashMap::new(),
        };
        storage.load_cache()?;
        Ok(storage)
    }

    fn load_cache(&mut self) -> Result<()> {
        self.cache.clear();
        if !self.base_dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("txt") {
                if let Some(key) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(value) = fs::read_to_string(&path) {
                        self.cache.insert(key.to_string(), value);
                    }
                }
            }
        }
        Ok(())
    }

    fn value_path(&self, key: &str) -> PathBuf {
        let safe_key = key.replace(['/', '\\', ':'], "_");
        self.base_dir.join(format!("{}.txt", safe_key))
    }

    pub fn get_value(&self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        let path = self.value_path(key);
        fs::write(&path, value)?;
        self.cache.insert(key.to_string(), value.to_string());
        Ok(())
    }

    pub fn remove_value(&mut self, key: &str) -> Result<()> {
        let path = self.value_path(key);
        if !path.exists() {
            return Err(BifrostError::NotFound(format!("Value '{}' not found", key)));
        }
        fs::remove_file(&path)?;
        self.cache.remove(key);
        Ok(())
    }

    pub fn list_keys(&self) -> Result<Vec<String>> {
        let mut keys: Vec<String> = self.cache.keys().cloned().collect();
        keys.sort();
        Ok(keys)
    }

    pub fn clear(&mut self) -> Result<()> {
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("txt") {
                fs::remove_file(&path)?;
            }
        }
        self.cache.clear();
        Ok(())
    }

    pub fn exists(&self, key: &str) -> bool {
        self.cache.contains_key(key)
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.load_cache()
    }

    pub fn list_entries(&self) -> Result<Vec<ValueEntry>> {
        let mut entries: Vec<ValueEntry> = self
            .cache
            .iter()
            .map(|(k, v)| ValueEntry {
                name: k.clone(),
                value: v.clone(),
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    pub fn create(&mut self, name: &str, value: &str) -> Result<()> {
        if self.exists(name) {
            return Err(BifrostError::AlreadyExists(format!(
                "Value '{}' already exists",
                name
            )));
        }
        self.set_value(name, value)
    }

    pub fn update(&mut self, name: &str, value: &str) -> Result<()> {
        if !self.exists(name) {
            return Err(BifrostError::NotFound(format!(
                "Value '{}' not found",
                name
            )));
        }
        self.set_value(name, value)
    }

    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }

    pub fn load_from_file(&mut self, path: &std::path::Path) -> Result<usize> {
        let content = fs::read_to_string(path)?;
        let mut count = 0;

        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        match extension {
            "json" => {
                let map: HashMap<String, String> = serde_json::from_str(&content)
                    .map_err(|e| BifrostError::Parse(format!("Invalid JSON: {}", e)))?;
                for (k, v) in map {
                    self.set_value(&k, &v)?;
                    count += 1;
                }
            }
            "kv" | "env" => {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some(eq_pos) = line.find('=') {
                        let key = line[..eq_pos].trim();
                        let value = line[eq_pos + 1..].trim();
                        if !key.is_empty() {
                            self.set_value(key, value)?;
                            count += 1;
                        }
                    }
                }
            }
            _ => {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    self.set_value(name, content.trim())?;
                    count = 1;
                }
            }
        }

        Ok(count)
    }
}

impl ValueStore for ValuesStorage {
    fn get(&self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    fn set(&mut self, key: &str, value: String) {
        let path = self.value_path(key);
        let _ = fs::write(&path, &value);
        self.cache.insert(key.to_string(), value);
    }

    fn remove(&mut self, key: &str) -> Option<String> {
        let path = self.value_path(key);
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        self.cache.remove(key)
    }

    fn list(&self) -> Vec<(String, String)> {
        self.cache
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    fn contains(&self, key: &str) -> bool {
        self.cache.contains_key(key)
    }

    fn len(&self) -> usize {
        self.cache.len()
    }

    fn as_hashmap(&self) -> HashMap<String, String> {
        self.cache.clone()
    }
}

impl Default for ValuesStorage {
    fn default() -> Self {
        Self::new().expect("Failed to create default ValuesStorage")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, ValuesStorage) {
        let temp_dir = TempDir::new().unwrap();
        let storage = ValuesStorage::with_dir(temp_dir.path().to_path_buf()).unwrap();
        (temp_dir, storage)
    }

    #[test]
    fn test_set_and_get() {
        let (_temp_dir, mut storage) = setup();
        storage.set_value("test_key", "test_value").unwrap();

        let value = storage.get_value("test_key");
        assert_eq!(value, Some("test_value".to_string()));
    }

    #[test]
    fn test_get_not_found() {
        let (_temp_dir, storage) = setup();
        let value = storage.get_value("nonexistent");
        assert_eq!(value, None);
    }

    #[test]
    fn test_list() {
        let (_temp_dir, mut storage) = setup();
        storage.set_value("key1", "value1").unwrap();
        storage.set_value("key2", "value2").unwrap();
        storage.set_value("key3", "value3").unwrap();

        let keys = storage.list_keys().unwrap();
        assert_eq!(keys, vec!["key1", "key2", "key3"]);
    }

    #[test]
    fn test_list_empty() {
        let (_temp_dir, storage) = setup();
        let keys = storage.list_keys().unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_remove() {
        let (_temp_dir, mut storage) = setup();
        storage.set_value("test_key", "test_value").unwrap();
        assert!(storage.exists("test_key"));

        storage.remove_value("test_key").unwrap();
        assert!(!storage.exists("test_key"));
        assert_eq!(storage.get_value("test_key"), None);
    }

    #[test]
    fn test_remove_not_found() {
        let (_temp_dir, mut storage) = setup();
        let result = storage.remove_value("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_clear() {
        let (_temp_dir, mut storage) = setup();
        storage.set_value("key1", "value1").unwrap();
        storage.set_value("key2", "value2").unwrap();

        storage.clear().unwrap();

        let keys = storage.list_keys().unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_exists() {
        let (_temp_dir, mut storage) = setup();
        assert!(!storage.exists("test_key"));

        storage.set_value("test_key", "test_value").unwrap();
        assert!(storage.exists("test_key"));
    }

    #[test]
    fn test_overwrite() {
        let (_temp_dir, mut storage) = setup();
        storage.set_value("test_key", "value1").unwrap();
        storage.set_value("test_key", "value2").unwrap();

        let value = storage.get_value("test_key");
        assert_eq!(value, Some("value2".to_string()));
    }

    #[test]
    fn test_cache_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().to_path_buf();

        {
            let mut storage = ValuesStorage::with_dir(dir.clone()).unwrap();
            storage.set_value("key1", "value1").unwrap();
            storage.set_value("key2", "value2").unwrap();
        }

        {
            let storage = ValuesStorage::with_dir(dir).unwrap();
            assert_eq!(storage.get_value("key1"), Some("value1".to_string()));
            assert_eq!(storage.get_value("key2"), Some("value2".to_string()));
        }
    }

    #[test]
    fn test_refresh() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().to_path_buf();
        let mut storage = ValuesStorage::with_dir(dir.clone()).unwrap();
        storage.set_value("key1", "value1").unwrap();

        fs::write(dir.join("key2.txt"), "value2").unwrap();

        storage.refresh().unwrap();
        assert_eq!(storage.get_value("key2"), Some("value2".to_string()));
    }

    #[test]
    fn test_special_characters_in_key() {
        let (_temp_dir, mut storage) = setup();
        storage.set_value("path/to/key", "value").unwrap();

        assert!(storage.exists("path/to/key"));
        assert_eq!(storage.get_value("path/to/key"), Some("value".to_string()));
    }
}
