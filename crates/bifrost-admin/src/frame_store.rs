use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::connection_monitor::WebSocketFrameRecord;

const DEFAULT_RETENTION_HOURS: u64 = 24;
const FRAMES_SUBDIR: &str = "frames";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameStoreMetadata {
    pub connection_id: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub frame_count: u64,
    pub last_frame_id: u64,
    pub is_closed: bool,
}

impl FrameStoreMetadata {
    pub fn new(connection_id: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            connection_id: connection_id.to_string(),
            created_at: now,
            updated_at: now,
            frame_count: 0,
            last_frame_id: 0,
            is_closed: false,
        }
    }
}

pub struct FrameStore {
    base_dir: PathBuf,
    retention_hours: u64,
    metadata_cache: RwLock<HashMap<String, FrameStoreMetadata>>,
}

impl FrameStore {
    pub fn new(base_dir: PathBuf, retention_hours: Option<u64>) -> Self {
        let frames_dir = base_dir.join(FRAMES_SUBDIR);
        if !frames_dir.exists() {
            let _ = fs::create_dir_all(&frames_dir);
        }

        let store = Self {
            base_dir,
            retention_hours: retention_hours.unwrap_or(DEFAULT_RETENTION_HOURS),
            metadata_cache: RwLock::new(HashMap::new()),
        };

        store.load_metadata_cache();
        store
    }

    fn frames_dir(&self) -> PathBuf {
        self.base_dir.join(FRAMES_SUBDIR)
    }

    fn connection_file_path(&self, connection_id: &str) -> PathBuf {
        let safe_id = connection_id.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.frames_dir().join(format!("{}.jsonl", safe_id))
    }

    fn metadata_file_path(&self, connection_id: &str) -> PathBuf {
        let safe_id = connection_id.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.frames_dir().join(format!("{}.meta.json", safe_id))
    }

