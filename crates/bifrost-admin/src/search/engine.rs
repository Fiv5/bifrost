use regex::Regex;
use tracing::debug;

use super::types::{
    FilterCondition, MatchLocation, SearchFilters, SearchRequest, SearchResponse, SearchResultItem,
    SearchScope,
};
use crate::body_store::{BodyRef, SharedBodyStore};
use crate::connection_monitor::SharedConnectionMonitor;
use crate::frame_store::SharedFrameStore;
use crate::traffic::TrafficRecord;
use crate::traffic_db::{QueryParams, SharedTrafficDbStore, TrafficSummaryCompact};

const MAX_PREVIEW_CONTEXT: usize = 50;
const DEFAULT_BATCH_SIZE: usize = 50;
const MAX_SEARCH_CANDIDATES: usize = 500;

pub struct SearchEngine {
    traffic_db: SharedTrafficDbStore,
    body_store: Option<SharedBodyStore>,
    frame_store: Option<SharedFrameStore>,
    connection_monitor: Option<SharedConnectionMonitor>,
}

impl SearchEngine {
    pub fn new(traffic_db: SharedTrafficDbStore, body_store: Option<SharedBodyStore>) -> Self {
        Self {
            traffic_db,
            body_store,
            frame_store: None,
            connection_monitor: None,
        }
    }

    pub fn with_frame_support(
        traffic_db: SharedTrafficDbStore,
        body_store: Option<SharedBodyStore>,
        frame_store: Option<SharedFrameStore>,
        connection_monitor: Option<SharedConnectionMonitor>,
    ) -> Self {
        Self {
            traffic_db,
            body_store,
            frame_store,
            connection_monitor,
        }
    }

    pub fn search(&self, request: &SearchRequest) -> SearchResponse {
        let search_id = generate_search_id();
        let batch_size = request.limit.unwrap_or(DEFAULT_BATCH_SIZE);
        let keyword_lower = request.keyword.to_lowercase();

        debug!(
            keyword = %request.keyword,
            scope = ?request.scope,
            cursor = ?request.cursor,
            limit = batch_size,
            "[SEARCH] Starting search"
        );

        let query_params = self.build_query_params(request);
        let query_result = self.traffic_db.query(&query_params);

        debug!(
            candidates = query_result.records.len(),
            total = query_result.total,
            "[SEARCH] Got candidate records from database"
        );

        let mut results = Vec::new();
        let mut searched = 0;
        let mut last_seq: Option<u64> = None;

        for compact in query_result.records.iter().take(MAX_SEARCH_CANDIDATES) {
            searched += 1;
            last_seq = Some(compact.seq);

            if let Some(record) = self.traffic_db.get_by_id(&compact.id) {
                if let Some(result) =
                    self.search_record(&request.scope, &keyword_lower, &record, compact)
                {
                    results.push(result);
                    if results.len() >= batch_size {
                        break;
                    }
                }
            }
        }

        let has_more = searched < query_result.records.len() || query_result.has_more;
        let total_matched = results.len();

        debug!(
            searched = searched,
            matched = total_matched,
            has_more = has_more,
            "[SEARCH] Search completed"
        );

        SearchResponse {
            results,
            total_searched: searched,
            total_matched,
            next_cursor: last_seq,
            has_more,
            search_id,
        }
    }

