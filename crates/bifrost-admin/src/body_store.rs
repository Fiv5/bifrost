use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BodyRef {
    Inline {
        data: String,
    },
    File {
        path: String,
        size: usize,
    },
    FileRange {
        path: String,
        offset: u64,
        size: usize,
    },
}

impl BodyRef {
    pub fn size(&self) -> usize {
        match self {
            BodyRef::Inline { data } => data.len(),
            BodyRef::File { size, .. } => *size,
            BodyRef::FileRange { size, .. } => *size,
        }
    }

    pub fn is_file(&self) -> bool {
        matches!(self, BodyRef::File { .. } | BodyRef::FileRange { .. })
    }
}

pub struct BodyStore {
    temp_dir: PathBuf,
    max_memory_size: usize,
    retention_days: u64,
    stream_flush_bytes: usize,
    stream_flush_interval: Duration,
}

pub struct BodyStreamWriter {
    path: PathBuf,
    file: fs::File,
    size: usize,
    buffer: Vec<u8>,
    flush_bytes: usize,
    flush_interval: Duration,
    last_flush: Instant,
}

impl BodyStreamWriter {
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn body_ref(&self) -> BodyRef {
        BodyRef::File {
            path: self.path.to_string_lossy().to_string(),
            size: self.size,
        }
    }

