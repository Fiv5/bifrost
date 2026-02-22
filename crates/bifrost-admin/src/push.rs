use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::info;

use crate::state::SharedAdminState;
use crate::traffic::TrafficSummary;

static CLIENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn generate_client_id() -> u64 {
    CLIENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PushMessage {
    #[serde(rename = "traffic_updates")]
    TrafficUpdates(TrafficUpdatesData),

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficUpdatesData {
    pub new_records: Vec<TrafficSummary>,
    pub updated_records: Vec<TrafficSummary>,
    pub has_more: bool,
    pub server_total: usize,
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
pub struct ConnectedData {
    pub client_id: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientSubscription {
    #[serde(default)]
    pub last_traffic_id: Option<String>,
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
}

fn default_history_limit() -> usize {
    60
}

impl Default for ClientSubscription {
    fn default() -> Self {
        Self {
            last_traffic_id: None,
            pending_ids: Vec::new(),
            need_overview: false,
            need_metrics: false,
            need_history: false,
            history_limit: default_history_limit(),
        }
    }
}

pub struct PushClient {
    pub id: u64,
    pub sender: mpsc::UnboundedSender<PushMessage>,
    pub subscription: RwLock<ClientSubscription>,
}

impl PushClient {
    pub fn new(subscription: ClientSubscription) -> (Self, mpsc::UnboundedReceiver<PushMessage>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let client = Self {
            id: generate_client_id(),
            sender,
            subscription: RwLock::new(subscription),
        };
        (client, receiver)
    }

    pub fn send(&self, msg: PushMessage) -> bool {
        self.sender.send(msg).is_ok()
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
    state: SharedAdminState,
}

impl PushManager {
    pub fn new(state: SharedAdminState) -> Self {
        Self {
            clients: DashMap::new(),
            state,
        }
    }

    pub fn register_client(
        &self,
        subscription: ClientSubscription,
    ) -> (Arc<PushClient>, mpsc::UnboundedReceiver<PushMessage>) {
        let (client, receiver) = PushClient::new(subscription);
        let client = Arc::new(client);
        let client_id = client.id;
        self.clients.insert(client_id, client.clone());
        info!(client_id = client_id, "Push client registered");
        (client, receiver)
    }

    pub fn unregister_client(&self, client_id: u64) {
        if self.clients.remove(&client_id).is_some() {
            info!(client_id = client_id, "Push client unregistered");
        }
    }

    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    fn enrich_summary(&self, mut summary: TrafficSummary) -> TrafficSummary {
        if summary.is_sse || summary.is_websocket || summary.is_tunnel {
            if let Some(status) = self
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
        }
        summary
    }

    pub async fn broadcast_traffic_updates(&self) {
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
        let mut clients_to_remove = Vec::new();

        let metrics = self.state.metrics_collector.get_current();
        let metrics_data = MetricsData {
            metrics: serde_json::to_value(&metrics).unwrap_or_default(),
        };

        for client_ref in self.clients.iter() {
            let client = client_ref.value();
            let subscription = client.get_subscription();

            if subscription.need_metrics {
                let msg = PushMessage::MetricsUpdate(metrics_data.clone());
                if !client.send(msg) {
                    clients_to_remove.push(client.id);
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
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            if manager_metrics.client_count() > 0 {
                manager_metrics.broadcast_metrics().await;
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