    fn build_query_params(&self, request: &SearchRequest) -> QueryParams {
        let mut params = QueryParams {
            cursor: request.cursor,
            limit: Some(MAX_SEARCH_CANDIDATES),
            direction: crate::traffic_db::Direction::Backward,
            ..Default::default()
        };

        let filters = &request.filters;

        if let Some(rule_hit) = filters.has_rule_hit {
            params.has_rule_hit = Some(rule_hit);
        }

        for protocol in &filters.protocols {
            match protocol.to_uppercase().as_str() {
                "WS" | "WSS" => params.is_websocket = Some(true),
                "H3" | "H3S" => params.is_h3 = Some(true),
                _ => {}
            }
        }

        for condition in &filters.conditions {
            match condition.field.as_str() {
                "host" => {
                    if condition.operator == "contains" || condition.operator == "equals" {
                        params.host_contains = Some(condition.value.clone());
                    }
                }
                "path" => {
                    if condition.operator == "contains" || condition.operator == "equals" {
                        params.path_contains = Some(condition.value.clone());
                    }
                }
                "url" => {
                    if condition.operator == "contains" || condition.operator == "equals" {
                        params.url_contains = Some(condition.value.clone());
                    }
                }
                "method" => {
                    if condition.operator == "equals" {
                        params.method = Some(condition.value.clone());
                    }
                }
                "client_app" => {
                    params.client_app = Some(condition.value.clone());
                }
                "client_ip" => {
                    params.client_ip = Some(condition.value.clone());
                }
                "content_type" => {
                    params.content_type = Some(condition.value.clone());
                }
                _ => {}
            }
        }

        if !filters.client_ips.is_empty() {
            params.client_ip = Some(filters.client_ips[0].clone());
        }

        if !filters.client_apps.is_empty() {
            params.client_app = Some(filters.client_apps[0].clone());
        }

        if !filters.domains.is_empty() {
            params.host_contains = Some(filters.domains[0].clone());
        }

        params
    }

    fn search_record(
        &self,
        scope: &SearchScope,
        keyword: &str,
        record: &TrafficRecord,
        compact: &TrafficSummaryCompact,
    ) -> Option<SearchResultItem> {
        let mut matches = Vec::new();

        if scope.should_search_url() {
            if let Some(m) = self.search_text(&record.url, keyword, "url") {
                matches.push(m);
            }
        }

        if scope.should_search_request_headers() {
            if let Some(headers) = &record.request_headers {
                for (k, v) in headers {
                    let header_text = format!("{}: {}", k, v);
                    if let Some(m) = self.search_text(&header_text, keyword, "request_header") {
                        matches.push(m);
                        break;
                    }
                }
            }
        }

        if scope.should_search_response_headers() {
            if let Some(headers) = &record.response_headers {
                for (k, v) in headers {
                    let header_text = format!("{}: {}", k, v);
                    if let Some(m) = self.search_text(&header_text, keyword, "response_header") {
                        matches.push(m);
                        break;
                    }
                }
            }
        }

        if scope.should_search_request_body() {
            if let Some(body_ref) = &record.request_body_ref {
                if let Some(m) = self.search_body(body_ref, keyword, "request_body") {
                    matches.push(m);
                }
            }
        }

        if scope.should_search_response_body() {
            if let Some(body_ref) = &record.response_body_ref {
                if let Some(m) = self.search_body(body_ref, keyword, "response_body") {
                    matches.push(m);
                }
            }
        }

        if record.is_websocket && scope.should_search_websocket_messages() {
            if let Some(frame_matches) =
                self.search_frames(&record.id, keyword, "websocket_message")
            {
                matches.extend(frame_matches);
            }
        }

        if record.is_sse && scope.should_search_sse_events() {
            if let Some(frame_matches) = self.search_frames(&record.id, keyword, "sse_event") {
                matches.extend(frame_matches);
            }
        }

        if matches.is_empty() {
            None
        } else {
            Some(SearchResultItem {
                record: compact.clone(),
                matches,
            })
        }
    }

    fn search_text(&self, text: &str, keyword: &str, field: &str) -> Option<MatchLocation> {
        let text_lower = text.to_lowercase();
        if let Some(pos) = text_lower.find(keyword) {
            let start = find_char_boundary(text, pos.saturating_sub(MAX_PREVIEW_CONTEXT), false);
            let end = find_char_boundary(
                text,
                (pos + keyword.len() + MAX_PREVIEW_CONTEXT).min(text.len()),
                true,
            );

            let preview = if start > 0 || end < text.len() {
                let prefix = if start > 0 { "..." } else { "" };
                let suffix = if end < text.len() { "..." } else { "" };
                format!("{}{}{}", prefix, &text[start..end], suffix)
            } else {
                text[start..end].to_string()
            };

            Some(MatchLocation {
                field: field.to_string(),
                preview,
                offset: pos,
            })
        } else {
            None
        }
    }

