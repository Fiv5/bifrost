use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use lru::LruCache;
use parking_lot::Mutex;

use crate::body_store::BodyRef;

const WS_PAYLOAD_SUBDIR: &str = "ws_payload";

struct WsPayloadWriter {
    path: PathBuf,
    file: fs::File,
    buffer: Vec<u8>,
    size: u64,
    last_flush: Instant,
    flush_bytes: usize,
    flush_interval: Duration,
}

impl WsPayloadWriter {
    fn new(path: PathBuf, flush_bytes: usize, flush_interval: Duration) -> std::io::Result<Self> {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            path,
            file,
            buffer: Vec::with_capacity(flush_bytes),
            size,
            last_flush: Instant::now(),
            flush_bytes,
            flush_interval,
        })
    }

    fn append(&mut self, bytes: &[u8]) -> std::io::Result<BodyRef> {
        let offset = self.size;
        self.size += bytes.len() as u64;
        self.buffer.extend_from_slice(bytes);
        if self.buffer.len() >= self.flush_bytes || self.last_flush.elapsed() >= self.flush_interval
        {
            self.flush()?;
        }
        Ok(BodyRef::FileRange {
            path: self.path.to_string_lossy().to_string(),
            offset,
            size: bytes.len(),
        })
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

    fn update_config(&mut self, flush_bytes: usize, flush_interval: Duration) {
        self.flush_bytes = flush_bytes;
        self.flush_interval = flush_interval;
        if self.buffer.capacity() < flush_bytes {
            self.buffer.reserve(flush_bytes - self.buffer.capacity());
        }
    }
}

struct WsPayloadStoreState {
    flush_bytes: usize,
    flush_interval: Duration,
    max_open_files: usize,
    retention_days: u64,
    writers: LruCache<String, WsPayloadWriter>,
}

pub struct WsPayloadStore {
    base_dir: PathBuf,
    state: Mutex<WsPayloadStoreState>,
}

#[derive(Debug, Clone, Default)]
pub struct WsPayloadStoreConfigUpdate {
    pub flush_bytes: Option<usize>,
    pub flush_interval_ms: Option<u64>,
    pub max_open_files: Option<usize>,
    pub retention_days: Option<u64>,
}

impl WsPayloadStore {
    pub fn new(
        base_dir: PathBuf,
        flush_bytes: usize,
        flush_interval: Duration,
        max_open_files: usize,
        retention_days: u64,
    ) -> Self {
        let payload_dir = base_dir.join(WS_PAYLOAD_SUBDIR);
        if !payload_dir.exists() {
            let _ = fs::create_dir_all(&payload_dir);
        }
        let cache_size = std::num::NonZeroUsize::new(max_open_files.max(1))
            .unwrap_or_else(|| std::num::NonZeroUsize::new(1).expect("non-zero"));
        Self {
            base_dir,
            state: Mutex::new(WsPayloadStoreState {
                flush_bytes,
                flush_interval,
                max_open_files,
                retention_days,
                writers: LruCache::new(cache_size),
            }),
        }
    }

    fn payload_dir(&self) -> PathBuf {
        self.base_dir.join(WS_PAYLOAD_SUBDIR)
    }

    fn safe_connection_id(connection_id: &str) -> String {
        connection_id.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
    }

    fn connection_path(&self, safe_id: &str) -> PathBuf {
        self.payload_dir().join(format!("{}.bin", safe_id))
    }

    pub fn append_bytes(&self, connection_id: &str, bytes: &[u8]) -> Option<BodyRef> {
        if bytes.is_empty() {
            return None;
        }
        let safe_id = Self::safe_connection_id(connection_id);
        let path = self.connection_path(&safe_id);
        let mut state = self.state.lock();
        if state.writers.get(&safe_id).is_none() {
            if let Ok(writer) = WsPayloadWriter::new(path, state.flush_bytes, state.flush_interval)
            {
                state.writers.put(safe_id.clone(), writer);
                while state.writers.len() > state.max_open_files {
                    if let Some((_id, mut evicted)) = state.writers.pop_lru() {
                        let _ = evicted.flush();
                    } else {
                        break;
                    }
                }
            } else {
                return None;
            }
        }
        let writer = state.writers.get_mut(&safe_id)?;
        writer.append(bytes).ok()
    }

