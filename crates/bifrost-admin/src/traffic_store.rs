use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::traffic::{TrafficFilter, TrafficRecord, TrafficSummary};

const TRAFFIC_RECORDS_FILE: &str = "records.jsonl";
const TRAFFIC_METADATA_FILE: &str = "metadata.json";
const DEFAULT_RETENTION_HOURS: u64 = 24 * 7;
const CLEANUP_INTERVAL_HOURS: u64 = 1;

const FLUSH_INTERVAL_SECS: u64 = 5;
const FLUSH_BATCH_SIZE: usize = 50;
const METADATA_SAVE_INTERVAL_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficStoreMetadata {
    pub total_records: u64,
    pub last_sequence: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub oldest_record_timestamp: Option<u64>,
    pub newest_record_timestamp: Option<u64>,
}

impl Default for TrafficStoreMetadata {
    fn default() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            total_records: 0,
            last_sequence: 0,
            created_at: now,
            updated_at: now,
            oldest_record_timestamp: None,
            newest_record_timestamp: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrafficStoreConfigUpdate {
    pub max_records: Option<usize>,
    pub retention_hours: Option<u64>,
}

struct PendingWrites {
    records: Vec<TrafficRecord>,
    last_flush: Instant,
}

impl Default for PendingWrites {
    fn default() -> Self {
        Self {
            records: Vec::with_capacity(FLUSH_BATCH_SIZE),
            last_flush: Instant::now(),
        }
    }
}

pub struct TrafficStore {
    traffic_dir: PathBuf,
    records: RwLock<VecDeque<TrafficRecord>>,
    max_records: AtomicUsize,
    retention_hours: AtomicU64,
    tx: broadcast::Sender<TrafficRecord>,
    sequence: AtomicU64,
    metadata: RwLock<TrafficStoreMetadata>,
    last_cleanup: RwLock<Instant>,
    pending_writes: Mutex<PendingWrites>,
    last_metadata_save: Mutex<Instant>,
    metadata_dirty: AtomicBool,
    file_needs_rewrite: AtomicBool,
}

impl TrafficStore {
    pub fn new(traffic_dir: PathBuf, max_records: usize, retention_hours: Option<u64>) -> Self {
        if !traffic_dir.exists() {
            let _ = fs::create_dir_all(&traffic_dir);
        }

        let retention = retention_hours.unwrap_or(DEFAULT_RETENTION_HOURS);
        let (tx, _) = broadcast::channel(1000);

        let store = Self {
            traffic_dir,
            records: RwLock::new(VecDeque::with_capacity(max_records)),
            max_records: AtomicUsize::new(max_records),
            retention_hours: AtomicU64::new(retention),
            tx,
            sequence: AtomicU64::new(1),
            metadata: RwLock::new(TrafficStoreMetadata::default()),
            last_cleanup: RwLock::new(Instant::now()),
            pending_writes: Mutex::new(PendingWrites::default()),
            last_metadata_save: Mutex::new(Instant::now()),
            metadata_dirty: AtomicBool::new(false),
            file_needs_rewrite: AtomicBool::new(false),
        };

        store.load_from_disk();

        store
    }

    fn records_file_path(&self) -> PathBuf {
        self.traffic_dir.join(TRAFFIC_RECORDS_FILE)
    }

    fn metadata_file_path(&self) -> PathBuf {
        self.traffic_dir.join(TRAFFIC_METADATA_FILE)
    }

    fn load_from_disk(&self) {
        self.cleanup_temp_files();

        let records_path = self.records_file_path();
        if !records_path.exists() {
            tracing::debug!(
                "[TRAFFIC_STORE] No existing records file found at {}",
                records_path.display()
            );
            return;
        }

        let file = match File::open(&records_path) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("[TRAFFIC_STORE] Failed to open records file: {}", e);
                return;
            }
        };

        let reader = BufReader::new(file);
        let max = self.max_records.load(Ordering::Relaxed);
        let retention_hours = self.retention_hours.load(Ordering::Relaxed);
        let retention_duration = Duration::from_secs(retention_hours * 60 * 60);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let cutoff_timestamp = now.saturating_sub(retention_duration.as_millis() as u64);

        let mut loaded_records = VecDeque::new();
        let mut max_sequence: u64 = 0;
        let mut skipped_count: usize = 0;
        let mut total_lines: usize = 0;

