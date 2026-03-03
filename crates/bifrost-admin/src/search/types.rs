use serde::{Deserialize, Serialize};

use crate::traffic_db::TrafficSummaryCompact;

#[derive(Debug, Clone, Deserialize)]
pub struct SearchRequest {
    pub keyword: String,

    #[serde(default)]
    pub scope: SearchScope,

    #[serde(default)]
    pub filters: SearchFilters,

    pub cursor: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchScope {
    #[serde(default)]
    pub request_body: bool,
    #[serde(default)]
    pub response_body: bool,
    #[serde(default)]
    pub request_headers: bool,
    #[serde(default)]
    pub response_headers: bool,
    #[serde(default)]
    pub url: bool,
    #[serde(default)]
    pub websocket_messages: bool,
    #[serde(default)]
    pub sse_events: bool,
    #[serde(default = "default_true")]
    pub all: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SearchScope {
    fn default() -> Self {
        Self {
            request_body: false,
            response_body: false,
            request_headers: false,
            response_headers: false,
            url: false,
            websocket_messages: false,
            sse_events: false,
            all: true,
        }
    }
}

impl SearchScope {
    pub fn should_search_url(&self) -> bool {
        self.all || self.url
    }

    pub fn should_search_request_headers(&self) -> bool {
        self.all || self.request_headers
    }

    pub fn should_search_response_headers(&self) -> bool {
        self.all || self.response_headers
    }

    pub fn should_search_request_body(&self) -> bool {
        self.all || self.request_body
    }

    pub fn should_search_response_body(&self) -> bool {
        self.all || self.response_body
    }

    pub fn should_search_websocket_messages(&self) -> bool {
        self.all || self.websocket_messages
    }

    pub fn should_search_sse_events(&self) -> bool {
        self.all || self.sse_events
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SearchFilters {
    #[serde(default)]
    pub protocols: Vec<String>,
    #[serde(default)]
    pub status_ranges: Vec<String>,
    #[serde(default)]
    pub content_types: Vec<String>,
    pub has_rule_hit: Option<bool>,

    #[serde(default)]
    pub conditions: Vec<FilterCondition>,

    #[serde(default)]
    pub client_ips: Vec<String>,
    #[serde(default)]
    pub client_apps: Vec<String>,
    #[serde(default)]
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FilterCondition {
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total_searched: usize,
    pub total_matched: usize,
    pub next_cursor: Option<u64>,
    pub has_more: bool,
    pub search_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResultItem {
    pub record: TrafficSummaryCompact,
    pub matches: Vec<MatchLocation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchLocation {
    pub field: String,
    pub preview: String,
    pub offset: usize,
}