    fn load_metadata_cache(&self) {
        let frames_dir = self.frames_dir();
        if !frames_dir.exists() {
            return;
        }

        let mut cache = self.metadata_cache.write();
        if let Ok(entries) = fs::read_dir(&frames_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json")
                    && path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().ends_with(".meta.json"))
                {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(metadata) = serde_json::from_str::<FrameStoreMetadata>(&content) {
                            cache.insert(metadata.connection_id.clone(), metadata);
                        }
                    }
                }
            }
        }
        tracing::debug!(
            "[FRAME_STORE] Loaded {} metadata entries from cache",
            cache.len()
        );
    }

    fn save_metadata(&self, metadata: &FrameStoreMetadata) {
        let path = self.metadata_file_path(&metadata.connection_id);
        if let Ok(content) = serde_json::to_string_pretty(metadata) {
            let _ = fs::write(&path, content);
        }
        self.metadata_cache
            .write()
            .insert(metadata.connection_id.clone(), metadata.clone());
    }

    pub fn append_frame(
        &self,
        connection_id: &str,
        frame: &WebSocketFrameRecord,
    ) -> std::io::Result<()> {
        let path = self.connection_file_path(connection_id);

        let file = OpenOptions::new().create(true).append(true).open(&path)?;

        let mut writer = BufWriter::new(file);
        let json = serde_json::to_string(frame)?;
        writeln!(writer, "{}", json)?;
        writer.flush()?;

        let metadata_to_save = {
            let mut cache = self.metadata_cache.write();
            let metadata = cache.entry(connection_id.to_string()).or_insert_with(|| {
                let meta_path = self.metadata_file_path(connection_id);
                if let Ok(content) = fs::read_to_string(&meta_path) {
                    serde_json::from_str(&content)
                        .unwrap_or_else(|_| FrameStoreMetadata::new(connection_id))
                } else {
                    FrameStoreMetadata::new(connection_id)
                }
            });

            metadata.frame_count += 1;
            metadata.last_frame_id = frame.frame_id;
            metadata.updated_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            metadata.clone()
        };

        self.save_metadata(&metadata_to_save);

        Ok(())
    }

    pub fn load_frames(
        &self,
        connection_id: &str,
        after_frame_id: Option<u64>,
        limit: usize,
    ) -> std::io::Result<(Vec<WebSocketFrameRecord>, bool)> {
        let path = self.connection_file_path(connection_id);

        if !path.exists() {
            return Ok((Vec::new(), false));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let mut frames = Vec::new();
        let mut total_matching = 0;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<WebSocketFrameRecord>(&line) {
                Ok(frame) => {
                    let should_include = match after_frame_id {
                        Some(after_id) => frame.frame_id > after_id,
                        None => true,
                    };

                    if should_include {
                        total_matching += 1;
                        if frames.len() < limit {
                            frames.push(frame);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "[FRAME_STORE] Failed to parse frame line for {}: {}",
                        connection_id,
                        e
                    );
                }
            }
        }

        let has_more = total_matching > limit;
        Ok((frames, has_more))
    }

    pub fn load_all_frames(
        &self,
        connection_id: &str,
    ) -> std::io::Result<Vec<WebSocketFrameRecord>> {
        let (frames, _) = self.load_frames(connection_id, None, usize::MAX)?;
        Ok(frames)
    }

    pub fn get_metadata(&self, connection_id: &str) -> Option<FrameStoreMetadata> {
        self.metadata_cache.read().get(connection_id).cloned()
    }

    pub fn mark_connection_closed(&self, connection_id: &str) {
        let mut cache = self.metadata_cache.write();
        if let Some(metadata) = cache.get_mut(connection_id) {
            metadata.is_closed = true;
            metadata.updated_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let m = metadata.clone();
            drop(cache);
            self.save_metadata(&m);
        }
    }

    pub fn connection_exists(&self, connection_id: &str) -> bool {
        self.connection_file_path(connection_id).exists()
    }

    pub fn get_last_frame_id(&self, connection_id: &str) -> Option<u64> {
        self.metadata_cache
            .read()
            .get(connection_id)
            .map(|m| m.last_frame_id)
    }

    pub fn cleanup_expired(&self) -> std::io::Result<usize> {
        let frames_dir = self.frames_dir();
        if !frames_dir.exists() {
            return Ok(0);
        }

        let retention_duration = Duration::from_secs(self.retention_hours * 60 * 60);
        let now = SystemTime::now();
        let mut removed_count = 0;

        let mut to_remove = Vec::new();
        {
            let cache = self.metadata_cache.read();
            for (connection_id, metadata) in cache.iter() {
                let updated_at = UNIX_EPOCH + Duration::from_millis(metadata.updated_at);
                if let Ok(age) = now.duration_since(updated_at) {
                    if age > retention_duration && metadata.is_closed {
                        to_remove.push(connection_id.clone());
                    }
                }
            }
        }

        for connection_id in to_remove {
            if self.remove_connection(&connection_id).is_ok() {
                removed_count += 1;
            }
        }

        tracing::info!(
            "[FRAME_STORE] Cleaned up {} expired frame files",
            removed_count
        );
        Ok(removed_count)
    }

    pub fn remove_connection(&self, connection_id: &str) -> std::io::Result<()> {
        let frame_path = self.connection_file_path(connection_id);
        let meta_path = self.metadata_file_path(connection_id);

        if frame_path.exists() {
            fs::remove_file(&frame_path)?;
        }
        if meta_path.exists() {
            fs::remove_file(&meta_path)?;
        }

        self.metadata_cache.write().remove(connection_id);
        Ok(())
    }

    pub fn list_connections(&self) -> Vec<String> {
        self.metadata_cache.read().keys().cloned().collect()
    }

    pub fn clear(&self) -> std::io::Result<usize> {
        let frames_dir = self.frames_dir();
        let mut removed_count = 0;

        if frames_dir.exists() {
            if let Ok(entries) = fs::read_dir(&frames_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && fs::remove_file(&path).is_ok() {
                        removed_count += 1;
                    }
                }
            }
        }

        self.metadata_cache.write().clear();

        tracing::info!("[FRAME_STORE] Cleared {} frame files", removed_count);

        Ok(removed_count)
    }

    pub fn stats(&self) -> FrameStoreStats {
        let frames_dir = self.frames_dir();
        let mut file_count = 0;
        let mut total_size = 0u64;

        if frames_dir.exists() {
            if let Ok(entries) = fs::read_dir(&frames_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|e| e == "jsonl") {
                        file_count += 1;
                        if let Ok(metadata) = entry.metadata() {
                            total_size += metadata.len();
                        }
                    }
                }
            }
        }

        FrameStoreStats {
            connection_count: file_count,
            total_size,
            frames_dir: frames_dir.to_string_lossy().to_string(),
            retention_hours: self.retention_hours,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameStoreStats {
    pub connection_count: usize,
    pub total_size: u64,
    pub frames_dir: String,
    pub retention_hours: u64,
}

pub type SharedFrameStore = Arc<FrameStore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic::{FrameDirection, FrameType};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn create_test_dir() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "bifrost_frame_store_test_{}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
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

    fn create_test_frame(frame_id: u64, payload: &str) -> WebSocketFrameRecord {
        WebSocketFrameRecord {
            frame_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            direction: FrameDirection::Send,
            frame_type: FrameType::Text,
            payload_size: payload.len(),
            payload_preview: Some(payload.to_string()),
            payload_ref: None,
            is_masked: false,
            is_fin: true,
        }
    }

    #[test]
    fn test_append_and_load_frames() {
        let dir = create_test_dir();
        let store = FrameStore::new(dir.clone(), Some(24));

        let frame1 = create_test_frame(0, "Hello");
        let frame2 = create_test_frame(1, "World");

        store.append_frame("conn-1", &frame1).unwrap();
        store.append_frame("conn-1", &frame2).unwrap();

        let (frames, has_more) = store.load_frames("conn-1", None, 10).unwrap();
        assert_eq!(frames.len(), 2);
        assert!(!has_more);
        assert_eq!(frames[0].frame_id, 0);
        assert_eq!(frames[1].frame_id, 1);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_load_frames_with_after() {
        let dir = create_test_dir();
        let store = FrameStore::new(dir.clone(), Some(24));

        for i in 0..5 {
            let frame = create_test_frame(i, &format!("Message {}", i));
            store.append_frame("conn-1", &frame).unwrap();
        }

        let (frames, _) = store.load_frames("conn-1", Some(2), 10).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame_id, 3);
        assert_eq!(frames[1].frame_id, 4);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_metadata() {
        let dir = create_test_dir();
        let store = FrameStore::new(dir.clone(), Some(24));

        let frame = create_test_frame(0, "Test");
        store.append_frame("conn-1", &frame).unwrap();

        let metadata = store.get_metadata("conn-1").unwrap();
        assert_eq!(metadata.connection_id, "conn-1");
        assert_eq!(metadata.frame_count, 1);
        assert_eq!(metadata.last_frame_id, 0);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_connection_closed() {
        let dir = create_test_dir();
        let store = FrameStore::new(dir.clone(), Some(24));

        let frame = create_test_frame(0, "Test");
        store.append_frame("conn-1", &frame).unwrap();
        store.mark_connection_closed("conn-1");

        let metadata = store.get_metadata("conn-1").unwrap();
        assert!(metadata.is_closed);

        cleanup_test_dir(&dir);
    }
}