        for line in reader.lines() {
            total_lines += 1;
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!("[TRAFFIC_STORE] Failed to read line {}: {}", total_lines, e);
                    skipped_count += 1;
                    continue;
                }
            };

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
                tracing::warn!(
                    "[TRAFFIC_STORE] Skipping malformed line {} (incomplete JSON)",
                    total_lines
                );
                skipped_count += 1;
                continue;
            }

            match serde_json::from_str::<TrafficRecord>(trimmed) {
                Ok(record) => {
                    if record.timestamp >= cutoff_timestamp {
                        if record.sequence > max_sequence {
                            max_sequence = record.sequence;
                        }
                        loaded_records.push_back(record);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "[TRAFFIC_STORE] Failed to parse record at line {}: {}",
                        total_lines,
                        e
                    );
                    skipped_count += 1;
                }
            }
        }

        while loaded_records.len() > max {
            loaded_records.pop_front();
        }

        let loaded_count = loaded_records.len();
        let need_rewrite = loaded_count > 0 || skipped_count > 0;
        let (oldest_ts, newest_ts) = {
            let first_ts = loaded_records.front().map(|r| r.timestamp);
            let last_ts = loaded_records.back().map(|r| r.timestamp);
            (first_ts, last_ts)
        };

        *self.records.write() = loaded_records;

        self.sequence.store(max_sequence + 1, Ordering::SeqCst);

        {
            let mut metadata = self.metadata.write();
            metadata.last_sequence = max_sequence;
            metadata.total_records = loaded_count as u64;
            metadata.updated_at = now;
            if loaded_count > 0 {
                metadata.oldest_record_timestamp = oldest_ts;
                metadata.newest_record_timestamp = newest_ts;
            }
        }

        tracing::info!(
            "[TRAFFIC_STORE] Loaded {} records from disk (skipped {} malformed), next sequence: {}",
            loaded_count,
            skipped_count,
            self.sequence.load(Ordering::SeqCst)
        );

        if need_rewrite {
            self.rewrite_records_file_internal();
            self.save_metadata_internal();
        }
    }

    fn cleanup_temp_files(&self) {
        let temp_path = self.traffic_dir.join("records.jsonl.tmp");
        if temp_path.exists() {
            tracing::info!(
                "[TRAFFIC_STORE] Cleaning up orphaned temp file: {}",
                temp_path.display()
            );
            let _ = fs::remove_file(&temp_path);
        }
    }

    fn save_metadata_internal(&self) {
        let metadata_snapshot = self.metadata.read().clone();

        let path = self.metadata_file_path();
        let temp_path = self.traffic_dir.join("metadata.json.tmp");

        if let Ok(content) = serde_json::to_string_pretty(&metadata_snapshot) {
            if fs::write(&temp_path, &content).is_ok() {
                if let Err(e) = fs::rename(&temp_path, &path) {
                    tracing::error!("[TRAFFIC_STORE] Failed to rename metadata file: {}", e);
                    let _ = fs::remove_file(&temp_path);
                    let _ = fs::write(&path, content);
                }
            } else {
                let _ = fs::write(&path, content);
            }
        }
        self.metadata_dirty.store(false, Ordering::SeqCst);
    }

    fn maybe_save_metadata(&self) {
        if !self.metadata_dirty.load(Ordering::Relaxed) {
            return;
        }

        let last_save = self.last_metadata_save.lock();
        if last_save.elapsed() >= Duration::from_secs(METADATA_SAVE_INTERVAL_SECS) {
            drop(last_save);
            self.save_metadata_internal();
            *self.last_metadata_save.lock() = Instant::now();
        }
    }

    fn flush_pending_writes(&self) {
        if self.file_needs_rewrite.load(Ordering::Relaxed) {
            {
                let mut pending = self.pending_writes.lock();
                pending.records.clear();
                pending.last_flush = Instant::now();
            }
            self.rewrite_records_file_internal();
            self.file_needs_rewrite.store(false, Ordering::SeqCst);
            return;
        }

        let records_to_write: Vec<TrafficRecord>;
        {
            let mut pending = self.pending_writes.lock();
            if pending.records.is_empty() {
                return;
            }
            records_to_write = std::mem::take(&mut pending.records);
            pending.last_flush = Instant::now();
        }

        if records_to_write.is_empty() {
            return;
        }

        let valid_ids: std::collections::HashSet<String> = {
            let current_records = self.records.read();
            current_records.iter().map(|r| r.id.clone()).collect()
        };

        let filtered_records: Vec<&TrafficRecord> = records_to_write
            .iter()
            .filter(|r| valid_ids.contains(&r.id))
            .collect();

        if filtered_records.is_empty() {
            return;
        }

        let path = self.records_file_path();
        let file = match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("[TRAFFIC_STORE] Failed to open file for batch write: {}", e);
                return;
            }
        };

        let mut writer = BufWriter::new(file);
        for record in &filtered_records {
            if let Ok(json) = serde_json::to_string(record) {
                let _ = writeln!(writer, "{}", json);
            }
        }
        let _ = writer.flush();

        tracing::debug!(
            "[TRAFFIC_STORE] Flushed {} records to disk",
            filtered_records.len()
        );
    }

    fn maybe_flush_pending(&self) {
        let should_flush = {
            let pending = self.pending_writes.lock();
            pending.records.len() >= FLUSH_BATCH_SIZE
                || (pending.last_flush.elapsed() >= Duration::from_secs(FLUSH_INTERVAL_SECS)
                    && !pending.records.is_empty())
        };

        if should_flush {
            self.flush_pending_writes();
            self.maybe_save_metadata();
        }
    }

    fn rewrite_records_file_internal(&self) {
        let records_snapshot: Vec<TrafficRecord> = self.records.read().iter().cloned().collect();

        let path = self.records_file_path();
        let temp_path = self.traffic_dir.join("records.jsonl.tmp");

        let file = match File::create(&temp_path) {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("[TRAFFIC_STORE] Failed to create temp file: {}", e);
                return;
            }
        };

        let mut writer = BufWriter::new(file);
        for record in &records_snapshot {
            if let Ok(json) = serde_json::to_string(record) {
                let _ = writeln!(writer, "{}", json);
            }
        }

        if let Err(e) = writer.flush() {
            tracing::error!("[TRAFFIC_STORE] Failed to flush temp file: {}", e);
            let _ = fs::remove_file(&temp_path);
            return;
        }
        drop(writer);

        if let Err(e) = fs::rename(&temp_path, &path) {
            tracing::error!("[TRAFFIC_STORE] Failed to rename temp file: {}", e);
            let _ = fs::remove_file(&temp_path);
        }
    }

    fn rewrite_records_file(&self) {
        {
            let mut pending = self.pending_writes.lock();
            pending.records.clear();
            pending.last_flush = Instant::now();
        }
        self.rewrite_records_file_internal();
    }

    fn mark_file_dirty(&self) {
        self.file_needs_rewrite.store(true, Ordering::SeqCst);
    }

    pub fn update_config(&self, update: TrafficStoreConfigUpdate) {
        if let Some(max_records) = update.max_records {
            let old = self.max_records.swap(max_records, Ordering::SeqCst);
            if old != max_records {
                tracing::info!(
                    "[TRAFFIC_STORE] Config updated: max_records {} -> {}",
                    old,
                    max_records
                );
                self.trim_records_to_limit();
            }
        }
        if let Some(retention_hours) = update.retention_hours {
            let old = self.retention_hours.swap(retention_hours, Ordering::SeqCst);
            if old != retention_hours {
                tracing::info!(
                    "[TRAFFIC_STORE] Config updated: retention_hours {} -> {}",
                    old,
                    retention_hours
                );
            }
        }
    }

    fn trim_records_to_limit(&self) {
        let max = self.max_records.load(Ordering::Relaxed);
        let mut records = self.records.write();
        let mut trimmed = false;
        while records.len() > max {
            records.pop_front();
            trimmed = true;
        }
        if trimmed {
            drop(records);
            self.mark_file_dirty();
        }
    }

    pub fn cleanup_expired_records(&self) -> usize {
        let retention_hours = self.retention_hours.load(Ordering::Relaxed);
        let retention_duration = Duration::from_secs(retention_hours * 60 * 60);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let cutoff_timestamp = now.saturating_sub(retention_duration.as_millis() as u64);

        let mut records = self.records.write();
        let original_len = records.len();

        records.retain(|r| r.timestamp >= cutoff_timestamp);

        let removed_count = original_len - records.len();
        drop(records);

        if removed_count > 0 {
            self.rewrite_records_file();
            self.save_metadata_internal();
            tracing::info!(
                "[TRAFFIC_STORE] Cleaned up {} expired records",
                removed_count
            );
        }

        *self.last_cleanup.write() = Instant::now();
        removed_count
    }

    pub fn record(&self, mut record: TrafficRecord) {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        record.sequence = seq;

        let _ = self.tx.send(record.clone());

        {
            let mut metadata = self.metadata.write();
            metadata.total_records += 1;
            metadata.last_sequence = seq;
            metadata.updated_at = record.timestamp;
            if metadata.oldest_record_timestamp.is_none() {
                metadata.oldest_record_timestamp = Some(record.timestamp);
            }
            metadata.newest_record_timestamp = Some(record.timestamp);
        }
        self.metadata_dirty.store(true, Ordering::SeqCst);

        let max = self.max_records.load(Ordering::Relaxed);
        let evicted;
        {
            let mut records = self.records.write();
            evicted = records.len() >= max;
            if evicted {
                records.pop_front();
            }
            records.push_back(record.clone());
        }

        if evicted {
            self.mark_file_dirty();
        }

        {
            let mut pending = self.pending_writes.lock();
            pending.records.push(record);
        }

        self.maybe_flush_pending();
    }

    pub fn set_max_records(&self, max_records: usize) {
        let old = self.max_records.swap(max_records, Ordering::SeqCst);
        if old != max_records {
            tracing::info!(
                "[TRAFFIC_STORE] Config updated: max_records {} -> {}",
                old,
                max_records
            );
            self.trim_records_to_limit();
        }
    }

    pub fn get_all(&self) -> Vec<TrafficSummary> {
        self.records
            .read()
            .iter()
            .map(TrafficSummary::from)
            .collect()
    }

    pub fn get_recent(&self, limit: usize) -> Vec<TrafficSummary> {
        self.records
            .read()
            .iter()
            .rev()
            .take(limit)
            .map(TrafficSummary::from)
            .collect()
    }

    pub fn get_by_id(&self, id: &str) -> Option<TrafficRecord> {
        self.records.read().iter().find(|r| r.id == id).cloned()
    }

    pub fn update_by_id<F>(&self, id: &str, updater: F) -> bool
    where
        F: FnOnce(&mut TrafficRecord),
    {
        let mut records = self.records.write();
        if let Some(record) = records.iter_mut().find(|r| r.id == id) {
            updater(record);
            drop(records);
            self.mark_file_dirty();
            true
        } else {
            false
        }
    }

    pub fn clear(&self) {
        {
            let mut pending = self.pending_writes.lock();
            pending.records.clear();
            pending.last_flush = Instant::now();
        }

        self.records.write().clear();
        self.sequence.store(1, Ordering::SeqCst);
        self.file_needs_rewrite.store(false, Ordering::SeqCst);

        let records_path = self.records_file_path();
        let _ = fs::remove_file(&records_path);

        *self.metadata.write() = TrafficStoreMetadata::default();
        self.save_metadata_internal();

        tracing::info!("[TRAFFIC_STORE] Cleared all records");
    }

    pub fn count(&self) -> usize {
        self.records.read().len()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TrafficRecord> {
        self.tx.subscribe()
    }

    pub fn filter(&self, filter: &TrafficFilter) -> Vec<TrafficSummary> {
        self.records
            .read()
            .iter()
            .filter(|r| filter.matches(r))
            .map(TrafficSummary::from)
            .collect()
    }

    pub fn get_after(
        &self,
        after_id: Option<&str>,
        filter: &TrafficFilter,
        limit: usize,
    ) -> (Vec<TrafficSummary>, bool) {
        let records = self.records.read();

        let start_idx = if let Some(after_id) = after_id {
            records
                .iter()
                .position(|r| r.id == after_id)
                .map(|idx| idx + 1)
                .unwrap_or(0)
        } else {
            0
        };

        let filtered: Vec<TrafficSummary> = records
            .iter()
            .skip(start_idx)
            .filter(|r| filter.matches(r))
            .map(TrafficSummary::from)
            .collect();

        let total = filtered.len();
        let has_more = total > limit;
        let result = filtered.into_iter().take(limit).collect();

        (result, has_more)
    }

    pub fn get_by_ids(&self, ids: &[&str]) -> Vec<TrafficSummary> {
        let records = self.records.read();
        ids.iter()
            .filter_map(|id| records.iter().find(|r| r.id == *id))
            .map(TrafficSummary::from)
            .collect()
    }

    pub fn total(&self) -> usize {
        self.records.read().len()
    }

    pub fn current_sequence(&self) -> u64 {
        self.sequence.load(Ordering::SeqCst)
    }

    pub fn flush(&self) {
        self.flush_pending_writes();
        self.save_metadata_internal();
    }

    pub fn stats(&self) -> TrafficStoreStats {
        let metadata = self.metadata.read().clone();
        let records_path = self.records_file_path();
        let file_size = fs::metadata(&records_path).map(|m| m.len()).unwrap_or(0);
        let record_count = self.records.read().len();
        let pending_count = self.pending_writes.lock().records.len();

        TrafficStoreStats {
            record_count,
            file_size,
            total_records_processed: metadata.total_records,
            last_sequence: metadata.last_sequence,
            oldest_record_timestamp: metadata.oldest_record_timestamp,
            newest_record_timestamp: metadata.newest_record_timestamp,
            traffic_dir: self.traffic_dir.to_string_lossy().to_string(),
            max_records: self.max_records.load(Ordering::Relaxed),
            retention_hours: self.retention_hours.load(Ordering::Relaxed),
            pending_writes: pending_count,
        }
    }
}

