use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::info;

use crate::state::SharedAdminState;
use crate::traffic::TrafficSummary;
use crate::traffic_db::{Direction, QueryParams, TrafficSummaryCompact};

static CLIENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
const PUSH_CHANNEL_CAPACITY: usize = 64;
pub const MAX_SUBSCRIBED_IDS: usize = 500;
pub const MAX_CLIENT_CHANNELS: usize = 3;
pub const MAX_ID_LEN: usize = 256;

fn generate_client_id() -> u64 {
    CLIENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PushMessage {
    #[serde(rename = "traffic_updates")]
    TrafficUpdates(TrafficUpdatesData),

    #[serde(rename = "traffic_delta")]
    TrafficDelta(TrafficDeltaData),

    #[serde(rename = "traffic_deleted")]
    TrafficDeleted(TrafficDeletedData),

    #[serde(rename = "overview_update")]
    OverviewUpdate(OverviewData),

    #[serde(rename = "metrics_update")]
    MetricsUpdate(MetricsData),

    #[serde(rename = "history_update")]
    HistoryUpdate(HistoryData),

    #[serde(rename = "connected")]
    Connected(ConnectedData),

    #[serde(rename = "error")]
    Error(ErrorData),

    #[serde(rename = "replay_request_updated")]
    ReplayRequestUpdated(ReplayRequestUpdatedData),

    #[serde(rename = "replay_history_updated")]
    ReplayHistoryUpdated(ReplayHistoryUpdatedData),

    #[serde(rename = "disconnect")]
    Disconnect(DisconnectData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficUpdatesData {
    pub new_records: Vec<TrafficSummary>,
    pub updated_records: Vec<TrafficSummary>,
    pub has_more: bool,
    pub server_total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficDeltaData {
    pub inserts: Vec<TrafficSummaryCompact>,
    pub updates: Vec<TrafficSummaryCompact>,
    pub has_more: bool,
    pub server_total: usize,
    pub server_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficDeletedData {
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewData {
    pub system: serde_json::Value,
    pub metrics: serde_json::Value,
    pub rules: RulesInfo,
    pub traffic: TrafficInfo,
    pub server: ServerInfo,
    pub pending_authorizations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesInfo {
    pub total: usize,
    pub enabled: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficInfo {
    pub recorded: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub port: u16,
    pub admin_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsData {
    pub metrics: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryData {
    pub history: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRequestUpdatedData {
    pub action: String,
    pub request_id: Option<String>,
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayHistoryUpdatedData {
    pub action: String,
    pub request_id: String,
    pub history_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedData {
    pub client_id: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectData {
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientSubscription {
    #[serde(default)]
    pub last_traffic_id: Option<String>,
    #[serde(default)]
    pub last_sequence: Option<u64>,
    #[serde(default)]
    pub pending_ids: Vec<String>,
    #[serde(default)]
    pub need_overview: bool,
    #[serde(default)]
    pub need_metrics: bool,
    #[serde(default)]
    pub need_history: bool,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default = "default_metrics_interval_ms")]
    pub metrics_interval_ms: u64,
}

fn default_history_limit() -> usize {
    60
}

fn default_metrics_interval_ms() -> u64 {
    1000
}

pub const METRICS_INTERVAL_MIN_MS: u64 = 200;
pub const METRICS_INTERVAL_MAX_MS: u64 = 5000;

impl Default for ClientSubscription {
    fn default() -> Self {
        Self {
            last_traffic_id: None,
            last_sequence: None,
            pending_ids: Vec::new(),
            need_overview: false,
            need_metrics: false,
            need_history: false,
            history_limit: default_history_limit(),
            metrics_interval_ms: default_metrics_interval_ms(),
        }
    }
}

pub struct PushClient {
    pub id: u64,
    pub client_key: String,
    pub sender: mpsc::Sender<PushMessage>,
    pub subscription: RwLock<ClientSubscription>,
}

impl PushClient {
    pub fn new(
        client_key: String,
        subscription: ClientSubscription,
    ) -> (Self, mpsc::Receiver<PushMessage>) {
        let (sender, receiver) = mpsc::channel(PUSH_CHANNEL_CAPACITY);
        let client = Self {
            id: generate_client_id(),
            client_key,
            sender,
            subscription: RwLock::new(subscription),
        };
        (client, receiver)
    }

    pub fn send(&self, msg: PushMessage) -> bool {
        self.sender.try_send(msg).is_ok()
    }

    pub fn update_subscription(&self, subscription: ClientSubscription) {
        *self.subscription.write() = subscription;
    }

    pub fn get_subscription(&self) -> ClientSubscription {
        self.subscription.read().clone()
    }
}

pub struct PushManager {
    clients: DashMap<u64, Arc<PushClient>>,
    buckets: DashMap<String, Vec<u64>>,
    bucket_order: Mutex<VecDeque<String>>,
    state: SharedAdminState,
}

impl PushManager {
    pub fn new(state: SharedAdminState) -> Self {
        Self {
            clients: DashMap::new(),
            buckets: DashMap::new(),
            bucket_order: Mutex::new(VecDeque::new()),
            state,
        }
    }

    pub fn register_client(
        &self,
        client_key: String,
        subscription: ClientSubscription,
    ) -> (Arc<PushClient>, mpsc::Receiver<PushMessage>) {
        let evicted = self.ensure_bucket_capacity(&client_key);
        for client_id in evicted {
            if let Some((_, client)) = self.clients.remove(&client_id) {
                let _ = client.send(PushMessage::Disconnect(DisconnectData {
                    reason: "Too many active client channels".to_string(),
                }));
            }
        }

        let (client, receiver) = PushClient::new(client_key.clone(), subscription);
        let client = Arc::new(client);
        let client_id = client.id;
        self.clients.insert(client_id, client.clone());
        self.buckets
            .entry(client_key)
            .and_modify(|v| v.push(client_id))
            .or_insert_with(|| vec![client_id]);
        info!(client_id = client_id, "Push client registered");
        (client, receiver)
    }

    pub fn unregister_client(&self, client_id: u64) {
        if let Some((_, client)) = self.clients.remove(&client_id) {
            if let Some(mut bucket) = self.buckets.get_mut(&client.client_key) {
                bucket.retain(|id| *id != client_id);
                if bucket.is_empty() {
                    drop(bucket);
                    self.buckets.remove(&client.client_key);
                    let mut order = self.bucket_order.lock();
                    order.retain(|k| k != &client.client_key);
                }
            }
            info!(client_id = client_id, "Push client unregistered");
        }
    }

    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    fn ensure_bucket_capacity(&self, client_key: &str) -> Vec<u64> {
        let mut evicted_client_ids = Vec::new();
        let mut order = self.bucket_order.lock();

        if let Some(pos) = order.iter().position(|k| k == client_key) {
            let k = order.remove(pos).unwrap_or_else(|| client_key.to_string());
            order.push_back(k);
        } else {
            order.push_back(client_key.to_string());
        }

        while order.len() > MAX_CLIENT_CHANNELS {
            let Some(evicted_key) = order.pop_front() else {
                break;
            };
            if let Some((_, client_ids)) = self.buckets.remove(&evicted_key) {
                evicted_client_ids.extend(client_ids);
            }
        }

        evicted_client_ids
    }

    fn enrich_summary(&self, mut summary: TrafficSummary) -> TrafficSummary {
        if summary.is_sse || summary.is_websocket || summary.is_tunnel {
            if summary.is_sse {
                if let Some(status) = self.state.sse_hub.get_socket_status(&summary.id) {
                    summary.frame_count = status.frame_count;
                    summary.socket_status = Some(status);
                }
            } else if let Some(status) = self
                .state
                .connection_monitor
                .get_connection_status(&summary.id)
            {
                summary.frame_count = status.frame_count;
                summary.socket_status = Some(status);
            } else if let Some(ref fs) = self.state.frame_store {
                if let Some(metadata) = fs.get_metadata(&summary.id) {
                    summary.frame_count = metadata.frame_count as usize;
                    summary.socket_status = Some(crate::traffic::SocketStatus {
                        is_open: !metadata.is_closed,
                        frame_count: metadata.frame_count as usize,
                        ..Default::default()
                    });
                }
            }

            if summary.is_sse {
                if let Some(ref socket_status) = summary.socket_status {
                    let total = socket_status.send_bytes + socket_status.receive_bytes;
                    summary.response_size = summary.response_size.max(total as usize);
                }
            }
        }
        summary
    }

    fn enrich_compact_summary(&self, mut summary: TrafficSummaryCompact) -> TrafficSummaryCompact {
        if summary.is_sse() || summary.is_websocket() || summary.is_tunnel() {
            if summary.is_sse() {
                if let Some(status) = self.state.sse_hub.get_socket_status(&summary.id) {
                    summary.fc = status.frame_count;
                    summary.ss = Some(status);
                }
            } else if let Some(status) = self
                .state
                .connection_monitor
                .get_connection_status(&summary.id)
            {
                summary.fc = status.frame_count;
                summary.ss = Some(status);
            } else if let Some(ref fs) = self.state.frame_store {
                if let Some(metadata) = fs.get_metadata(&summary.id) {
                    summary.fc = metadata.frame_count as usize;
                    summary.ss = Some(crate::traffic::SocketStatus {
                        is_open: !metadata.is_closed,
                        frame_count: metadata.frame_count as usize,
                        ..Default::default()
                    });
                }
            }

            if summary.is_sse() {
                if let Some(ref socket_status) = summary.ss {
                    let total = socket_status.send_bytes + socket_status.receive_bytes;
                    summary.res_sz = summary.res_sz.max(total as usize);
                }
            }
        }
        summary
    }

    pub async fn broadcast_traffic_updates(&self) {
        if let Some(ref db_store) = self.state.traffic_db_store {
            self.broadcast_traffic_delta(db_store).await;
            return;
        }
        self.broadcast_traffic_updates_legacy().await;
    }

    async fn broadcast_traffic_delta(&self, db_store: &crate::traffic_db::SharedTrafficDbStore) {
        let mut clients_to_remove = Vec::new();

        for client_ref in self.clients.iter() {
            let client = client_ref.value();
            let subscription = client.get_subscription();

            let query_params = QueryParams {
                cursor: subscription.last_sequence,
                limit: Some(500),
                direction: Direction::Forward,
                ..Default::default()
            };

            let result = db_store.query(&query_params);
            let new_records: Vec<_> = result
                .records
                .into_iter()
                .map(|s| self.enrich_compact_summary(s))
                .collect();

            let updated_records: Vec<TrafficSummaryCompact> =
                if !subscription.pending_ids.is_empty() {
                    let ids: Vec<&str> = subscription
                        .pending_ids
                        .iter()
                        .map(|s| s.as_str())
                        .collect();
                    db_store
                        .get_by_ids(&ids)
                        .into_iter()
                        .map(|s| self.enrich_compact_summary(s))
                        .collect()
                } else {
                    Vec::new()
                };

            if !new_records.is_empty() || !updated_records.is_empty() {
                let last_seq = new_records.last().map(|r| r.seq);

                let mut new_pending_ids: HashSet<String> =
                    subscription.pending_ids.iter().cloned().collect();

                for record in &updated_records {
                    let is_pending = record.s == 0
                        || ((record.is_websocket() || record.is_sse() || record.is_tunnel())
                            && record.ss.as_ref().map(|s| s.is_open).unwrap_or(false));
                    if !is_pending {
                        new_pending_ids.remove(&record.id);
                    }
                }

                for record in &new_records {
                    let is_pending = record.s == 0
                        || ((record.is_websocket() || record.is_sse() || record.is_tunnel())
                            && record.ss.as_ref().map(|s| s.is_open).unwrap_or(false));
                    if is_pending {
                        new_pending_ids.insert(record.id.clone());
                    }
                }

                let msg = PushMessage::TrafficDelta(TrafficDeltaData {
                    inserts: new_records,
                    updates: updated_records,
                    has_more: result.has_more,
                    server_total: result.total,
                    server_sequence: result.server_sequence,
                });

                if !client.send(msg) {
                    clients_to_remove.push(client.id);
                } else {
                    let mut sub = client.subscription.write();
                    if let Some(seq) = last_seq {
                        sub.last_sequence = Some(seq);
                    }
                    sub.pending_ids = new_pending_ids.into_iter().collect();
                }
            }
        }

        for client_id in clients_to_remove {
            self.unregister_client(client_id);
        }
    }

    fn send_initial_traffic_delta(
        &self,
        client: &Arc<PushClient>,
        db_store: &crate::traffic_db::SharedTrafficDbStore,
        subscription: &ClientSubscription,
    ) {
        let query_params = QueryParams {
            cursor: subscription.last_sequence,
            limit: Some(500),
            direction: Direction::Forward,
            ..Default::default()
        };

        let result = db_store.query(&query_params);
        let new_records: Vec<_> = result
            .records
            .into_iter()
            .map(|s| self.enrich_compact_summary(s))
            .collect();

        let updated_records: Vec<TrafficSummaryCompact> = if !subscription.pending_ids.is_empty() {
            let ids: Vec<&str> = subscription
                .pending_ids
                .iter()
                .map(|s| s.as_str())
                .collect();
            db_store
                .get_by_ids(&ids)
                .into_iter()
                .map(|s| self.enrich_compact_summary(s))
                .collect()
        } else {
            Vec::new()
        };

        if !new_records.is_empty() || !updated_records.is_empty() {
            let last_seq = new_records.last().map(|r| r.seq);

            let mut new_pending_ids: HashSet<String> =
                subscription.pending_ids.iter().cloned().collect();

            for record in &updated_records {
                let is_pending = record.s == 0
                    || ((record.is_websocket() || record.is_sse() || record.is_tunnel())
                        && record.ss.as_ref().map(|s| s.is_open).unwrap_or(false));
                if !is_pending {
                    new_pending_ids.remove(&record.id);
                }
            }

            for record in &new_records {
                let is_pending = record.s == 0
                    || ((record.is_websocket() || record.is_sse() || record.is_tunnel())
                        && record.ss.as_ref().map(|s| s.is_open).unwrap_or(false));
                if is_pending {
                    new_pending_ids.insert(record.id.clone());
                }
            }

            let msg = PushMessage::TrafficDelta(TrafficDeltaData {
                inserts: new_records,
                updates: updated_records,
                has_more: result.has_more,
                server_total: result.total,
                server_sequence: result.server_sequence,
            });

            if client.send(msg) {
                let mut sub = client.subscription.write();
                if let Some(seq) = last_seq {
                    sub.last_sequence = Some(seq);
                }
                sub.pending_ids = new_pending_ids.into_iter().collect();
            }
        }
    }

    fn send_initial_traffic_legacy(
        &self,
        client: &Arc<PushClient>,
        subscription: &ClientSubscription,
    ) {
        let (new_records, has_more) = if let Some(ref traffic_store) = self.state.traffic_store {
            traffic_store.get_after(
                subscription.last_traffic_id.as_deref(),
                &Default::default(),
                500,
            )
        } else {
            self.state.traffic_recorder.get_after(
                subscription.last_traffic_id.as_deref(),
                &Default::default(),
                500,
            )
        };

        let new_records: Vec<_> = new_records
            .into_iter()
            .map(|s| self.enrich_summary(s))
            .collect();

        let updated_records = if !subscription.pending_ids.is_empty() {
            let ids: Vec<&str> = subscription
                .pending_ids
                .iter()
                .map(|s| s.as_str())
                .collect();
            let summaries = if let Some(ref traffic_store) = self.state.traffic_store {
                traffic_store.get_by_ids(&ids)
            } else {
                self.state.traffic_recorder.get_by_ids(&ids)
            };
            summaries
                .into_iter()
                .map(|s| self.enrich_summary(s))
                .collect()
        } else {
            Vec::new()
        };

        let server_total = if let Some(ref traffic_store) = self.state.traffic_store {
            traffic_store.total()
        } else {
            self.state.traffic_recorder.total()
        };

        if !new_records.is_empty() || !updated_records.is_empty() {
            let last_id = new_records.last().map(|r| r.id.clone());

            let mut new_pending_ids: HashSet<String> =
                subscription.pending_ids.iter().cloned().collect();

            for record in &updated_records {
                let is_pending = record.status == 0
                    || ((record.is_websocket || record.is_sse || record.is_tunnel)
                        && record
                            .socket_status
                            .as_ref()
                            .map(|s| s.is_open)
                            .unwrap_or(false));
                if !is_pending {
                    new_pending_ids.remove(&record.id);
                }
            }

            for record in &new_records {
                let is_pending = record.status == 0
                    || ((record.is_websocket || record.is_sse || record.is_tunnel)
                        && record
                            .socket_status
                            .as_ref()
                            .map(|s| s.is_open)
                            .unwrap_or(false));
                if is_pending {
                    new_pending_ids.insert(record.id.clone());
                }
            }

            let msg = PushMessage::TrafficUpdates(TrafficUpdatesData {
                new_records,
                updated_records,
                has_more,
                server_total,
            });

            if client.send(msg) {
                let mut sub = client.subscription.write();
                if let Some(id) = last_id {
                    sub.last_traffic_id = Some(id);
                }
                sub.pending_ids = new_pending_ids.into_iter().collect();
            }
        }
    }

    async fn broadcast_traffic_updates_legacy(&self) {
        let mut clients_to_remove = Vec::new();

        for client_ref in self.clients.iter() {
            let client = client_ref.value();
            let subscription = client.get_subscription();

            let (new_records, has_more) = if let Some(ref traffic_store) = self.state.traffic_store
            {
                traffic_store.get_after(
                    subscription.last_traffic_id.as_deref(),
                    &Default::default(),
                    500,
                )
            } else {
                self.state.traffic_recorder.get_after(
                    subscription.last_traffic_id.as_deref(),
                    &Default::default(),
                    500,
                )
            };

            let new_records: Vec<_> = new_records
                .into_iter()
                .map(|s| self.enrich_summary(s))
                .collect();

            let updated_records = if !subscription.pending_ids.is_empty() {
                let ids: Vec<&str> = subscription
                    .pending_ids
                    .iter()
                    .map(|s| s.as_str())
                    .collect();
                let summaries = if let Some(ref traffic_store) = self.state.traffic_store {
                    traffic_store.get_by_ids(&ids)
                } else {
                    self.state.traffic_recorder.get_by_ids(&ids)
                };
                summaries
                    .into_iter()
                    .map(|s| self.enrich_summary(s))
                    .collect()
            } else {
                Vec::new()
            };

            let server_total = if let Some(ref traffic_store) = self.state.traffic_store {
                traffic_store.total()
            } else {
                self.state.traffic_recorder.total()
            };

            if !new_records.is_empty() || !updated_records.is_empty() {
                let last_id = new_records.last().map(|r| r.id.clone());

                let mut new_pending_ids: HashSet<String> =
                    subscription.pending_ids.iter().cloned().collect();

                for record in &updated_records {
                    let is_pending = record.status == 0
                        || ((record.is_websocket || record.is_sse || record.is_tunnel)
                            && record
                                .socket_status
                                .as_ref()
                                .map(|s| s.is_open)
                                .unwrap_or(false));
                    if !is_pending {
                        new_pending_ids.remove(&record.id);
                    }
                }

                for record in &new_records {
                    let is_pending = record.status == 0
                        || ((record.is_websocket || record.is_sse || record.is_tunnel)
                            && record
                                .socket_status
                                .as_ref()
                                .map(|s| s.is_open)
                                .unwrap_or(false));
                    if is_pending {
                        new_pending_ids.insert(record.id.clone());
                    }
                }

                let msg = PushMessage::TrafficUpdates(TrafficUpdatesData {
                    new_records,
                    updated_records,
                    has_more,
                    server_total,
                });

                if !client.send(msg) {
                    clients_to_remove.push(client.id);
                } else {
                    let mut sub = client.subscription.write();
                    if let Some(id) = last_id {
                        sub.last_traffic_id = Some(id);
                    }
                    sub.pending_ids = new_pending_ids.into_iter().collect();
                }
            }
        }

        for client_id in clients_to_remove {
            self.unregister_client(client_id);
        }
    }

    pub async fn broadcast_overview(&self) {
        let mut clients_to_remove = Vec::new();

        let system_info = crate::metrics::SystemInfo::new(self.state.start_time);
        let metrics = self.state.metrics_collector.get_current();
        let traffic_count = self.state.traffic_recorder.count();

        let (rules_total, rules_enabled) = match self.state.rules_storage.load_all() {
            Ok(rules) => {
                let enabled = rules.iter().filter(|r| r.enabled).count();
                (rules.len(), enabled)
            }
            Err(_) => (0, 0),
        };

        let pending_count = if let Some(ref access_control) = self.state.access_control {
            let ac = access_control.read().await;
            ac.pending_authorization_count()
        } else {
            0
        };

        let overview = OverviewData {
            system: serde_json::to_value(&system_info).unwrap_or_default(),
            metrics: serde_json::to_value(&metrics).unwrap_or_default(),
            rules: RulesInfo {
                total: rules_total,
                enabled: rules_enabled,
            },
            traffic: TrafficInfo {
                recorded: traffic_count,
            },
            server: ServerInfo {
                port: self.state.port,
                admin_url: format!("http://127.0.0.1:{}/_bifrost/", self.state.port),
            },
            pending_authorizations: pending_count,
        };

        for client_ref in self.clients.iter() {
            let client = client_ref.value();
            let subscription = client.get_subscription();

            if subscription.need_overview {
                let msg = PushMessage::OverviewUpdate(overview.clone());
                if !client.send(msg) {
                    clients_to_remove.push(client.id);
                }
            }
        }

        for client_id in clients_to_remove {
            self.unregister_client(client_id);
        }
    }

    pub async fn broadcast_metrics(&self) {
        self.broadcast_metrics_with_interval(0).await;
    }

    pub async fn broadcast_metrics_with_interval(&self, elapsed_ms: u64) {
        let mut clients_to_remove = Vec::new();

        let metrics = self.state.metrics_collector.get_current();
        let metrics_data = MetricsData {
            metrics: serde_json::to_value(&metrics).unwrap_or_default(),
        };

        for client_ref in self.clients.iter() {
            let client = client_ref.value();
            let subscription = client.get_subscription();

            if subscription.need_metrics {
                let client_interval = subscription
                    .metrics_interval_ms
                    .clamp(METRICS_INTERVAL_MIN_MS, METRICS_INTERVAL_MAX_MS);

                let should_send = elapsed_ms == 0 || elapsed_ms % client_interval < 500;

                if should_send {
                    let msg = PushMessage::MetricsUpdate(metrics_data.clone());
                    if !client.send(msg) {
                        clients_to_remove.push(client.id);
                    }
                }
            }
        }

        for client_id in clients_to_remove {
            self.unregister_client(client_id);
        }
    }

    pub async fn broadcast_history(&self) {
        let mut clients_to_remove = Vec::new();

        for client_ref in self.clients.iter() {
            let client = client_ref.value();
            let subscription = client.get_subscription();

            if subscription.need_history {
                let history = self
                    .state
                    .metrics_collector
                    .get_history(Some(subscription.history_limit));
                let history_json: Vec<serde_json::Value> = history
                    .into_iter()
                    .map(|m| serde_json::to_value(&m).unwrap_or_default())
                    .collect();

                let msg = PushMessage::HistoryUpdate(HistoryData {
                    history: history_json,
                });

                if !client.send(msg) {
                    clients_to_remove.push(client.id);
                }
            }
        }

        for client_id in clients_to_remove {
            self.unregister_client(client_id);
        }
    }

    pub async fn send_initial_data(&self, client: &Arc<PushClient>) {
        let subscription = client.get_subscription();

        if let Some(ref db_store) = self.state.traffic_db_store {
            self.send_initial_traffic_delta(client, db_store, &subscription);
        } else {
            self.send_initial_traffic_legacy(client, &subscription);
        }

        if subscription.need_overview {
            let system_info = crate::metrics::SystemInfo::new(self.state.start_time);
            let metrics = self.state.metrics_collector.get_current();
            let traffic_count = self.state.traffic_recorder.count();

            let (rules_total, rules_enabled) = match self.state.rules_storage.load_all() {
                Ok(rules) => {
                    let enabled = rules.iter().filter(|r| r.enabled).count();
                    (rules.len(), enabled)
                }
                Err(_) => (0, 0),
            };

            let pending_count = if let Some(ref access_control) = self.state.access_control {
                let ac = access_control.read().await;
                ac.pending_authorization_count()
            } else {
                0
            };

            let overview = OverviewData {
                system: serde_json::to_value(&system_info).unwrap_or_default(),
                metrics: serde_json::to_value(&metrics).unwrap_or_default(),
                rules: RulesInfo {
                    total: rules_total,
                    enabled: rules_enabled,
                },
                traffic: TrafficInfo {
                    recorded: traffic_count,
                },
                server: ServerInfo {
                    port: self.state.port,
                    admin_url: format!("http://127.0.0.1:{}/_bifrost/", self.state.port),
                },
                pending_authorizations: pending_count,
            };

            client.send(PushMessage::OverviewUpdate(overview));
        }

        if subscription.need_history {
            let history = self
                .state
                .metrics_collector
                .get_history(Some(subscription.history_limit));
            let history_json: Vec<serde_json::Value> = history
                .into_iter()
                .map(|m| serde_json::to_value(&m).unwrap_or_default())
                .collect();

            client.send(PushMessage::HistoryUpdate(HistoryData {
                history: history_json,
            }));
        }

        if subscription.need_metrics {
            let metrics = self.state.metrics_collector.get_current();
            client.send(PushMessage::MetricsUpdate(MetricsData {
                metrics: serde_json::to_value(&metrics).unwrap_or_default(),
            }));
        }
    }

    pub fn broadcast_replay_request_updated(
        &self,
        action: &str,
        request_id: Option<&str>,
        group_id: Option<&str>,
    ) {
        let msg = PushMessage::ReplayRequestUpdated(ReplayRequestUpdatedData {
            action: action.to_string(),
            request_id: request_id.map(|s| s.to_string()),
            group_id: group_id.map(|s| s.to_string()),
        });

        let mut clients_to_remove = Vec::new();
        for client_ref in self.clients.iter() {
            let client = client_ref.value();
            if !client.send(msg.clone()) {
                clients_to_remove.push(client.id);
            }
        }

        for client_id in clients_to_remove {
            self.unregister_client(client_id);
        }
    }

    pub fn broadcast_traffic_deleted(&self, ids: Vec<String>) {
        if ids.is_empty() {
            return;
        }
        let msg = PushMessage::TrafficDeleted(TrafficDeletedData { ids });
        let mut clients_to_remove = Vec::new();
        for client_ref in self.clients.iter() {
            let client = client_ref.value();
            if !client.send(msg.clone()) {
                clients_to_remove.push(client.id);
            }
        }

        for client_id in clients_to_remove {
            self.unregister_client(client_id);
        }
    }

    pub fn broadcast_replay_history_updated(
        &self,
        action: &str,
        request_id: &str,
        history_id: Option<&str>,
    ) {
        let msg = PushMessage::ReplayHistoryUpdated(ReplayHistoryUpdatedData {
            action: action.to_string(),
            request_id: request_id.to_string(),
            history_id: history_id.map(|s| s.to_string()),
        });

        let mut clients_to_remove = Vec::new();
        for client_ref in self.clients.iter() {
            let client = client_ref.value();
            if !client.send(msg.clone()) {
                clients_to_remove.push(client.id);
            }
        }

        for client_id in clients_to_remove {
            self.unregister_client(client_id);
        }
    }
}

pub type SharedPushManager = Arc<PushManager>;

pub fn start_push_tasks(manager: SharedPushManager) -> Vec<tokio::task::JoinHandle<()>> {
    let mut handles = Vec::new();

    let manager_traffic = manager.clone();
    handles.push(tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        loop {
            interval.tick().await;
            if manager_traffic.client_count() > 0 {
                manager_traffic.broadcast_traffic_updates().await;
            }
        }
    }));

    let manager_overview = manager.clone();
    handles.push(tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            if manager_overview.client_count() > 0 {
                manager_overview.broadcast_overview().await;
            }
        }
    }));

    let manager_metrics = manager.clone();
    handles.push(tokio::spawn(async move {
        let base_interval_ms: u64 = 500;
        let mut interval = tokio::time::interval(Duration::from_millis(base_interval_ms));
        let mut tick_count: u64 = 0;
        loop {
            interval.tick().await;
            tick_count = tick_count.wrapping_add(1);
            if manager_metrics.client_count() > 0 {
                let elapsed_ms = tick_count * base_interval_ms;
                manager_metrics
                    .broadcast_metrics_with_interval(elapsed_ms)
                    .await;
            }
        }
    }));

    let manager_history = manager.clone();
    handles.push(tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if manager_history.client_count() > 0 {
                manager_history.broadcast_history().await;
            }
        }
    }));

    handles
}
