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
const SEARCH_BATCH_SIZE: usize = 200;
const MAX_SEARCH_ITERATIONS: usize = 50;
const MAX_TOTAL_SEARCHED: usize = 10000;

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
            "[SEARCH] Starting iterative search"
        );

        let mut results = Vec::new();
        let mut total_searched = 0;
        let mut current_cursor = request.cursor;
        let mut iterations = 0;
        let mut db_has_more = true;

        while results.len() < batch_size
            && iterations < MAX_SEARCH_ITERATIONS
            && total_searched < MAX_TOTAL_SEARCHED
            && db_has_more
        {
            iterations += 1;

            let query_params = self.build_query_params_with_cursor(request, current_cursor);
            let query_result = self.traffic_db.query(&query_params);

            if query_result.records.is_empty() {
                db_has_more = false;
                break;
            }

            debug!(
                iteration = iterations,
                candidates = query_result.records.len(),
                current_results = results.len(),
                total_searched = total_searched,
                "[SEARCH] Processing batch"
            );

            for compact in &query_result.records {
                total_searched += 1;
                current_cursor = Some(compact.seq);

                if !self.matches_filter_compact(compact, &request.filters) {
                    continue;
                }

                if let Some(record) = self.traffic_db.get_by_id(&compact.id) {
                    if !request.filters.conditions.is_empty()
                        && !self.matches_conditions(&record, &request.filters.conditions)
                    {
                        continue;
                    }

                    if let Some(result) =
                        self.search_record(&request.scope, &keyword_lower, &record, compact)
                    {
                        results.push(result);
                        if results.len() >= batch_size {
                            break;
                        }
                    }
                }

                if total_searched >= MAX_TOTAL_SEARCHED {
                    break;
                }
            }

            db_has_more = query_result.has_more;
        }

        let has_more = db_has_more && total_searched < MAX_TOTAL_SEARCHED;
        let total_matched = results.len();

        debug!(
            iterations = iterations,
            total_searched = total_searched,
            matched = total_matched,
            has_more = has_more,
            "[SEARCH] Iterative search completed"
        );

        SearchResponse {
            results,
            total_searched,
            total_matched,
            next_cursor: current_cursor,
            has_more,
            search_id,
        }
    }

    fn build_query_params_with_cursor(
        &self,
        request: &SearchRequest,
        cursor: Option<u64>,
    ) -> QueryParams {
        let mut params = QueryParams {
            cursor,
            limit: Some(SEARCH_BATCH_SIZE),
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
                "H3" => params.is_h3 = Some(true),
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

    fn matches_filter_compact(
        &self,
        compact: &TrafficSummaryCompact,
        filters: &SearchFilters,
    ) -> bool {
        use crate::TrafficFlags;

        if let Some(rule_hit) = filters.has_rule_hit {
            let has_rule = (compact.flags & TrafficFlags::HAS_RULE_HIT) != 0;
            if has_rule != rule_hit {
                return false;
            }
        }

        if !filters.protocols.is_empty() {
            let protocol_upper = compact.proto.to_uppercase();
            let is_websocket = (compact.flags & TrafficFlags::IS_WEBSOCKET) != 0;
            let is_sse = (compact.flags & TrafficFlags::IS_SSE) != 0;
            let is_h3 = (compact.flags & TrafficFlags::IS_H3) != 0;
            let mut matched = false;

            for p in &filters.protocols {
                match p.to_uppercase().as_str() {
                    "HTTP" => {
                        if protocol_upper == "HTTP"
                            || protocol_upper == "HTTP/1.0"
                            || protocol_upper == "HTTP/1.1"
                        {
                            matched = true;
                            break;
                        }
                    }
                    "HTTPS" => {
                        if protocol_upper == "HTTPS" || protocol_upper == "HTTP/2" {
                            matched = true;
                            break;
                        }
                    }
                    "H2" => {
                        if protocol_upper.contains("HTTP/2") {
                            matched = true;
                            break;
                        }
                    }
                    "WS" => {
                        if is_websocket && protocol_upper == "WS" {
                            matched = true;
                            break;
                        }
                    }
                    "WSS" => {
                        if is_websocket && protocol_upper == "WSS" {
                            matched = true;
                            break;
                        }
                    }
                    "H3" => {
                        if is_h3 || protocol_upper == "H3" {
                            matched = true;
                            break;
                        }
                    }
                    "SSE" => {
                        if is_sse {
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
            let status = compact.s;
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
            let res_ct = compact.ct.as_deref().unwrap_or("").to_lowercase();
            let req_ct = compact.req_ct.as_deref().unwrap_or("").to_lowercase();
            let mut matched = false;

            for ct in &filters.content_types {
                let ct_lower = ct.to_lowercase();
                let patterns: Vec<&str> = match ct_lower.as_str() {
                    "json" => vec!["json", "application/json", "text/json"],
                    "form" => vec![
                        "form",
                        "x-www-form-urlencoded",
                        "multipart/form-data",
                        "application/x-www-form-urlencoded",
                    ],
                    "xml" => vec!["xml", "application/xml", "text/xml"],
                    "js" => vec!["javascript", "text/javascript", "application/javascript"],
                    "css" => vec!["css", "text/css"],
                    "font" => vec![
                        "font",
                        "woff",
                        "woff2",
                        "ttf",
                        "otf",
                        "eot",
                        "font/",
                        "application/font",
                    ],
                    "doc" => vec!["html", "text/html", "application/xhtml"],
                    "media" => vec![
                        "image", "video", "audio", "image/", "video/", "audio/", "png", "jpg",
                        "jpeg", "gif", "webp", "svg", "mp4", "webm", "mp3", "wav",
                    ],
                    "sse" => vec!["event-stream", "text/event-stream"],
                    _ => vec![ct_lower.as_str()],
                };

                for pattern in patterns {
                    if res_ct.contains(pattern) || req_ct.contains(pattern) {
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

        if !filters.client_ips.is_empty() && !filters.client_ips.contains(&compact.cip) {
            return false;
        }

        if !filters.client_apps.is_empty() {
            match &compact.capp {
                Some(app) if filters.client_apps.contains(app) => {}
                _ => return false,
            }
        }

        if !filters.domains.is_empty() {
            let host = &compact.h;
            if !filters.domains.iter().any(|d| host.contains(d)) {
                return false;
            }
        }

        true
    }

    fn matches_conditions(&self, record: &TrafficRecord, conditions: &[FilterCondition]) -> bool {
        for condition in conditions {
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