    fn search_body(&self, body_ref: &BodyRef, keyword: &str, field: &str) -> Option<MatchLocation> {
        match body_ref {
            BodyRef::Inline { data } => self.search_text(data, keyword, field),
            BodyRef::File { .. } => {
                if let Some(ref body_store) = self.body_store {
                    let store = body_store.read();
                    if let Some(content) = store.load(body_ref) {
                        return self.search_text(&content, keyword, field);
                    }
                }
                None
            }
        }
    }

    fn search_frames(
        &self,
        connection_id: &str,
        keyword: &str,
        field: &str,
    ) -> Option<Vec<MatchLocation>> {
        use std::collections::HashSet;

        let mut matches = Vec::new();
        let mut seen_frame_ids: HashSet<u64> = HashSet::new();

        if let Some(ref monitor) = self.connection_monitor {
            if let Some((frames, _)) = monitor.get_frames(connection_id, None, usize::MAX) {
                for frame in frames {
                    if seen_frame_ids.contains(&frame.frame_id) {
                        continue;
                    }
                    seen_frame_ids.insert(frame.frame_id);

                    if let Some(preview) = &frame.payload_preview {
                        if let Some(m) = self.search_text(preview, keyword, field) {
                            matches.push(m);
                            break;
                        }
                    }

                    if let Some(body_ref) = &frame.payload_ref {
                        if let Some(m) = self.search_body(body_ref, keyword, field) {
                            matches.push(m);
                            break;
                        }
                    }
                }
            }
        }

        if matches.is_empty() {
            if let Some(ref fs) = self.frame_store {
                if let Ok(frames) = fs.load_all_frames(connection_id) {
                    for frame in frames {
                        if seen_frame_ids.contains(&frame.frame_id) {
                            continue;
                        }
                        seen_frame_ids.insert(frame.frame_id);

                        if let Some(preview) = &frame.payload_preview {
                            if let Some(m) = self.search_text(preview, keyword, field) {
                                matches.push(m);
                                break;
                            }
                        }

                        if let Some(body_ref) = &frame.payload_ref {
                            if let Some(m) = self.search_body(body_ref, keyword, field) {
                                matches.push(m);
                                break;
                            }
                        }
                    }
                }
            }
        }

        if matches.is_empty() {
            None
        } else {
            Some(matches)
        }
    }