impl Drop for TrafficStore {
    fn drop(&mut self) {
        self.flush_pending_writes();
        self.save_metadata_internal();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficStoreStats {
    pub record_count: usize,
    pub file_size: u64,
    pub total_records_processed: u64,
    pub last_sequence: u64,
    pub oldest_record_timestamp: Option<u64>,
    pub newest_record_timestamp: Option<u64>,
    pub traffic_dir: String,
    pub max_records: usize,
    pub retention_hours: u64,
    pub pending_writes: usize,
}

pub type SharedTrafficStore = Arc<TrafficStore>;

pub fn start_traffic_cleanup_task(store: SharedTrafficStore) {
    let store_for_flush = store.clone();
    tokio::spawn(async move {
        let mut flush_interval = tokio::time::interval(Duration::from_secs(FLUSH_INTERVAL_SECS));
        flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            flush_interval.tick().await;
            store_for_flush.flush();
        }
    });

    tokio::spawn(async move {
        let mut cleanup_interval =
            tokio::time::interval(Duration::from_secs(CLEANUP_INTERVAL_HOURS * 60 * 60));
        cleanup_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            cleanup_interval.tick().await;
            let removed = store.cleanup_expired_records();
            if removed > 0 {
                tracing::info!(
                    "[TRAFFIC_STORE] Periodic cleanup removed {} expired records",
                    removed
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic::TrafficRecord;
    use std::sync::atomic::AtomicU64;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn create_test_dir() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "bifrost_traffic_store_test_{}_{}_{}",
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

    fn create_test_record(id: &str) -> TrafficRecord {
        TrafficRecord::new(
            id.to_string(),
            "GET".to_string(),
            "https://example.com/api/test".to_string(),
        )
    }

    #[test]
    fn test_record_and_retrieve() {
        let dir = create_test_dir();
        let store = TrafficStore::new(dir.clone(), 100, Some(24));

        let record = create_test_record("test-1");
        store.record(record);

        assert_eq!(store.count(), 1);
        assert!(store.get_by_id("test-1").is_some());

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_persistence_and_recovery() {
        let dir = create_test_dir();

        {
            let store = TrafficStore::new(dir.clone(), 100, Some(24));
            store.record(create_test_record("test-1"));
            store.record(create_test_record("test-2"));
            store.record(create_test_record("test-3"));
            assert_eq!(store.count(), 3);
            store.flush();
        }

        {
            let store = TrafficStore::new(dir.clone(), 100, Some(24));
            assert_eq!(store.count(), 3);
            assert!(store.get_by_id("test-1").is_some());
            assert!(store.get_by_id("test-2").is_some());
            assert!(store.get_by_id("test-3").is_some());
        }

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_max_records_limit() {
        let dir = create_test_dir();
        let store = TrafficStore::new(dir.clone(), 3, Some(24));

        for i in 0..5 {
            store.record(create_test_record(&format!("test-{}", i)));
        }

        assert_eq!(store.count(), 3);
        assert!(store.get_by_id("test-0").is_none());
        assert!(store.get_by_id("test-1").is_none());
        assert!(store.get_by_id("test-2").is_some());
        assert!(store.get_by_id("test-3").is_some());
        assert!(store.get_by_id("test-4").is_some());

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_sequence_continuity_after_recovery() {
        let dir = create_test_dir();
        let last_seq;

        {
            let store = TrafficStore::new(dir.clone(), 100, Some(24));
            store.record(create_test_record("test-1"));
            store.record(create_test_record("test-2"));
            let record = store.get_by_id("test-2").unwrap();
            last_seq = record.sequence;
            store.flush();
        }

        {
            let store = TrafficStore::new(dir.clone(), 100, Some(24));
            store.record(create_test_record("test-3"));
            let record = store.get_by_id("test-3").unwrap();
            assert!(record.sequence > last_seq);
        }

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_clear() {
        let dir = create_test_dir();
        let store = TrafficStore::new(dir.clone(), 100, Some(24));

        store.record(create_test_record("test-1"));
        store.record(create_test_record("test-2"));
        assert_eq!(store.count(), 2);

        store.clear();
        assert_eq!(store.count(), 0);

        store.record(create_test_record("test-3"));
        let record = store.get_by_id("test-3").unwrap();
        assert_eq!(record.sequence, 1);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_stats() {
        let dir = create_test_dir();
        let store = TrafficStore::new(dir.clone(), 100, Some(24));

        store.record(create_test_record("test-1"));
        store.record(create_test_record("test-2"));
        store.flush();

        let stats = store.stats();
        assert_eq!(stats.record_count, 2);
        assert!(stats.file_size > 0);
        assert_eq!(stats.max_records, 100);
        assert_eq!(stats.retention_hours, 24);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_batch_flush() {
        let dir = create_test_dir();
        let store = TrafficStore::new(dir.clone(), 100, Some(24));

        for i in 0..10 {
            store.record(create_test_record(&format!("test-{}", i)));
        }

        {
            let stats = store.stats();
            assert!(stats.pending_writes > 0 || stats.file_size > 0);
        }

        store.flush();

        {
            let stats = store.stats();
            assert_eq!(stats.pending_writes, 0);
            assert!(stats.file_size > 0);
        }

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_malformed_json_recovery() {
        let dir = create_test_dir();

        {
            let store = TrafficStore::new(dir.clone(), 100, Some(24));
            store.record(create_test_record("test-1"));
            store.record(create_test_record("test-2"));
            store.flush();
        }

        let records_path = dir.join(TRAFFIC_RECORDS_FILE);
        let content = fs::read_to_string(&records_path).unwrap();
        let corrupted = format!("{}\n{{incomplete json\n", content);
        fs::write(&records_path, corrupted).unwrap();

        {
            let store = TrafficStore::new(dir.clone(), 100, Some(24));
            assert_eq!(store.count(), 2);
            assert!(store.get_by_id("test-1").is_some());
            assert!(store.get_by_id("test-2").is_some());
        }

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_temp_file_cleanup() {
        let dir = create_test_dir();

        let temp_path = dir.join("records.jsonl.tmp");
        fs::write(&temp_path, "orphaned temp file").unwrap();
        assert!(temp_path.exists());

        let store = TrafficStore::new(dir.clone(), 100, Some(24));
        assert!(!temp_path.exists());
        drop(store);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_eviction_file_consistency() {
        let dir = create_test_dir();
        let store = TrafficStore::new(dir.clone(), 3, Some(24));

        for i in 0..5 {
            store.record(create_test_record(&format!("test-{}", i)));
        }
        store.flush();

        let records_path = dir.join(TRAFFIC_RECORDS_FILE);
        let content = fs::read_to_string(&records_path).unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 3);

        cleanup_test_dir(&dir);
    }

    #[test]
    fn test_sequence_from_file_not_metadata() {
        let dir = create_test_dir();

        {
            let store = TrafficStore::new(dir.clone(), 100, Some(24));
            store.record(create_test_record("test-1"));
            store.record(create_test_record("test-2"));
            store.flush();
        }

        let metadata_path = dir.join(TRAFFIC_METADATA_FILE);
        let mut metadata: TrafficStoreMetadata =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();
        metadata.last_sequence = 100;
        fs::write(&metadata_path, serde_json::to_string(&metadata).unwrap()).unwrap();

        {
            let store = TrafficStore::new(dir.clone(), 100, Some(24));
            store.record(create_test_record("test-3"));
            let record = store.get_by_id("test-3").unwrap();
            assert_eq!(record.sequence, 3);
        }

        cleanup_test_dir(&dir);
    }
}
