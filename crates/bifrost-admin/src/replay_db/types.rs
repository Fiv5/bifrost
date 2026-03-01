use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleMode {
    #[default]
    Enabled,
    Selected,
    None,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleConfig {
    pub mode: RuleMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_rules: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyType {
    #[default]
    None,
    FormData,
    XWwwFormUrlencoded,
    Raw,
    Binary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawType {
    Json,
    Xml,
    #[default]
    Text,
    Javascript,
    Html,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyValueItem {
    pub id: String,
    pub key: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplayBody {
    #[serde(rename = "type")]
    pub body_type: BodyType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_type: Option<RawType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub form_data: Vec<KeyValueItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayGroup {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub sort_order: i32,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRequest {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValueItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<ReplayBody>,
    pub is_saved: bool,
    pub sort_order: i32,
    pub created_at: u64,
    pub updated_at: u64,
}

impl ReplayRequest {
    pub fn new(method: String, url: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            group_id: None,
            name: None,
            method,
            url,
            headers: Vec::new(),
            body: None,
            is_saved: false,
            sort_order: 0,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayHistory {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub traffic_id: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub duration_ms: u64,
    pub executed_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_config: Option<RuleConfig>,
}

impl ReplayHistory {
    pub fn new(
        request_id: Option<String>,
        traffic_id: String,
        method: String,
        url: String,
        status: u16,
        duration_ms: u64,
        rule_config: Option<RuleConfig>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            request_id,
            traffic_id,
            method,
            url,
            status,
            duration_ms,
            executed_at: chrono::Utc::now().timestamp_millis() as u64,
            rule_config,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayDbStats {
    pub request_count: usize,
    pub history_count: usize,
    pub group_count: usize,
    pub db_size: u64,
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRequestSummary {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub method: String,
    pub url: String,
    pub is_saved: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

impl From<&ReplayRequest> for ReplayRequestSummary {
    fn from(req: &ReplayRequest) -> Self {
        Self {
            id: req.id.clone(),
            group_id: req.group_id.clone(),
            name: req.name.clone(),
            method: req.method.clone(),
            url: req.url.clone(),
            is_saved: req.is_saved,
            created_at: req.created_at,
            updated_at: req.updated_at,
        }
    }
}

pub const MAX_REQUESTS: usize = 1000;
pub const MAX_HISTORY: usize = 10000;
pub const MAX_CONCURRENT_REPLAYS: usize = 100;
