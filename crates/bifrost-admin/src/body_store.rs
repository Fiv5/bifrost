use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BodyRef {
    Inline { data: String },
    File { path: String, size: usize },
}

impl BodyRef {
    pub fn size(&self) -> usize {
        match self {
            BodyRef::Inline { data } => data.len(),
            BodyRef::File { size, .. } => *size,
        }
    }

    pub fn is_file(&self) -> bool {
        matches!(self, BodyRef::File { .. })
    }
}

pub struct BodyStore {
    temp_dir: PathBuf,
    max_memory_size: usize,
    retention_days: u64,
}

impl BodyStore {
    pub fn new(temp_dir: PathBuf, max_memory_size: usize, retention_days: u64) -> Self {
        if !temp_dir.exists() {
            let _ = fs::create_dir_all(&temp_dir);
        }
        Self {
            temp_dir,
            max_memory_size,
            retention_days,
        }
    }

    pub fn store(&self, id: &str, kind: &str, data: &[u8]) -> Option<BodyRef> {
        if data.is_empty() {
            return None;
        }

        if data.len() <= self.max_memory_size {
            let text = String::from_utf8_lossy(data).to_string();
            return Some(BodyRef::Inline { data: text });
        }

        let filename = format!("{}_{}", id, kind);
        let path = self.temp_dir.join(&filename);

        match fs::File::create(&path) {
            Ok(mut file) => {
                if file.write_all(data).is_ok() {
                    Some(BodyRef::File {
                        path: path.to_string_lossy().to_string(),
                        size: data.len(),
                    })
                } else {
                    let _ = fs::remove_file(&path);
                    let text = String::from_utf8_lossy(data).to_string();
                    Some(BodyRef::Inline { data: text })
                }
            }
            Err(_) => {
                let text = String::from_utf8_lossy(data).to_string();
                Some(BodyRef::Inline { data: text })
            }
        }
    }

    pub fn load(&self, body_ref: &BodyRef) -> Option<String> {
        match body_ref {
            BodyRef::Inline { data } => Some(data.clone()),
            BodyRef::File { path, .. } => {
                let path = PathBuf::from(path);
                if !path.exists() {
                    return None;
                }
                let mut file = fs::File::open(&path).ok()?;
                let mut contents = Vec::new();
                file.read_to_end(&mut contents).ok()?;
                Some(String::from_utf8_lossy(&contents).to_string())
            }
        }
    }

    pub fn cleanup_expired(&self) -> std::io::Result<usize> {
        if !self.temp_dir.exists() {
            return Ok(0);
        }

        let retention_duration = Duration::from_secs(self.retention_days * 24 * 60 * 60);
        let now = SystemTime::now();
        let mut removed_count = 0;

        for entry in fs::read_dir(&self.temp_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(age) = now.duration_since(modified) {
                            if age > retention_duration && fs::remove_file(&path).is_ok() {
                                removed_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(removed_count)
    }

    pub fn clear(&self) -> std::io::Result<usize> {
        if !self.temp_dir.exists() {
            return Ok(0);
        }

        let mut removed_count = 0;
        for entry in fs::read_dir(&self.temp_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && fs::remove_file(&path).is_ok() {
                removed_count += 1;
            }
        }
        Ok(removed_count)
    }

    pub fn remove(&self, body_ref: &BodyRef) {
        if let BodyRef::File { path, .. } = body_ref {
            let _ = fs::remove_file(path);
        }
    }
}

pub type SharedBodyStore = Arc<RwLock<BodyStore>>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn create_test_dir() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = env::temp_dir().join(format!(
            "bifrost_test_{}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            counter
        ));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn cleanup_test_dir(dir: &PathBuf) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_store_inline_small_body() {
        let dir = create_test_dir();
        let store = BodyStore::new(dir.clone(), 1024, 7);

        let data = b"Hello, World!";
        let body_ref = store.store("test1", "req", data).unwrap();

        assert!(matches!(body_ref, BodyRef::Inline { .. }));
        assert_eq!(store.load(&body_ref).unwrap(), "Hello, World!");

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_store_file_large_body() {
        let dir = create_test_dir();
        let store = BodyStore::new(dir.clone(), 10, 7);

        let data = b"This is a large body that exceeds the memory limit";
        let body_ref = store.store("test2", "res", data).unwrap();

        assert!(matches!(body_ref, BodyRef::File { .. }));
        assert!(body_ref.is_file());
        assert_eq!(body_ref.size(), data.len());
        assert_eq!(
            store.load(&body_ref).unwrap(),
            "This is a large body that exceeds the memory limit"
        );

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_empty_body() {
        let dir = create_test_dir();
        let store = BodyStore::new(dir.clone(), 1024, 7);

        let body_ref = store.store("test3", "req", b"");
        assert!(body_ref.is_none());

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_cleanup() {
        let dir = create_test_dir();
        let store = BodyStore::new(dir.clone(), 10, 0);

        let data = b"Test data for cleanup";
        store.store("test4", "req", data);

        std::thread::sleep(std::time::Duration::from_millis(100));

        let removed = store.cleanup_expired().unwrap();
        assert!(removed >= 1);

        cleanup_test_dir(&dir);
    }
}
