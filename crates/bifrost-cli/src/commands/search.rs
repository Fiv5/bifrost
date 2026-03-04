use std::io::stdout;
use std::time::Duration;

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::CrosstermBackend,
    style::{Color as RColor, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, Wrap,
    },
    Frame, Terminal,
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total_searched: usize,
    pub total_matched: usize,
    pub next_cursor: Option<u64>,
    pub has_more: bool,
    search_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResultItem {
    pub record: TrafficSummary,
    pub matches: Vec<MatchLocation>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TrafficSummary {
    pub id: String,
    seq: u64,
    pub ts: u64,
    pub m: String,
    pub h: String,
    pub p: String,
    pub s: u16,
    ct: Option<String>,
    req_ct: Option<String>,
    pub req_sz: usize,
    pub res_sz: usize,
    pub dur: u64,
    pub proto: String,
    cip: String,
    capp: Option<String>,
    flags: u32,
    fc: usize,
    st: String,
    et: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MatchLocation {
    pub field: String,
    pub preview: String,
    offset: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct TrafficDetail {
    id: String,
    method: String,
    host: String,
    url: String,
    path: String,
    status: u16,
    protocol: String,
    content_type: Option<String>,
    request_content_type: Option<String>,
    request_size: usize,
    response_size: usize,
    duration_ms: u64,
    client_ip: String,
    client_app: Option<String>,
    #[allow(dead_code)]
    timestamp: u64,
    request_headers: Option<Vec<(String, String)>>,
    response_headers: Option<Vec<(String, String)>>,
    is_websocket: bool,
    is_sse: bool,
    is_tunnel: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Table,
    Compact,
    Json,
    JsonPretty,
}

impl std::str::FromStr for OutputFormat {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "compact" | "c" => Self::Compact,
            "json" | "j" => Self::Json,
            "json-pretty" | "jp" => Self::JsonPretty,
            _ => Self::Table,
        })
    }
}

pub struct SearchOptions {
    pub keyword: String,
    pub port: u16,
    pub limit: usize,
    pub format: OutputFormat,
    pub interactive: bool,
    pub scope_url: bool,
    pub scope_headers: bool,
    pub scope_body: bool,
    pub filter_status: Option<String>,
    #[allow(dead_code)]
    pub filter_method: Option<String>,
    #[allow(dead_code)]
    pub filter_protocol: Option<String>,
    pub filter_content_type: Option<String>,
    pub filter_domain: Option<String>,
    pub no_color: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            keyword: String::new(),
            port: 9900,
            limit: 50,
            format: OutputFormat::Table,
            interactive: false,
            scope_url: false,
            scope_headers: false,
            scope_body: false,
            filter_status: None,
            filter_method: None,
            filter_protocol: None,
            filter_content_type: None,
            filter_domain: None,
            no_color: false,
        }
    }
}

fn check_proxy_running(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/metrics", port);
    ureq::get(&url)
        .timeout(Duration::from_secs(2))
        .call()
        .is_ok()
}

pub fn run_search(options: SearchOptions) -> i32 {
    if !check_proxy_running(options.port) {
        eprintln!(
            "\x1b[31m✗\x1b[0m Bifrost proxy is not running on port {}",
            options.port
        );
        eprintln!(
            "  Start it with: \x1b[36mbifrost start -p {}\x1b[0m",
            options.port
        );
        return 1;
    }

    if options.interactive {
        run_interactive_search(options)
    } else {
        run_simple_search(options)
    }
}

fn run_simple_search(options: SearchOptions) -> i32 {
    let response = match execute_search(&options, None) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("\x1b[31m✗\x1b[0m Search failed: {}", e);
            return 1;
        }
    };

    if response.results.is_empty() {
        if options.format == OutputFormat::Json || options.format == OutputFormat::JsonPretty {
            println!("{{\"results\":[],\"total_matched\":0}}");
        } else {
            println!(
                "\x1b[33m⚠\x1b[0m No results found for '\x1b[1m{}\x1b[0m'",
                options.keyword
            );
            println!(
                "  Searched {} records",
                format_number(response.total_searched)
            );
        }
        return 0;
    }

    match options.format {
        OutputFormat::Table => print_table_format(&response, &options),
        OutputFormat::Compact => print_compact_format(&response, &options),
        OutputFormat::Json => print_json_format(&response, false),
        OutputFormat::JsonPretty => print_json_format(&response, true),
    }

    0
}

