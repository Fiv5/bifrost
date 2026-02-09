use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use bifrost_core::{Result, BifrostError};

pub struct ValuesStorage {
    base_dir: PathBuf,
    cache: HashMap<String, String>,
}

impl ValuesStorage {
    pub fn new() -> Result<Self> {
        let base_dir = dirs::home_dir()
            .ok_or_else(|| BifrostError::Config("Cannot find home directory".to_string()))?
            .join(".bifrost")
            .join("values");
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

    pub fn get(&self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let path = self.value_path(key);
        fs::write(&path, value)?;
        self.cache.insert(key.to_string(), value.to_string());
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Result<()> {
        let path = self.value_path(key);
        if !path.exists() {
            return Err(BifrostError::NotFound(format!("Value '{}' not found", key)));
        }
        fs::remove_file(&path)?;
        self.cache.remove(key);
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<String>> {
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
        storage.set("test_key", "test_value").unwrap();

        let value = storage.get("test_key");
        assert_eq!(value, Some("test_value".to_string()));
    }

    #[test]
    fn test_get_not_found() {
        let (_temp_dir, storage) = setup();
        let value = storage.get("nonexistent");
        assert_eq!(value, None);
    }

    #[test]
    fn test_list() {
        let (_temp_dir, mut storage) = setup();
        storage.set("key1", "value1").unwrap();
        storage.set("key2", "value2").unwrap();
        storage.set("key3", "value3").unwrap();

        let keys = storage.list().unwrap();
        assert_eq!(keys, vec!["key1", "key2", "key3"]);
    }

    #[test]
    fn test_list_empty() {
        let (_temp_dir, storage) = setup();
        let keys = storage.list().unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_remove() {
        let (_temp_dir, mut storage) = setup();
        storage.set("test_key", "test_value").unwrap();
        assert!(storage.exists("test_key"));

        storage.remove("test_key").unwrap();
        assert!(!storage.exists("test_key"));
        assert_eq!(storage.get("test_key"), None);
    }

    #[test]
    fn test_remove_not_found() {
        let (_temp_dir, mut storage) = setup();
        let result = storage.remove("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_clear() {
        let (_temp_dir, mut storage) = setup();
        storage.set("key1", "value1").unwrap();
        storage.set("key2", "value2").unwrap();

        storage.clear().unwrap();

        let keys = storage.list().unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_exists() {
        let (_temp_dir, mut storage) = setup();
        assert!(!storage.exists("test_key"));

        storage.set("test_key", "test_value").unwrap();
        assert!(storage.exists("test_key"));
    }

    #[test]
    fn test_overwrite() {
        let (_temp_dir, mut storage) = setup();
        storage.set("test_key", "value1").unwrap();
        storage.set("test_key", "value2").unwrap();

        let value = storage.get("test_key");
        assert_eq!(value, Some("value2".to_string()));
    }

    #[test]
    fn test_cache_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().to_path_buf();

        {
            let mut storage = ValuesStorage::with_dir(dir.clone()).unwrap();
            storage.set("key1", "value1").unwrap();
            storage.set("key2", "value2").unwrap();
        }

        {
            let storage = ValuesStorage::with_dir(dir).unwrap();
            assert_eq!(storage.get("key1"), Some("value1".to_string()));
            assert_eq!(storage.get("key2"), Some("value2".to_string()));
        }
    }

    #[test]
    fn test_refresh() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().to_path_buf();
        let mut storage = ValuesStorage::with_dir(dir.clone()).unwrap();
        storage.set("key1", "value1").unwrap();

        fs::write(dir.join("key2.txt"), "value2").unwrap();

        storage.refresh().unwrap();
        assert_eq!(storage.get("key2"), Some("value2".to_string()));
    }

    #[test]
    fn test_special_characters_in_key() {
        let (_temp_dir, mut storage) = setup();
        storage.set("path/to/key", "value").unwrap();

        assert!(storage.exists("path/to/key"));
        assert_eq!(storage.get("path/to/key"), Some("value".to_string()));
    }
}