    pub fn matches_filter(&self, record: &TrafficRecord, filters: &SearchFilters) -> bool {
        if let Some(rule_hit) = filters.has_rule_hit {
            if record.has_rule_hit != rule_hit {
                return false;
            }
        }

        if !filters.protocols.is_empty() {
            let protocol_upper = record.protocol.to_uppercase();
            let mut matched = false;

            for p in &filters.protocols {
                match p.to_uppercase().as_str() {
                    "HTTP" => {
                        if protocol_upper == "HTTP" || protocol_upper.starts_with("HTTP/1") {
                            matched = true;
                            break;
                        }
                    }
                    "HTTPS" => {
                        if protocol_upper == "HTTPS" {
                            matched = true;
                            break;
                        }
                    }
                    "WS" => {
                        if record.is_websocket && protocol_upper == "WS" {
                            matched = true;
                            break;
                        }
                    }
                    "WSS" => {
                        if record.is_websocket && protocol_upper == "WSS" {
                            matched = true;
                            break;
                        }
                    }
                    "H3" | "H3S" => {
                        if record.is_h3 {
                            matched = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if !matched {
                return false;
            }
        }

        if !filters.status_ranges.is_empty() {
            let status = record.status;
            let mut matched = false;

            for range in &filters.status_ranges {
                match range.as_str() {
                    "error" => {
                        if status == 0 || status >= 500 {
                            matched = true;
                            break;
                        }
                    }
                    "1xx" => {
                        if (100..200).contains(&status) {
                            matched = true;
                            break;
                        }
                    }
                    "2xx" => {
                        if (200..300).contains(&status) {
                            matched = true;
                            break;
                        }
                    }
                    "3xx" => {
                        if (300..400).contains(&status) {
                            matched = true;
                            break;
                        }
                    }
                    "4xx" => {
                        if (400..500).contains(&status) {
                            matched = true;
                            break;
                        }
                    }
                    "5xx" => {
                        if (500..600).contains(&status) {
                            matched = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if !matched {
                return false;
            }
        }

        if !filters.content_types.is_empty() {
            let content_type = record.content_type.as_deref().unwrap_or("").to_lowercase();
            let mut matched = false;

            for ct in &filters.content_types {
                let ct_lower = ct.to_lowercase();
                let patterns = match ct_lower.as_str() {
                    "json" => vec!["application/json", "text/json"],
                    "form" => vec!["application/x-www-form-urlencoded", "multipart/form-data"],
                    "xml" => vec!["application/xml", "text/xml"],
                    "js" => vec!["application/javascript", "text/javascript"],
                    "css" => vec!["text/css"],
                    "font" => vec!["font/", "application/font"],
                    "doc" => vec!["text/html", "application/xhtml"],
                    "media" => vec!["image/", "video/", "audio/"],
                    _ => vec![ct_lower.as_str()],
                };

                for pattern in patterns {
                    if content_type.contains(pattern) {
                        matched = true;
                        break;
                    }
                }

                if matched {
                    break;
                }
            }

            if !matched {
                return false;
            }
        }

        if !filters.client_ips.is_empty() && !filters.client_ips.contains(&record.client_ip) {
            return false;
        }

        if !filters.client_apps.is_empty() {
            match &record.client_app {
                Some(app) if filters.client_apps.contains(app) => {}
                _ => return false,
            }
        }

        if !filters.domains.is_empty() {
            let host = &record.host;
            if !filters.domains.iter().any(|d| host.contains(d)) {
                return false;
            }
        }

        for condition in &filters.conditions {
            if !self.matches_condition(record, condition) {
                return false;
            }
        }

        true
    }

    fn matches_condition(&self, record: &TrafficRecord, condition: &FilterCondition) -> bool {
        let field_value = match condition.field.as_str() {
            "url" => &record.url,
            "host" => &record.host,
            "path" => &record.path,
            "method" => &record.method,
            "content_type" => record.content_type.as_deref().unwrap_or(""),
            "client_app" => record.client_app.as_deref().unwrap_or(""),
            "client_ip" => &record.client_ip,
            _ => return true,
        };

        let field_lower = field_value.to_lowercase();
        let value_lower = condition.value.to_lowercase();

        match condition.operator.as_str() {
            "contains" => field_lower.contains(&value_lower),
            "equals" => field_lower == value_lower,
            "not_contains" => !field_lower.contains(&value_lower),
            "regex" => {
                if let Ok(re) = Regex::new(&condition.value) {
                    re.is_match(field_value)
                } else {
                    false
                }
            }
            _ => field_lower.contains(&value_lower),
        }
    }
}

fn generate_search_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("search_{}", timestamp)
}

fn find_char_boundary(s: &str, byte_index: usize, search_forward: bool) -> usize {
    if byte_index >= s.len() {
        return s.len();
    }

    if s.is_char_boundary(byte_index) {
        return byte_index;
    }

    if search_forward {
        for i in byte_index..s.len() {
            if s.is_char_boundary(i) {
                return i;
            }
        }
        s.len()
    } else {
        for i in (0..byte_index).rev() {
            if s.is_char_boundary(i) {
                return i;
            }
        }
        0
    }
}
