use std::sync::Arc;

use bifrost_storage::RulesStorage;

use crate::metrics::{MetricsCollector, SharedMetricsCollector};
use crate::traffic::{SharedTrafficRecorder, TrafficRecorder};

pub struct AdminState {
    pub traffic_recorder: SharedTrafficRecorder,
    pub metrics_collector: SharedMetricsCollector,
    pub rules_storage: RulesStorage,
    pub start_time: u64,
    pub port: u16,
}

impl AdminState {
    pub fn new(port: u16) -> Self {
        Self {
            traffic_recorder: Arc::new(TrafficRecorder::default()),
            metrics_collector: Arc::new(MetricsCollector::default()),
            rules_storage: RulesStorage::default(),
            start_time: chrono::Utc::now().timestamp() as u64,
            port,
        }
    }

    pub fn with_rules_storage(mut self, storage: RulesStorage) -> Self {
        self.rules_storage = storage;
        self
    }

    pub fn with_traffic_recorder(mut self, recorder: TrafficRecorder) -> Self {
        self.traffic_recorder = Arc::new(recorder);
        self
    }

    pub fn with_metrics_collector(mut self, collector: MetricsCollector) -> Self {
        self.metrics_collector = Arc::new(collector);
        self
    }
}

pub type SharedAdminState = Arc<AdminState>;