fn execute_search(options: &SearchOptions, cursor: Option<u64>) -> Result<SearchResponse, String> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/search", options.port);

    let mut scope = serde_json::json!({});
    if options.scope_url || options.scope_headers || options.scope_body {
        scope = serde_json::json!({
            "all": false,
            "url": options.scope_url,
            "request_headers": options.scope_headers,
            "response_headers": options.scope_headers,
            "request_body": options.scope_body,
            "response_body": options.scope_body,
        });
    }

    let mut filters = serde_json::json!({});
    if let Some(ref status) = options.filter_status {
        filters["status_ranges"] = serde_json::json!([status]);
    }
    if let Some(ref domain) = options.filter_domain {
        filters["domains"] = serde_json::json!([domain]);
    }
    if let Some(ref ct) = options.filter_content_type {
        filters["content_types"] = serde_json::json!([ct]);
    }
    if let Some(ref proto) = options.filter_protocol {
        filters["protocols"] = serde_json::json!([proto]);
    }

    let mut body = serde_json::json!({
        "keyword": options.keyword,
        "scope": scope,
        "filters": filters,
        "limit": options.limit,
    });

    if let Some(c) = cursor {
        body["cursor"] = serde_json::json!(c);
    }

    let response = ureq::post(&url)
        .timeout(Duration::from_secs(30))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| format!("Request failed: {}", e))?;

    response
        .into_json::<SearchResponse>()
        .map_err(|e| format!("Failed to parse response: {}", e))
}

fn print_table_format(response: &SearchResponse, options: &SearchOptions) {
    let use_color = !options.no_color && atty::is(atty::Stream::Stdout);

    println!();
    if use_color {
        println!(
            "\x1b[1;32m✓\x1b[0m Found \x1b[1m{}\x1b[0m matches for '\x1b[1;36m{}\x1b[0m'",
            response.total_matched, options.keyword
        );
    } else {
        println!(
            "Found {} matches for '{}'",
            response.total_matched, options.keyword
        );
    }
    println!();

    let header = if use_color {
        format!(
            "\x1b[1;37m{:>10}  {:>6}  {:>6}  {:7}  {:40}  {:46}  {:>10}  {:>8}\x1b[0m",
            "SEQ", "STATUS", "METHOD", "PROTO", "HOST", "PATH", "SIZE", "TIME"
        )
    } else {
        format!(
            "{:>10}  {:>6}  {:>6}  {:7}  {:40}  {:46}  {:>10}  {:>8}",
            "SEQ", "STATUS", "METHOD", "PROTO", "HOST", "PATH", "SIZE", "TIME"
        )
    };
    println!("{}", header);
    println!("{}", "─".repeat(150));

    for item in &response.results {
        let r = &item.record;

        let status_str = if r.s == 0 {
            "...".to_string()
        } else {
            r.s.to_string()
        };

        let (status_color, status_display) = if use_color {
            match r.s {
                0 => ("\x1b[90m", format!("{:>6}", status_str)),
                200..=299 => ("\x1b[32m", format!("{:>6}", status_str)),
                300..=399 => ("\x1b[33m", format!("{:>6}", status_str)),
                400..=499 => ("\x1b[31m", format!("{:>6}", status_str)),
                500..=599 => ("\x1b[1;31m", format!("{:>6}", status_str)),
                _ => ("\x1b[37m", format!("{:>6}", status_str)),
            }
        } else {
            ("", format!("{:>6}", status_str))
        };

        let method_display = if use_color {
            match r.m.as_str() {
                "GET" => format!("\x1b[36m{:>6}\x1b[0m", r.m),
                "POST" => format!("\x1b[33m{:>6}\x1b[0m", r.m),
                "PUT" => format!("\x1b[35m{:>6}\x1b[0m", r.m),
                "DELETE" => format!("\x1b[31m{:>6}\x1b[0m", r.m),
                "PATCH" => format!("\x1b[34m{:>6}\x1b[0m", r.m),
                _ => format!("{:>6}", r.m),
            }
        } else {
            format!("{:>6}", r.m)
        };

        let proto = truncate_str(&r.proto, 7);
        let host = highlight_keyword(&truncate_str(&r.h, 40), &options.keyword, use_color);
        let path = highlight_keyword(&truncate_str(&r.p, 46), &options.keyword, use_color);
        let size = format_size(r.res_sz);
        let time = format_duration(r.dur);
        let seq = r.seq.to_string();

        if use_color {
            println!(
                "\x1b[90m{:>10}\x1b[0m  {}{}  {}  {:7}  {}  {}  {:>10}  {:>8}\x1b[0m",
                seq, status_color, status_display, method_display, proto, host, path, size, time
            );
        } else {
            println!(
                "{:>10}  {}  {}  {:7}  {}  {}  {:>10}  {:>8}",
                seq, status_display, method_display, proto, host, path, size, time
            );
        }

        if !item.matches.is_empty() && item.matches.iter().any(|m| m.field != "url") {
            for m in &item.matches {
                if m.field == "url" {
                    continue;
                }
                let preview = highlight_keyword(&m.preview, &options.keyword, use_color);
                if use_color {
                    println!(
                        "        \x1b[90m└─ \x1b[34m{}\x1b[90m: {}\x1b[0m",
                        m.field, preview
                    );
                } else {
                    println!("        └─ {}: {}", m.field, preview);
                }
            }
        }
    }

    println!();
    if response.has_more {
        if use_color {
            println!(
                "\x1b[90m  ... and more results. Use --limit to see more, or -i for interactive mode.\x1b[0m"
            );
        } else {
            println!(
                "  ... and more results. Use --limit to see more, or -i for interactive mode."
            );
        }
    }
}

