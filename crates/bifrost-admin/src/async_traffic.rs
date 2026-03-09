use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::traffic::TrafficRecord;
use crate::traffic_db::SharedTrafficDbStore;
use crate::traffic_store::SharedTrafficStore;

pub type TrafficUpdater = Arc<dyn Fn(&mut TrafficRecord) + Send + Sync>;

#[derive(Clone)]
pub enum TrafficCommand {
    Record(Box<TrafficRecord>),
    Update { id: String, updater: TrafficUpdater },
}

impl std::fmt::Debug for TrafficCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrafficCommand::Record(record) => f.debug_tuple("Record").field(&record.id).finish(),
            TrafficCommand::Update { id, .. } => f
                .debug_struct("Update")
                .field("id", id)
                .field("updater", &"<fn>")
                .finish(),
        }
    }
}

pub struct AsyncTrafficWriter {
    tx: mpsc::Sender<TrafficCommand>,
}

impl AsyncTrafficWriter {
    pub fn new(buffer_size: usize) -> (Self, mpsc::Receiver<TrafficCommand>) {
        let (tx, rx) = mpsc::channel(buffer_size);
        (Self { tx }, rx)
    }

    #[inline]
    pub fn record(&self, record: TrafficRecord) {
        if let Err(e) = self.tx.try_send(TrafficCommand::Record(Box::new(record))) {
            match e {
                mpsc::error::TrySendError::Full(_) => {
                    warn!("Traffic channel full, dropping record");
                }
                mpsc::error::TrySendError::Closed(_) => {
                    error!("Traffic channel closed");
                }
            }
        }
    }

    #[inline]
    pub fn update_by_id<F>(&self, id: &str, updater: F)
    where
        F: Fn(&mut TrafficRecord) + Send + Sync + 'static,
    {
        let cmd = TrafficCommand::Update {
            id: id.to_string(),
            updater: Arc::new(updater),
        };
        if let Err(e) = self.tx.try_send(cmd) {
            match e {
                mpsc::error::TrySendError::Full(_) => {
                    warn!("Traffic channel full, dropping update for {}", id);
                }
                mpsc::error::TrySendError::Closed(_) => {
                    error!("Traffic channel closed");
                }
            }
        }
    }
}

impl Clone for AsyncTrafficWriter {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

pub type SharedAsyncTrafficWriter = Arc<AsyncTrafficWriter>;

pub fn start_async_traffic_processor(
    mut rx: mpsc::Receiver<TrafficCommand>,
    traffic_db_store: Option<SharedTrafficDbStore>,
    traffic_store: Option<SharedTrafficStore>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("Async traffic processor started");

        let mut batch: Vec<Box<TrafficRecord>> = Vec::with_capacity(64);
        let mut updates: Vec<(String, TrafficUpdater)> = Vec::with_capacity(32);

        loop {
            batch.clear();
            updates.clear();

            match rx.recv().await {
                Some(cmd) => {
                    match cmd {
                        TrafficCommand::Record(record) => batch.push(record),
                        TrafficCommand::Update { id, updater } => updates.push((id, updater)),
                    }

                    while batch.len() < 64 && updates.len() < 32 {
                        match rx.try_recv() {
                            Ok(TrafficCommand::Record(record)) => batch.push(record),
                            Ok(TrafficCommand::Update { id, updater }) => {
                                updates.push((id, updater))
                            }
                            Err(mpsc::error::TryRecvError::Empty) => break,
                            Err(mpsc::error::TryRecvError::Disconnected) => {
                                info!("Traffic channel disconnected, processing remaining batch");
                                break;
                            }
                        }
                    }

                    if !batch.is_empty() {
                        let batch_size = batch.len();
                        for record in batch.drain(..) {
                            if let Some(ref store) = traffic_store {
                                store.record(*record.clone());
                            }
                            if let Some(ref db_store) = traffic_db_store {
                                db_store.record(*record);
                            }
                        }
                        debug!("Processed {} traffic records", batch_size);
                    }

                    if !updates.is_empty() {
                        let update_count = updates.len();
                        for (id, updater) in updates.drain(..) {
                            if let Some(ref store) = traffic_store {
                                store.update_by_id(&id, |r| updater(r));
                            }
                            if let Some(ref db_store) = traffic_db_store {
                                db_store.update_by_id(&id, |r| updater(r));
                            }
                        }
                        debug!("Processed {} traffic updates", update_count);
                    }
                }
                None => {
                    info!("Traffic channel closed, shutting down processor");
                    break;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic_db::TrafficDbStore;
    use std::sync::Arc;
    use std::time::Duration;

    fn make_temp_dir(name: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        let ts = chrono::Utc::now().timestamp_millis();
        let dir = std::env::temp_dir().join(format!("bifrost-{}-{}-{}", name, pid, ts));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn test_async_traffic_writer() {
        let (writer, rx) = AsyncTrafficWriter::new(100);
        let dir = make_temp_dir("async-traffic-writer");
        let traffic_dir = dir.join("traffic");
        let _ = std::fs::create_dir_all(&traffic_dir);
        let db_store = Arc::new(
            TrafficDbStore::new(traffic_dir, 1000, 0, None)
                .expect("failed to create traffic db store"),
        );

        let handle = start_async_traffic_processor(rx, Some(db_store.clone()), None);

        let record = TrafficRecord::new(
            "test-1".to_string(),
            "GET".to_string(),
            "https://example.com".to_string(),
        );
        writer.record(record);

        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(db_store.get_by_id("test-1").is_some());

        drop(writer);
        let _ = tokio::time::timeout(Duration::from_secs(1), handle).await;
    }

    #[tokio::test]
    async fn test_async_traffic_update() {
        let (writer, rx) = AsyncTrafficWriter::new(100);
        let dir = make_temp_dir("async-traffic-update");
        let traffic_dir = dir.join("traffic");
        let _ = std::fs::create_dir_all(&traffic_dir);
        let db_store = Arc::new(
            TrafficDbStore::new(traffic_dir, 1000, 0, None)
                .expect("failed to create traffic db store"),
        );

        let handle = start_async_traffic_processor(rx, Some(db_store.clone()), None);

        let record = TrafficRecord::new(
            "test-update".to_string(),
            "GET".to_string(),
            "https://example.com".to_string(),
        );
        writer.record(record);

        tokio::time::sleep(Duration::from_millis(50)).await;

        writer.update_by_id("test-update", |r| {
            r.status = 200;
            r.duration_ms = 100;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let updated = db_store.get_by_id("test-update").unwrap();
        assert_eq!(updated.status, 200);
        assert_eq!(updated.duration_ms, 100);

        drop(writer);
        let _ = tokio::time::timeout(Duration::from_secs(1), handle).await;
    }

    #[tokio::test]
    async fn test_batch_processing() {
        let (writer, rx) = AsyncTrafficWriter::new(1000);
        let dir = make_temp_dir("async-traffic-batch");
        let traffic_dir = dir.join("traffic");
        let _ = std::fs::create_dir_all(&traffic_dir);
        let db_store = Arc::new(
            TrafficDbStore::new(traffic_dir, 10000, 0, None)
                .expect("failed to create traffic db store"),
        );

        let handle = start_async_traffic_processor(rx, Some(db_store.clone()), None);

        for i in 0..100 {
            let record = TrafficRecord::new(
                format!("batch-{}", i),
                "GET".to_string(),
                format!("https://example.com/{}", i),
            );
            writer.record(record);
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(db_store.count(), 100);

        drop(writer);
        let _ = tokio::time::timeout(Duration::from_secs(1), handle).await;
    }
}
