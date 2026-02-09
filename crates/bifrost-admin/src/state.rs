use std::sync::Arc;

use bifrost_core::ClientAccessControl;
use bifrost_storage::RulesStorage;
use tokio::sync::RwLock;

use crate::body_store::SharedBodyStore;
use crate::metrics::{MetricsCollector, SharedMetricsCollector};
use crate::traffic::{SharedTrafficRecorder, TrafficRecorder};

pub type SharedAccessControl = Arc<RwLock<ClientAccessControl>>;

pub struct AdminState {
    pub traffic_recorder: SharedTrafficRecorder,
    pub metrics_collector: SharedMetricsCollector,
    pub rules_storage: RulesStorage,
    pub access_control: Option<SharedAccessControl>,
    pub body_store: Option<SharedBodyStore>,
    pub start_time: u64,
    pub port: u16,
}

impl AdminState {
    pub fn new(port: u16) -> Self {
        Self {
            traffic_recorder: Arc::new(TrafficRecorder::default()),
            metrics_collector: Arc::new(MetricsCollector::default()),
            rules_storage: RulesStorage::default(),
            access_control: None,
            body_store: None,
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

    pub fn with_access_control(mut self, access_control: SharedAccessControl) -> Self {
        self.access_control = Some(access_control);
        self
    }

    pub fn with_body_store(mut self, body_store: SharedBodyStore) -> Self {
        self.body_store = Some(body_store);
        self
    }
}

pub type SharedAdminState = Arc<AdminState>;