    pub fn is_ws_payload_ref(&self, body_ref: &BodyRef) -> bool {
        match body_ref {
            BodyRef::FileRange { path, .. } => PathBuf::from(path).starts_with(self.payload_dir()),
            _ => false,
        }
    }

    pub fn read_range(&self, body_ref: &BodyRef) -> Option<Vec<u8>> {
        let (path, offset, size) = match body_ref {
            BodyRef::FileRange { path, offset, size } => (path, *offset, *size),
            _ => return None,
        };

        let safe_id = PathBuf::from(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());

        if let Some(safe_id) = safe_id {
            let mut state = self.state.lock();
            if let Some(writer) = state.writers.get_mut(&safe_id) {
                let _ = writer.flush();
            }
        }

        let path = PathBuf::from(path);
        if !path.exists() {
            return None;
        }
        let mut file = fs::File::open(&path).ok()?;
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut contents = vec![0u8; size];
        let mut read_size = 0usize;
        while read_size < size {
            let n = file.read(&mut contents[read_size..]).ok()?;
            if n == 0 {
                break;
            }
            read_size += n;
        }
        contents.truncate(read_size);
        Some(contents)
    }

    pub fn close(&self, connection_id: &str) {
        let safe_id = Self::safe_connection_id(connection_id);
        let mut state = self.state.lock();
        if let Some(mut writer) = state.writers.pop(&safe_id) {
            let _ = writer.flush();
        }
    }

    pub fn update_config(&self, update: WsPayloadStoreConfigUpdate) {
        let mut state = self.state.lock();
        if let Some(flush_bytes) = update.flush_bytes {
            state.flush_bytes = flush_bytes;
        }
        if let Some(interval_ms) = update.flush_interval_ms {
            state.flush_interval = Duration::from_millis(interval_ms);
        }
        if let Some(max_open_files) = update.max_open_files {
            state.max_open_files = max_open_files.max(1);
        }
        if let Some(retention_days) = update.retention_days {
            state.retention_days = retention_days;
        }
        let flush_bytes = state.flush_bytes;
        let flush_interval = state.flush_interval;
        for (_, writer) in state.writers.iter_mut() {
            writer.update_config(flush_bytes, flush_interval);
        }
        while state.writers.len() > state.max_open_files {
            if let Some((_id, mut evicted)) = state.writers.pop_lru() {
                let _ = evicted.flush();
            } else {
                break;
            }
        }
    }

    pub fn cleanup_expired(&self) -> std::io::Result<usize> {
        let payload_dir = self.payload_dir();
        if !payload_dir.exists() {
            return Ok(0);
        }
        let retention_duration = Duration::from_secs(self.get_retention_days() * 24 * 60 * 60);
        let now = SystemTime::now();
        let mut removed_count = 0;
        for entry in fs::read_dir(payload_dir)? {
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
        let payload_dir = self.payload_dir();
        if !payload_dir.exists() {
            return Ok(0);
        }
        let mut removed_count = 0;
        for entry in fs::read_dir(payload_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && fs::remove_file(&path).is_ok() {
                removed_count += 1;
            }
        }
        let mut state = self.state.lock();
        state.writers.clear();
        Ok(removed_count)
    }

    fn get_retention_days(&self) -> u64 {
        self.state.lock().retention_days
    }
}

pub type SharedWsPayloadStore = std::sync::Arc<WsPayloadStore>;

pub fn start_ws_payload_cleanup_task(store: SharedWsPayloadStore) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Ok(removed) = store.cleanup_expired() {
                if removed > 0 {
                    tracing::info!(
                        "[WS_PAYLOAD_STORE] Periodic cleanup removed {} expired files",
                        removed
                    );
                }
            }
        }
    });
}