    pub fn write_chunk(&mut self, data: &[u8]) -> std::io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        self.buffer.extend_from_slice(data);
        self.size += data.len();
        if self.buffer.len() >= self.flush_bytes || self.last_flush.elapsed() >= self.flush_interval
        {
            self.flush()?;
        }
        Ok(())
    }

    pub fn flush_interval(&self) -> Duration {
        self.flush_interval
    }

    pub fn flush_buffered(&mut self) -> std::io::Result<()> {
        self.flush()
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        self.file.write_all(&self.buffer)?;
        self.file.flush()?;
        self.buffer.clear();
        self.last_flush = Instant::now();
        Ok(())
    }

    pub fn finish(mut self) -> BodyRef {
        let _ = self.flush();
        BodyRef::File {
            path: self.path.to_string_lossy().to_string(),
            size: self.size,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BodyStoreConfigUpdate {
    pub max_memory_size: Option<usize>,
    pub retention_days: Option<u64>,
    pub stream_flush_bytes: Option<usize>,
    pub stream_flush_interval_ms: Option<u64>,
}

impl BodyStore {
    pub fn new(
        temp_dir: PathBuf,
        max_memory_size: usize,
        retention_days: u64,
        stream_flush_bytes: usize,
        stream_flush_interval: Duration,
    ) -> Self {
        if !temp_dir.exists() {
            let _ = fs::create_dir_all(&temp_dir);
        }
        Self {
            temp_dir,
            max_memory_size,
            retention_days,
            stream_flush_bytes,
            stream_flush_interval,
        }
    }

    pub fn update_config(&mut self, update: BodyStoreConfigUpdate) {
        if let Some(max_memory_size) = update.max_memory_size {
            tracing::info!(
                "BodyStore config updated: max_memory_size {} -> {}",
                self.max_memory_size,
                max_memory_size
            );
            self.max_memory_size = max_memory_size;
        }
        if let Some(retention_days) = update.retention_days {
            tracing::info!(
                "BodyStore config updated: retention_days {} -> {}",
                self.retention_days,
                retention_days
            );
            self.retention_days = retention_days;
        }
        if let Some(stream_flush_bytes) = update.stream_flush_bytes {
            tracing::info!(
                "BodyStore config updated: stream_flush_bytes {} -> {}",
                self.stream_flush_bytes,
                stream_flush_bytes
            );
            self.stream_flush_bytes = stream_flush_bytes;
        }
        if let Some(stream_flush_interval_ms) = update.stream_flush_interval_ms {
            tracing::info!(
                "BodyStore config updated: stream_flush_interval_ms {:?} -> {}",
                self.stream_flush_interval.as_millis(),
                stream_flush_interval_ms
            );
            self.stream_flush_interval = Duration::from_millis(stream_flush_interval_ms);
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

    pub fn store_force_file(&self, id: &str, kind: &str, data: &[u8]) -> Option<BodyRef> {
        if data.is_empty() {
            return None;
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

    pub fn start_stream(&self, id: &str, kind: &str) -> std::io::Result<BodyStreamWriter> {
        let filename = format!("{}_{}", id, kind);
        let path = self.temp_dir.join(&filename);
        let file = fs::File::create(&path)?;
        Ok(BodyStreamWriter {
            path,
            file,
            size: 0,
            buffer: Vec::with_capacity(self.stream_flush_bytes),
            flush_bytes: self.stream_flush_bytes,
            flush_interval: self.stream_flush_interval,
            last_flush: Instant::now(),
        })
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
            BodyRef::FileRange { path, offset, size } => {
                let path = PathBuf::from(path);
                if !path.exists() {
                    return None;
                }
                let mut file = fs::File::open(&path).ok()?;
                file.seek(SeekFrom::Start(*offset)).ok()?;
                let mut contents = vec![0u8; *size];
                let mut read_size = 0usize;
                while read_size < *size {
                    let n = file.read(&mut contents[read_size..]).ok()?;
                    if n == 0 {
                        break;
                    }
                    read_size += n;
                }
                contents.truncate(read_size);
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

    pub fn delete_by_ids(&self, ids: &[String]) -> std::io::Result<usize> {
        if ids.is_empty() || !self.temp_dir.exists() {
            return Ok(0);
        }

        let ids_set: std::collections::HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
        let mut removed_count = 0;

        for entry in fs::read_dir(&self.temp_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_stem().and_then(|s| s.to_str()) {
                    let base_id = file_name
                        .rsplit_once('_')
                        .map(|(id, _)| id)
                        .unwrap_or(file_name);
                    if ids_set.contains(base_id) && fs::remove_file(&path).is_ok() {
                        removed_count += 1;
                    }
                }
            }
        }

        tracing::debug!(count = removed_count, "[BODY_STORE] Deleted bodies by ids");
        Ok(removed_count)
    }

    pub fn remove(&self, body_ref: &BodyRef) {
        match body_ref {
            BodyRef::File { path, .. } | BodyRef::FileRange { path, .. } => {
                let _ = fs::remove_file(path);
            }
            BodyRef::Inline { .. } => {}
        }
    }

    pub fn stats(&self) -> BodyStoreStats {
        let mut file_count = 0;
        let mut total_size = 0u64;

        if self.temp_dir.exists() {
            if let Ok(entries) = fs::read_dir(&self.temp_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        file_count += 1;
                        if let Ok(metadata) = entry.metadata() {
                            total_size += metadata.len();
                        }
                    }
                }
            }
        }

        BodyStoreStats {
            file_count,
            total_size,
            temp_dir: self.temp_dir.to_string_lossy().to_string(),
            max_memory_size: self.max_memory_size,
            retention_days: self.retention_days,
        }
    }

    pub fn sizes_by_id(&self) -> std::io::Result<std::collections::HashMap<String, u64>> {
        let mut sizes = std::collections::HashMap::new();
        if !self.temp_dir.exists() {
            return Ok(sizes);
        }
        for entry in fs::read_dir(&self.temp_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if let Some(file_name) = path.file_stem().and_then(|s| s.to_str()) {
                    let base_id = file_name
                        .rsplit_once('_')
                        .map(|(id, _)| id)
                        .unwrap_or(file_name);
                    *sizes.entry(base_id.to_string()).or_insert(0) += size;
                }
            }
        }
        Ok(sizes)
    }
}

pub type SharedBodyStore = Arc<RwLock<BodyStore>>;

pub fn start_body_cleanup_task(store: SharedBodyStore) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Ok(removed) = store.read().cleanup_expired() {
                if removed > 0 {
                    tracing::info!(
                        "[BODY_STORE] Periodic cleanup removed {} expired files",
                        removed
                    );
                }
            }
        }
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyStoreStats {
    pub file_count: usize,
    pub total_size: u64,
    pub temp_dir: String,
    pub max_memory_size: usize,
    pub retention_days: u64,
}

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
        let store = BodyStore::new(dir.clone(), 1024, 7, 64 * 1024, Duration::from_millis(200));

        let data = b"Hello, World!";
        let body_ref = store.store("test1", "req", data).unwrap();

        assert!(matches!(body_ref, BodyRef::Inline { .. }));
        assert_eq!(store.load(&body_ref).unwrap(), "Hello, World!");

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_store_file_large_body() {
        let dir = create_test_dir();
        let store = BodyStore::new(dir.clone(), 10, 7, 64 * 1024, Duration::from_millis(200));

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
    fn test_load_file_range() {
        let dir = create_test_dir();
        let store = BodyStore::new(dir.clone(), 10, 7, 64 * 1024, Duration::from_millis(200));

        let data = b"Hello range body";
        let body_ref = store.store("test_range", "res", data).unwrap();
        let path = match body_ref {
            BodyRef::File { path, .. } => path,
            _ => {
                cleanup_test_dir(&dir);
                return;
            }
        };
        let range_ref = BodyRef::FileRange {
            path,
            offset: 6,
            size: 5,
        };
        assert_eq!(store.load(&range_ref).unwrap(), "range");

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_empty_body() {
        let dir = create_test_dir();
        let store = BodyStore::new(dir.clone(), 1024, 7, 64 * 1024, Duration::from_millis(200));

        let body_ref = store.store("test3", "req", b"");
        assert!(body_ref.is_none());

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_delete_by_ids_with_hyphenated_id() {
        let dir = create_test_dir();
        let store = BodyStore::new(dir.clone(), 1, 7, 64 * 1024, Duration::from_millis(200));

        let id = "req-123-abc";
        let data = b"large body for file storage";
        let body_ref = store.store(id, "req", data).unwrap();
        assert!(body_ref.is_file());

        let before_stats = store.stats();
        assert_eq!(before_stats.file_count, 1);

        let removed = store.delete_by_ids(&[id.to_string()]).unwrap();
        assert_eq!(removed, 1);

        let after_stats = store.stats();
        assert_eq!(after_stats.file_count, 0);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_cleanup() {
        let dir = create_test_dir();
        let store = BodyStore::new(dir.clone(), 10, 0, 64 * 1024, Duration::from_millis(200));

        let data = b"Test data for cleanup";
        store.store("test4", "req", data);

        std::thread::sleep(std::time::Duration::from_millis(100));

        let removed = store.cleanup_expired().unwrap();
        assert!(removed >= 1);

        cleanup_test_dir(&dir);
    }
}