fn print_compact_format(response: &SearchResponse, options: &SearchOptions) {
    let use_color = !options.no_color && atty::is(atty::Stream::Stdout);

    for item in &response.results {
        let r = &item.record;
        let status = if r.s == 0 { "..." } else { &r.s.to_string() };

        if use_color {
            let status_color = match r.s {
                0 => "\x1b[90m",
                200..=299 => "\x1b[32m",
                300..=399 => "\x1b[33m",
                400..=499 => "\x1b[31m",
                500..=599 => "\x1b[1;31m",
                _ => "\x1b[37m",
            };
            println!(
                "\x1b[90m{:>10}\x1b[0m {}{}\x1b[0m {} \x1b[36m{}\x1b[0m{}",
                r.seq, status_color, status, r.m, r.h, r.p
            );
        } else {
            println!("{:>10} {} {} {}{}", r.seq, status, r.m, r.h, r.p);
        }
    }
}

fn print_json_format(response: &SearchResponse, pretty: bool) {
    let output = serde_json::json!({
        "results": response.results.iter().map(|item| {
            serde_json::json!({
                "id": item.record.id,
                "seq": item.record.seq,
                "method": item.record.m,
                "host": item.record.h,
                "path": item.record.p,
                "status": item.record.s,
                "protocol": item.record.proto,
                "request_size": item.record.req_sz,
                "response_size": item.record.res_sz,
                "duration_ms": item.record.dur,
                "timestamp": item.record.ts,
                "matches": item.matches.iter().map(|m| {
                    serde_json::json!({
                        "field": m.field,
                        "preview": m.preview,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "total_matched": response.total_matched,
        "total_searched": response.total_searched,
        "has_more": response.has_more,
    });

    if pretty {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("{}", output);
    }
}

fn highlight_keyword(text: &str, keyword: &str, use_color: bool) -> String {
    if !use_color || keyword.is_empty() {
        return text.to_string();
    }

    let lower_text = text.to_lowercase();
    let lower_keyword = keyword.to_lowercase();

    if !lower_text.contains(&lower_keyword) {
        return text.to_string();
    }

    let mut result = String::new();
    let mut last_end = 0;

    for (start, _) in lower_text.match_indices(&lower_keyword) {
        let prefix = match text.get(last_end..start) {
            Some(s) => s,
            None => return text.to_string(),
        };
        result.push_str(prefix);
        result.push_str("\x1b[1;33m");
        let end = start + lower_keyword.len();
        let highlighted = match text.get(start..end) {
            Some(s) => s,
            None => return text.to_string(),
        };
        result.push_str(highlighted);
        result.push_str("\x1b[0m");
        last_end = end;
    }
    let rest = match text.get(last_end..) {
        Some(s) => s,
        None => return text.to_string(),
    };
    result.push_str(rest);

    result
}

fn truncate_str(s: &str, max_len: usize) -> String {
    let len = s.chars().count();
    if len <= max_len {
        return s.to_string();
    }

    let keep = max_len.saturating_sub(3);
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= keep {
            break;
        }
        out.push(ch);
    }
    if keep < max_len {
        out.push_str("...");
    }
    out
}

fn format_size(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;

    if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

fn format_duration(ms: u64) -> String {
    if ms == 0 {
        "...".to_string()
    } else if ms >= 1000 {
        format!("{:.2}s", ms as f64 / 1000.0)
    } else {
        format!("{}ms", ms)
    }
}

fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

struct InteractiveApp {
    options: SearchOptions,
    results: Vec<SearchResultItem>,
    total_matched: usize,
    total_searched: usize,
    has_more: bool,
    next_cursor: Option<u64>,
    selected_index: usize,
    scroll_offset: usize,
    search_input: String,
    mode: AppMode,
    detail_record: Option<TrafficDetail>,
    detail_scroll: usize,
    detail_tab: usize,
    request_body: Option<String>,
    response_body: Option<String>,
    loading: bool,
    error_message: Option<String>,
    visible_height: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum AppMode {
    List,
    Search,
    Detail,
}

impl InteractiveApp {
    fn new(options: SearchOptions) -> Self {
        let initial_keyword = options.keyword.clone();
        Self {
            options,
            results: Vec::new(),
            total_matched: 0,
            total_searched: 0,
            has_more: false,
            next_cursor: None,
            selected_index: 0,
            scroll_offset: 0,
            search_input: initial_keyword,
            mode: AppMode::List,
            detail_record: None,
            detail_scroll: 0,
            detail_tab: 0,
            request_body: None,
            response_body: None,
            loading: false,
            error_message: None,
            visible_height: 20,
        }
    }

    fn search(&mut self) {
        self.loading = true;
        self.error_message = None;
        self.options.keyword = self.search_input.clone();

        match execute_search(&self.options, None) {
            Ok(response) => {
                self.results = response.results;
                self.total_matched = response.total_matched;
                self.total_searched = response.total_searched;
                self.has_more = response.has_more;
                self.next_cursor = response.next_cursor;
                self.selected_index = 0;
                self.scroll_offset = 0;
            }
            Err(e) => {
                self.error_message = Some(e);
                self.results.clear();
            }
        }
        self.loading = false;
    }

    fn load_more(&mut self) {
        if !self.has_more || self.next_cursor.is_none() {
            return;
        }

        self.loading = true;
        match execute_search(&self.options, self.next_cursor) {
            Ok(response) => {
                self.results.extend(response.results);
                self.has_more = response.has_more;
                self.next_cursor = response.next_cursor;
            }
            Err(e) => {
                self.error_message = Some(e);
            }
        }
        self.loading = false;
    }

    fn load_detail(&mut self) {
        if self.results.is_empty() {
            return;
        }

        let id = &self.results[self.selected_index].record.id;
        let url = format!(
            "http://127.0.0.1:{}/_bifrost/api/traffic/{}",
            self.options.port, id
        );

        self.loading = true;
        match ureq::get(&url).timeout(Duration::from_secs(5)).call() {
            Ok(resp) => {
                if let Ok(detail) = resp.into_json::<TrafficDetail>() {
                    self.detail_record = Some(detail);
                    self.detail_scroll = 0;
                    self.detail_tab = 0;
                    self.mode = AppMode::Detail;

                    self.load_bodies();
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to load detail: {}", e));
            }
        }
        self.loading = false;
    }

    fn load_bodies(&mut self) {
        let id = match &self.detail_record {
            Some(r) => r.id.clone(),
            None => return,
        };

        let req_url = format!(
            "http://127.0.0.1:{}/_bifrost/api/traffic/{}/request-body",
            self.options.port, id
        );
        let res_url = format!(
            "http://127.0.0.1:{}/_bifrost/api/traffic/{}/response-body",
            self.options.port, id
        );

        if let Ok(resp) = ureq::get(&req_url).timeout(Duration::from_secs(5)).call() {
            if let Ok(body) = resp.into_json::<serde_json::Value>() {
                if let Some(data) = body.get("data") {
                    if !data.is_null() {
                        self.request_body = data.as_str().map(|s| s.to_string());
                    }
                }
            }
        }

        if let Ok(resp) = ureq::get(&res_url).timeout(Duration::from_secs(5)).call() {
            if let Ok(body) = resp.into_json::<serde_json::Value>() {
                if let Some(data) = body.get("data") {
                    if !data.is_null() {
                        self.response_body = data.as_str().map(|s| s.to_string());
                    }
                }
            }
        }
    }

    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    fn move_down(&mut self) {
        if self.selected_index < self.results.len().saturating_sub(1) {
            self.selected_index += 1;
            if self.selected_index >= self.scroll_offset + self.visible_height {
                self.scroll_offset = self.selected_index - self.visible_height + 1;
            }

            if self.selected_index >= self.results.len() - 5 && self.has_more {
                self.load_more();
            }
        }
    }

    fn page_up(&mut self) {
        let page_size = self.visible_height.saturating_sub(2);
        self.selected_index = self.selected_index.saturating_sub(page_size);
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }

    fn page_down(&mut self) {
        let page_size = self.visible_height.saturating_sub(2);
        let max_index = self.results.len().saturating_sub(1);
        self.selected_index = (self.selected_index + page_size).min(max_index);

        if self.selected_index >= self.scroll_offset + self.visible_height {
            self.scroll_offset = self.selected_index.saturating_sub(self.visible_height - 1);
        }

        if self.selected_index >= self.results.len() - 5 && self.has_more {
            self.load_more();
        }
    }

    fn scroll_detail(&mut self, delta: i32) {
        if delta < 0 {
            self.detail_scroll = self.detail_scroll.saturating_sub((-delta) as usize);
        } else {
            self.detail_scroll = self.detail_scroll.saturating_add(delta as usize);
        }
    }
}

fn run_interactive_search(options: SearchOptions) -> i32 {
    let mut app = InteractiveApp::new(options);

    if !app.search_input.is_empty() {
        app.search();
    }

    let result = run_tui(&mut app);

    if let Err(e) = result {
        eprintln!("TUI error: {}", e);
        return 1;
    }

    0
}

fn run_tui(app: &mut InteractiveApp) -> std::io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(Hide)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui_loop(&mut terminal, app);

    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.backend_mut().execute(Show)?;

    result
}

fn run_tui_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut InteractiveApp,
) -> std::io::Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.mode {
                    AppMode::List => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(())
                        }
                        KeyCode::Char('/') | KeyCode::Char('s') => {
                            app.mode = AppMode::Search;
                        }
                        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                        KeyCode::PageUp => app.page_up(),
                        KeyCode::PageDown => app.page_down(),
                        KeyCode::Home | KeyCode::Char('g') => {
                            app.selected_index = 0;
                            app.scroll_offset = 0;
                        }
                        KeyCode::End | KeyCode::Char('G') => {
                            if !app.results.is_empty() {
                                app.selected_index = app.results.len() - 1;
                                if app.selected_index >= app.visible_height {
                                    app.scroll_offset = app.selected_index - app.visible_height + 1;
                                }
                            }
                        }
                        KeyCode::Enter => app.load_detail(),
                        KeyCode::Char('r') => app.search(),
                        _ => {}
                    },
                    AppMode::Search => match key.code {
                        KeyCode::Esc => {
                            app.mode = AppMode::List;
                        }
                        KeyCode::Enter => {
                            app.mode = AppMode::List;
                            app.search();
                        }
                        KeyCode::Backspace => {
                            app.search_input.pop();
                        }
                        KeyCode::Char(c) => {
                            app.search_input.push(c);
                        }
                        _ => {}
                    },
                    AppMode::Detail => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.mode = AppMode::List;
                            app.detail_record = None;
                            app.request_body = None;
                            app.response_body = None;
                        }
                        KeyCode::Tab => {
                            app.detail_tab = (app.detail_tab + 1) % 4;
                            app.detail_scroll = 0;
                        }
                        KeyCode::BackTab => {
                            app.detail_tab = if app.detail_tab == 0 {
                                3
                            } else {
                                app.detail_tab - 1
                            };
                            app.detail_scroll = 0;
                        }
                        KeyCode::Up | KeyCode::Char('k') => app.scroll_detail(-1),
                        KeyCode::Down | KeyCode::Char('j') => app.scroll_detail(1),
                        KeyCode::PageUp => app.scroll_detail(-10),
                        KeyCode::PageDown => app.scroll_detail(10),
                        KeyCode::Home => app.detail_scroll = 0,
                        _ => {}
                    },
                }
            }
        }
    }
}

fn draw_ui(f: &mut Frame, app: &mut InteractiveApp) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(size);

    let list_rows = chunks[1].height.saturating_sub(3) / 2;
    app.visible_height = match app.mode {
        AppMode::Detail => size.height.saturating_sub(6) as usize,
        _ => list_rows.max(1) as usize,
    };

    draw_search_bar(f, app, chunks[0]);

    match app.mode {
        AppMode::Detail => draw_detail_view(f, app, chunks[1]),
        _ => draw_results_list(f, app, chunks[1]),
    }

    draw_status_bar(f, app, chunks[2]);
}

fn draw_search_bar(f: &mut Frame, app: &InteractiveApp, area: Rect) {
    let style = if app.mode == AppMode::Search {
        Style::default().fg(RColor::Yellow)
    } else {
        Style::default().fg(RColor::White)
    };

    let cursor_char = if app.mode == AppMode::Search {
        "│"
    } else {
        ""
    };
    let search_text = format!(" 🔍 {}{}", app.search_input, cursor_char);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Search ")
        .title_style(
            Style::default()
                .fg(RColor::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(style);

    let paragraph = Paragraph::new(search_text)
        .block(block)
        .style(Style::default().fg(RColor::White));

    f.render_widget(paragraph, area);
}

fn draw_results_list(f: &mut Frame, app: &InteractiveApp, area: Rect) {
    if app.loading {
        let loading = Paragraph::new(" Loading...")
            .style(Style::default().fg(RColor::Yellow))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Results "),
            );
        f.render_widget(loading, area);
        return;
    }

    if let Some(ref err) = app.error_message {
        let error = Paragraph::new(format!(" ✗ {}", err))
            .style(Style::default().fg(RColor::Red))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Error "),
            );
        f.render_widget(error, area);
        return;
    }

    if app.results.is_empty() {
        let empty_msg = if app.search_input.is_empty() {
            " Press / or s to start searching"
        } else {
            " No results found"
        };
        let empty = Paragraph::new(empty_msg)
            .style(Style::default().fg(RColor::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Results "),
            );
        f.render_widget(empty, area);
        return;
    }

    let header_cells = ["SEQ", "STATUS", "METHOD", "HOST", "PATH", "MATCH"]
        .iter()
        .map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(RColor::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        });
    let header = Row::new(header_cells).height(1);

    let visible_results: Vec<_> = app
        .results
        .iter()
        .enumerate()
        .skip(app.scroll_offset)
        .take(app.visible_height)
        .collect();

    let rows = visible_results.iter().map(|(idx, item)| {
        let r = &item.record;
        let is_selected = *idx == app.selected_index;

        let status_str = if r.s == 0 {
            "...".to_string()
        } else {
            r.s.to_string()
        };

        let status_style = match r.s {
            0 => Style::default().fg(RColor::DarkGray),
            200..=299 => Style::default().fg(RColor::Green),
            300..=399 => Style::default().fg(RColor::Yellow),
            400..=499 => Style::default().fg(RColor::Red),
            500..=599 => Style::default()
                .fg(RColor::LightRed)
                .add_modifier(Modifier::BOLD),
            _ => Style::default().fg(RColor::White),
        };

        let method_style = match r.m.as_str() {
            "GET" => Style::default().fg(RColor::Cyan),
            "POST" => Style::default().fg(RColor::Yellow),
            "PUT" => Style::default().fg(RColor::Magenta),
            "DELETE" => Style::default().fg(RColor::Red),
            "PATCH" => Style::default().fg(RColor::Blue),
            _ => Style::default().fg(RColor::White),
        };

        let row_style = if is_selected {
            Style::default().bg(RColor::DarkGray)
        } else {
            Style::default()
        };

        let seq_line = if is_selected {
            Line::from(vec![
                Span::styled("▶", Style::default().fg(RColor::Yellow)),
                Span::styled(
                    format!("{:>9}", r.seq),
                    Style::default()
                        .fg(RColor::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        } else {
            Line::from(Span::styled(
                format!("{:>10}", r.seq),
                Style::default().fg(RColor::DarkGray),
            ))
        };

        let (match_header, match_preview) = build_match_summary(item, &app.options.keyword);
        let match_cell = Cell::from(Text::from(vec![match_header, match_preview]))
            .style(Style::default().fg(RColor::White));

        Row::new(vec![
            Cell::from(Text::from(vec![seq_line, Line::from("")])),
            Cell::from(status_str).style(status_style),
            Cell::from(r.m.clone()).style(method_style),
            Cell::from(truncate_str(&r.h, 28)),
            Cell::from(truncate_str(&r.p, 40)),
            match_cell,
        ])
        .height(2)
        .style(row_style)
    });

    let title = format!(" Results ({}/{}) ", app.total_matched, app.total_searched);

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Length(30),
            Constraint::Min(20),
            Constraint::Min(40),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title)
            .title_style(
                Style::default()
                    .fg(RColor::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
    );

    f.render_widget(table, area);

    if app.results.len() > app.visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"));

        let mut scrollbar_state =
            ScrollbarState::new(app.results.len()).position(app.selected_index);

        let scrollbar_area = Rect {
            x: area.x + area.width - 1,
            y: area.y + 1,
            width: 1,
            height: area.height.saturating_sub(2),
        };

        f.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }
}

fn build_match_summary(item: &SearchResultItem, keyword: &str) -> (Line<'static>, Line<'static>) {
    if item.matches.is_empty() {
        let header = Line::from(Span::styled(
            "NO_MATCH",
            Style::default().fg(RColor::DarkGray),
        ));
        return (header, Line::from(""));
    }

    let mut fields = Vec::<&str>::new();
    for m in &item.matches {
        let f = m.field.as_str();
        if !fields.contains(&f) {
            fields.push(f);
        }
    }

    let primary = item
        .matches
        .iter()
        .find(|m| m.field != "url")
        .unwrap_or(&item.matches[0]);

    let label = match_field_label(&primary.field);
    let extra = fields.len().saturating_sub(1);
    let header_text = if extra > 0 {
        format!("{}+{} ({})", label, extra, item.matches.len())
    } else {
        format!("{} ({})", label, item.matches.len())
    };

    let header = Line::from(vec![
        Span::styled(
            header_text,
            Style::default()
                .fg(RColor::LightCyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            truncate_str(&primary.preview, 28),
            Style::default().fg(RColor::DarkGray),
        ),
    ]);

    let preview_spans = highlight_spans(&truncate_str(&primary.preview, 80), keyword);
    let preview = Line::from(preview_spans);
    (header, preview)
}

fn match_field_label(field: &str) -> &'static str {
    match field {
        "url" => "URL",
        "request_headers" => "REQ_HDR",
        "response_headers" => "RES_HDR",
        "request_body" => "REQ_BODY",
        "response_body" => "RES_BODY",
        "frames" => "FRAMES",
        _ => "MATCH",
    }
}

fn highlight_spans(text: &str, keyword: &str) -> Vec<Span<'static>> {
    if keyword.is_empty() {
        return vec![Span::raw(text.to_string())];
    }

    let lower_text = text.to_lowercase();
    let lower_keyword = keyword.to_lowercase();
    if !lower_text.contains(&lower_keyword) {
        return vec![Span::raw(text.to_string())];
    }

    let mut spans = Vec::new();
    let mut last_end = 0usize;

    for (start, _) in lower_text.match_indices(&lower_keyword) {
        if start > last_end {
            let prefix = match text.get(last_end..start) {
                Some(s) => s,
                None => return vec![Span::raw(text.to_string())],
            };
            spans.push(Span::raw(prefix.to_string()));
        }
        let end = start + lower_keyword.len();
        let highlighted = match text.get(start..end) {
            Some(s) => s,
            None => return vec![Span::raw(text.to_string())],
        };
        spans.push(Span::styled(
            highlighted.to_string(),
            Style::default()
                .fg(RColor::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        last_end = end;
    }

    if last_end < text.len() {
        let rest = match text.get(last_end..) {
            Some(s) => s,
            None => return vec![Span::raw(text.to_string())],
        };
        spans.push(Span::raw(rest.to_string()));
    }

    spans
}

fn draw_detail_view(f: &mut Frame, app: &InteractiveApp, area: Rect) {
    let detail = match &app.detail_record {
        Some(d) => d,
        None => return,
    };

    let tabs_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 3,
    };

    let content_area = Rect {
        x: area.x,
        y: area.y + 3,
        width: area.width,
        height: area.height.saturating_sub(3),
    };

    let tab_titles = [
        " Overview ",
        " Request Headers ",
        " Response Headers ",
        " Body ",
    ];

    let tabs: Vec<Line> = tab_titles
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if i == app.detail_tab {
                Line::from(Span::styled(
                    *t,
                    Style::default()
                        .fg(RColor::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(*t, Style::default().fg(RColor::DarkGray)))
            }
        })
        .collect();

    let tabs_widget = ratatui::widgets::Tabs::new(tabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(format!(" {} ", detail.id))
                .title_style(
                    Style::default()
                        .fg(RColor::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .highlight_style(Style::default().fg(RColor::Yellow))
        .select(app.detail_tab);

    f.render_widget(tabs_widget, tabs_area);

    let content_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let content = match app.detail_tab {
        0 => format_overview(detail),
        1 => format_headers(&detail.request_headers, "Request Headers"),
        2 => format_headers(&detail.response_headers, "Response Headers"),
        3 => format_body(app),
        _ => String::new(),
    };

    let lines: Vec<&str> = content.lines().collect();
    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(app.detail_scroll)
        .take(content_area.height.saturating_sub(2) as usize)
        .map(|s| Line::from(*s))
        .collect();

    let paragraph = Paragraph::new(visible_lines)
        .block(content_block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, content_area);
}

fn format_overview(detail: &TrafficDetail) -> String {
    let status_emoji = match detail.status {
        0 => "⏳",
        200..=299 => "✅",
        300..=399 => "↗️",
        400..=499 => "⚠️",
        500..=599 => "❌",
        _ => "❓",
    };

    format!(
        r#"
  {} {}  {}

  ┌─────────────────────────────────────────────────────────────────────
  │  URL          {}{}
  │  Method       {}
  │  Protocol     {}
  │  Status       {} {}
  │
  │  Request
  │    Size       {}
  │    Type       {}
  │
  │  Response
  │    Size       {}
  │    Type       {}
  │    Duration   {}
  │
  │  Client
  │    IP         {}
  │    App        {}
  │
  │  Flags
  │    WebSocket  {}
  │    SSE        {}
  │    Tunnel     {}
  └─────────────────────────────────────────────────────────────────────
"#,
        status_emoji,
        detail.method,
        detail.url,
        detail.host,
        detail.path,
        detail.method,
        detail.protocol,
        detail.status,
        status_emoji,
        format_size(detail.request_size),
        detail.request_content_type.as_deref().unwrap_or("-"),
        format_size(detail.response_size),
        detail.content_type.as_deref().unwrap_or("-"),
        format_duration(detail.duration_ms),
        detail.client_ip,
        detail.client_app.as_deref().unwrap_or("-"),
        if detail.is_websocket { "Yes" } else { "No" },
        if detail.is_sse { "Yes" } else { "No" },
        if detail.is_tunnel { "Yes" } else { "No" },
    )
}

fn format_headers(headers: &Option<Vec<(String, String)>>, title: &str) -> String {
    match headers {
        Some(h) if !h.is_empty() => {
            let mut result = format!("\n  {}\n  {}\n\n", title, "─".repeat(60));
            for (name, value) in h {
                result.push_str(&format!("  {}: {}\n", name, value));
            }
            result
        }
        _ => format!("\n  No {} available", title.to_lowercase()),
    }
}

fn format_body(app: &InteractiveApp) -> String {
    let mut result = String::new();

    result.push_str("\n  ═══ Request Body ═══\n\n");
    match &app.request_body {
        Some(body) if !body.is_empty() => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
                if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                    for line in pretty.lines() {
                        result.push_str(&format!("  {}\n", line));
                    }
                } else {
                    result.push_str(&format!("  {}\n", body));
                }
            } else {
                for line in body.lines().take(100) {
                    result.push_str(&format!("  {}\n", line));
                }
            }
        }
        _ => result.push_str("  (empty)\n"),
    }

    result.push_str("\n  ═══ Response Body ═══\n\n");
    match &app.response_body {
        Some(body) if !body.is_empty() => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
                if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                    for line in pretty.lines() {
                        result.push_str(&format!("  {}\n", line));
                    }
                } else {
                    result.push_str(&format!("  {}\n", body));
                }
            } else {
                for line in body.lines().take(100) {
                    result.push_str(&format!("  {}\n", line));
                }
            }
        }
        _ => result.push_str("  (empty)\n"),
    }

    result
}

fn draw_status_bar(f: &mut Frame, app: &InteractiveApp, area: Rect) {
    let help_text = match app.mode {
        AppMode::List => " ↑/k ↓/j Navigate │ Enter View │ /,s Search │ r Refresh │ q Quit ",
        AppMode::Search => " Type to search │ Enter Confirm │ Esc Cancel ",
        AppMode::Detail => " Tab Switch │ ↑/k ↓/j Scroll │ Esc/q Back ",
    };

    let status = Paragraph::new(help_text)
        .style(Style::default().fg(RColor::DarkGray).bg(RColor::Black))
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(status, area);
}
