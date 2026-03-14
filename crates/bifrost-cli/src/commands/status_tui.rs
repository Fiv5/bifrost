use std::io::stdout;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Row, Sparkline, Table, Tabs},
    Frame, Terminal,
};
use serde::Deserialize;

use crate::process::{is_process_running, read_pid, read_runtime_port};

fn direct_agent() -> ureq::Agent {
    bifrost_core::direct_ureq_agent_builder()
        .timeout(HTTP_TIMEOUT)
        .build()
}

#[derive(Debug, Deserialize, Default, Clone)]
struct TrafficTypeMetrics {
    requests: u64,
    bytes_sent: u64,
    bytes_received: u64,
    active_connections: u64,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct MetricsSnapshot {
    #[allow(dead_code)]
    timestamp: u64,
    memory_used: u64,
    memory_total: u64,
    cpu_usage: f32,
    total_requests: u64,
    active_connections: u64,
    bytes_sent: u64,
    bytes_received: u64,
    bytes_sent_rate: f32,
    bytes_received_rate: f32,
    qps: f32,
    max_qps: f32,
    max_bytes_sent_rate: f32,
    max_bytes_received_rate: f32,
    http: TrafficTypeMetrics,
    https: TrafficTypeMetrics,
    tunnel: TrafficTypeMetrics,
    ws: TrafficTypeMetrics,
    wss: TrafficTypeMetrics,
    socks5: TrafficTypeMetrics,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default, Clone)]
struct AppMetrics {
    app_name: String,
    requests: u64,
    active_connections: u64,
    bytes_sent: u64,
    bytes_received: u64,
    http_requests: u64,
    https_requests: u64,
    tunnel_requests: u64,
    ws_requests: u64,
    wss_requests: u64,
    h3_requests: u64,
    socks5_requests: u64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default, Clone)]
struct HostMetrics {
    host: String,
    requests: u64,
    active_connections: u64,
    bytes_sent: u64,
    bytes_received: u64,
    http_requests: u64,
    https_requests: u64,
    tunnel_requests: u64,
    ws_requests: u64,
    wss_requests: u64,
    h3_requests: u64,
    socks5_requests: u64,
}

#[derive(Debug, Deserialize, Clone)]
struct RuleGroup {
    name: String,
    enabled: bool,
    rule_count: usize,
}

#[derive(Debug, Deserialize, Clone)]
struct Value {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize, Clone)]
struct ValuesResponse {
    values: Vec<Value>,
    #[allow(dead_code)]
    total: usize,
}

#[derive(Debug, Deserialize, Clone)]
struct Script {
    name: String,
    #[allow(dead_code)]
    script_type: String,
}

#[derive(Debug, Deserialize, Clone)]
struct ScriptsResponse {
    request: Vec<Script>,
    response: Vec<Script>,
}

#[derive(Debug, Deserialize, Clone)]
struct ConfigResponse {
    tls: TlsConfig,
    port: u16,
    host: String,
}

#[derive(Debug, Deserialize, Clone)]
struct CliProxyStatus {
    enabled: bool,
    shell: String,
    config_files: Vec<String>,
    proxy_url: String,
}

#[derive(Debug, Deserialize, Clone)]
struct TlsConfig {
    enable_tls_interception: bool,
    intercept_include: Vec<String>,
    app_intercept_include: Vec<String>,
    unsafe_ssl: bool,
}

#[derive(Debug, Deserialize, Clone)]
struct TrafficConfig {
    max_records: usize,
    max_db_size_bytes: u64,
    max_body_memory_size: usize,
    max_body_buffer_size: usize,
    max_body_probe_size: usize,
    binary_traffic_performance_mode: bool,
    file_retention_days: u64,
    sse_stream_flush_bytes: usize,
    sse_stream_flush_interval_ms: u64,
    ws_payload_flush_bytes: usize,
    ws_payload_flush_interval_ms: u64,
    ws_payload_max_open_files: usize,
}

#[derive(Debug, Deserialize, Clone)]
struct BodyStoreStats {
    file_count: usize,
    total_size: u64,
}

#[derive(Debug, Deserialize, Clone)]
struct FrameStoreStats {
    connection_count: usize,
    total_size: u64,
}

#[derive(Debug, Deserialize, Clone)]
struct PerformanceConfigResponse {
    traffic: TrafficConfig,
    body_store_stats: Option<BodyStoreStats>,
    frame_store_stats: Option<FrameStoreStats>,
}

const SLOW_REFRESH_INTERVAL: u64 = 5;
const CPU_HISTORY_SIZE: usize = 3600;
const QPS_HISTORY_SIZE: usize = 60;

struct App {
    port: u16,
    is_running: bool,
    pid: Option<u32>,
    metrics: MetricsSnapshot,
    qps_history: Vec<f64>,
    cpu_history: Vec<f32>,
    max_cpu: f32,
    memory_used_history: Vec<u64>,
    max_memory_used: u64,
    app_metrics: Vec<AppMetrics>,
    host_metrics: Vec<HostMetrics>,
    rules: Vec<RuleGroup>,
    values: Vec<Value>,
    scripts: ScriptsResponse,
    config: Option<ConfigResponse>,
    performance_config: Option<PerformanceConfigResponse>,
    cli_proxy: Option<CliProxyStatus>,
    selected_tab: usize,
    last_update: Instant,
    last_slow_refresh: Instant,
    refresh_count: u64,
}

impl App {
    fn new() -> Self {
        Self {
            port: read_runtime_port().unwrap_or(9900),
            is_running: false,
            pid: None,
            metrics: MetricsSnapshot::default(),
            qps_history: vec![0.0; QPS_HISTORY_SIZE],
            cpu_history: vec![0.0; CPU_HISTORY_SIZE],
            max_cpu: 0.0,
            memory_used_history: vec![0; CPU_HISTORY_SIZE],
            max_memory_used: 0,
            app_metrics: Vec::new(),
            host_metrics: Vec::new(),
            rules: Vec::new(),
            values: Vec::new(),
            scripts: ScriptsResponse {
                request: Vec::new(),
                response: Vec::new(),
            },
            config: None,
            performance_config: None,
            cli_proxy: None,
            selected_tab: 0,
            last_update: Instant::now(),
            last_slow_refresh: Instant::now() - Duration::from_secs(SLOW_REFRESH_INTERVAL),
            refresh_count: 0,
        }
    }

    fn refresh(&mut self) {
        self.refresh_with_options(false);
    }

    fn refresh_with_options(&mut self, force_all: bool) {
        self.pid = read_pid();
        self.is_running = self.pid.map(is_process_running).unwrap_or(false);

        if !self.is_running {
            self.port = read_runtime_port().unwrap_or(9900);
            return;
        }

        let need_slow_refresh =
            self.last_slow_refresh.elapsed() >= Duration::from_secs(SLOW_REFRESH_INTERVAL);

        let port = self.port;
        let fetch_agg_metrics = force_all && self.selected_tab == 2;
        let (
            metrics,
            rules,
            values,
            scripts,
            config,
            performance_config,
            app_metrics,
            host_metrics,
            cli_proxy,
        ) = fetch_all_data(
            port,
            need_slow_refresh,
            self.refresh_count == 0 || force_all,
            fetch_agg_metrics,
        );

        if let Some(m) = metrics {
            self.qps_history.remove(0);
            self.qps_history.push(m.qps as f64);

            self.cpu_history.remove(0);
            self.cpu_history.push(m.cpu_usage);
            self.max_cpu = self.max_cpu.max(m.cpu_usage);

            self.memory_used_history.remove(0);
            self.memory_used_history.push(m.memory_used);
            self.max_memory_used = self.max_memory_used.max(m.memory_used);

            self.metrics = m;
        }

        if let Some(r) = rules {
            self.rules = r;
        }
        if let Some(v) = values {
            self.values = v;
        }
        if let Some(s) = scripts {
            self.scripts = s;
        }
        if let Some(c) = config {
            self.config = Some(c);
        }
        if let Some(p) = performance_config {
            self.performance_config = Some(p);
        }
        if let Some(a) = app_metrics {
            self.app_metrics = a;
        }
        if let Some(h) = host_metrics {
            self.host_metrics = h;
        }
        if let Some(s) = cli_proxy {
            self.cli_proxy = Some(s);
        }

        if need_slow_refresh {
            self.last_slow_refresh = Instant::now();
        }
        self.last_update = Instant::now();
        self.refresh_count += 1;
    }

    fn next_tab(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % 3;
        // 首次切换到 tab 时立即刷新一次，避免等待下一次 tick/slow refresh。
        // 注意：apps/hosts 属于 DB 聚合，仅在 Traffic Details(tab=2) 时触发。
        self.refresh_with_options(true);
    }

    fn prev_tab(&mut self) {
        self.selected_tab = if self.selected_tab == 0 {
            2
        } else {
            self.selected_tab - 1
        };
        // 首次切换到 tab 时立即刷新一次，避免等待下一次 tick/slow refresh。
        // 注意：apps/hosts 属于 DB 聚合，仅在 Traffic Details(tab=2) 时触发。
        self.refresh_with_options(true);
    }
}

const HTTP_TIMEOUT: Duration = Duration::from_millis(500);

type FetchAllDataResult = (
    Option<MetricsSnapshot>,
    Option<Vec<RuleGroup>>,
    Option<Vec<Value>>,
    Option<ScriptsResponse>,
    Option<ConfigResponse>,
    Option<PerformanceConfigResponse>,
    Option<Vec<AppMetrics>>,
    Option<Vec<HostMetrics>>,
    Option<CliProxyStatus>,
);

fn fetch_all_data(
    port: u16,
    need_slow_refresh: bool,
    force_all: bool,
    fetch_agg_metrics: bool,
) -> FetchAllDataResult {
    let (tx, rx) = mpsc::channel();

    let tx_metrics = tx.clone();
    thread::spawn(move || {
        let _ = tx_metrics.send(("metrics", fetch_metrics(port)));
    });

    if need_slow_refresh || force_all {
        let tx_rules = tx.clone();
        thread::spawn(move || {
            let _ = tx_rules.send(("rules", fetch_rules(port)));
        });

        let tx_values = tx.clone();
        thread::spawn(move || {
            let _ = tx_values.send(("values", fetch_values(port)));
        });

        let tx_scripts = tx.clone();
        thread::spawn(move || {
            let _ = tx_scripts.send(("scripts", fetch_scripts(port)));
        });

        let tx_config = tx.clone();
        thread::spawn(move || {
            let _ = tx_config.send(("config", fetch_config(port)));
        });

        let tx_performance = tx.clone();
        thread::spawn(move || {
            let _ = tx_performance.send(("performance", fetch_performance_config(port)));
        });

        // apps/hosts 属于 DB 聚合计算：仅在用户主动触发时请求，避免后台定时拉取导致 CPU 开销过高。
        if fetch_agg_metrics {
            let tx_apps = tx.clone();
            thread::spawn(move || {
                let _ = tx_apps.send(("apps", fetch_app_metrics(port)));
            });

            let tx_hosts = tx.clone();
            thread::spawn(move || {
                let _ = tx_hosts.send(("hosts", fetch_host_metrics(port)));
            });
        }

        let tx_cli_proxy = tx.clone();
        thread::spawn(move || {
            let _ = tx_cli_proxy.send(("cli_proxy", fetch_cli_proxy(port)));
        });
    }

    drop(tx);

    let mut metrics = None;
    let mut rules = None;
    let mut values = None;
    let mut scripts = None;
    let mut config = None;
    let mut performance = None;
    let mut app_metrics = None;
    let mut host_metrics = None;
    let mut cli_proxy = None;

    for (key, data) in rx {
        match key {
            "metrics" => metrics = data.and_then(|d| d.downcast().ok()).map(|b| *b),
            "rules" => rules = data.and_then(|d| d.downcast().ok()).map(|b| *b),
            "values" => values = data.and_then(|d| d.downcast().ok()).map(|b| *b),
            "scripts" => scripts = data.and_then(|d| d.downcast().ok()).map(|b| *b),
            "config" => config = data.and_then(|d| d.downcast().ok()).map(|b| *b),
            "performance" => performance = data.and_then(|d| d.downcast().ok()).map(|b| *b),
            "apps" => app_metrics = data.and_then(|d| d.downcast().ok()).map(|b| *b),
            "hosts" => host_metrics = data.and_then(|d| d.downcast().ok()).map(|b| *b),
            "cli_proxy" => cli_proxy = data.and_then(|d| d.downcast().ok()).map(|b| *b),
            _ => {}
        }
    }

    (
        metrics,
        rules,
        values,
        scripts,
        config,
        performance,
        app_metrics,
        host_metrics,
        cli_proxy,
    )
}

fn fetch_metrics(port: u16) -> Option<Box<dyn std::any::Any + Send>> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/metrics", port);
    let result: Option<MetricsSnapshot> = direct_agent().get(&url).call().ok()?.into_json().ok();
    result.map(|r| Box::new(r) as Box<dyn std::any::Any + Send>)
}

fn fetch_rules(port: u16) -> Option<Box<dyn std::any::Any + Send>> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/rules", port);
    let result: Option<Vec<RuleGroup>> = direct_agent().get(&url).call().ok()?.into_json().ok();
    result.map(|r| Box::new(r) as Box<dyn std::any::Any + Send>)
}

fn fetch_values(port: u16) -> Option<Box<dyn std::any::Any + Send>> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/values", port);
    let resp: Option<ValuesResponse> = direct_agent().get(&url).call().ok()?.into_json().ok();
    resp.map(|r| Box::new(r.values) as Box<dyn std::any::Any + Send>)
}

fn fetch_scripts(port: u16) -> Option<Box<dyn std::any::Any + Send>> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/scripts", port);
    let result: Option<ScriptsResponse> = direct_agent().get(&url).call().ok()?.into_json().ok();
    result.map(|r| Box::new(r) as Box<dyn std::any::Any + Send>)
}

fn fetch_config(port: u16) -> Option<Box<dyn std::any::Any + Send>> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/config", port);
    let result: Option<ConfigResponse> = direct_agent().get(&url).call().ok()?.into_json().ok();
    result.map(|r| Box::new(r) as Box<dyn std::any::Any + Send>)
}

fn fetch_performance_config(port: u16) -> Option<Box<dyn std::any::Any + Send>> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/config/performance", port);
    let result: Option<PerformanceConfigResponse> =
        direct_agent().get(&url).call().ok()?.into_json().ok();
    result.map(|r| Box::new(r) as Box<dyn std::any::Any + Send>)
}

fn fetch_app_metrics(port: u16) -> Option<Box<dyn std::any::Any + Send>> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/metrics/apps", port);
    let result: Option<Vec<AppMetrics>> = direct_agent().get(&url).call().ok()?.into_json().ok();
    result.map(|r| Box::new(r) as Box<dyn std::any::Any + Send>)
}

fn fetch_host_metrics(port: u16) -> Option<Box<dyn std::any::Any + Send>> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/metrics/hosts", port);
    let result: Option<Vec<HostMetrics>> = direct_agent().get(&url).call().ok()?.into_json().ok();
    result.map(|r| Box::new(r) as Box<dyn std::any::Any + Send>)
}

fn fetch_cli_proxy(port: u16) -> Option<Box<dyn std::any::Any + Send>> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/proxy/cli", port);
    let result: Option<CliProxyStatus> = direct_agent().get(&url).call().ok()?.into_json().ok();
    result.map(|r| Box::new(r) as Box<dyn std::any::Any + Send>)
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_rate(rate: f32) -> String {
    const KB: f32 = 1024.0;
    const MB: f32 = KB * 1024.0;

    if rate >= MB {
        format!("{:.2} MB/s", rate / MB)
    } else if rate >= KB {
        format!("{:.2} KB/s", rate / KB)
    } else {
        format!("{:.0} B/s", rate)
    }
}

fn format_time_span(seconds: usize) -> String {
    if seconds >= 3600 {
        let hours = seconds / 3600;
        let mins = (seconds % 3600) / 60;
        format!("{}h{}m", hours, mins)
    } else if seconds >= 60 {
        let mins = seconds / 60;
        let secs = seconds % 60;
        format!("{}m{}s", mins, secs)
    } else {
        format!("{}s", seconds)
    }
}

pub fn run_status_tui() -> bifrost_core::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();
    app.refresh();

    let tick_rate = Duration::from_millis(1000);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| ui(frame, &app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Tab | KeyCode::Right => app.next_tab(),
                        KeyCode::BackTab | KeyCode::Left => app.prev_tab(),
                        KeyCode::Char('r') => app.refresh_with_options(true),
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.refresh();
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

fn ui(frame: &mut Frame, app: &App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_header(frame, main_layout[0], app);
    render_content(frame, main_layout[1], app);
    render_footer(frame, main_layout[2]);
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let status = if app.is_running {
        Span::styled(" ● Running ", Style::default().fg(Color::Green).bold())
    } else {
        Span::styled(" ○ Stopped ", Style::default().fg(Color::Red).bold())
    };

    let pid_info = app.pid.map(|p| format!("PID: {}", p)).unwrap_or_default();

    let tabs = vec!["Overview", "Rules & Config", "Traffic Details"];
    let tabs_widget = Tabs::new(tabs)
        .block(Block::default().borders(Borders::ALL).title(vec![
            Span::raw(" Bifrost Status "),
            status,
            Span::styled(
                format!(" {} ", pid_info),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .select(app.selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).bold());

    frame.render_widget(tabs_widget, area);
}

fn render_content(frame: &mut Frame, area: Rect, app: &App) {
    if !app.is_running {
        let msg = Paragraph::new("Server is not running. Start with: bifrost start -d")
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(msg, area);
        return;
    }

    match app.selected_tab {
        0 => render_overview(frame, area, app),
        1 => render_rules_config(frame, area, app),
        2 => render_traffic_details(frame, area, app),
        _ => {}
    }
}

fn render_overview(frame: &mut Frame, area: Rect, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Min(0),
        ])
        .split(area);

    render_system_metrics(frame, layout[0], app);
    render_cpu_memory_sparklines(frame, layout[1], app);
    render_qps_sparkline(frame, layout[2], app);
    render_connection_stats(frame, layout[3], app);
}

fn render_system_metrics(frame: &mut Frame, area: Rect, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" CPU "))
        .gauge_style(Style::default().fg(Color::Cyan))
        .percent(app.metrics.cpu_usage.min(100.0) as u16)
        .label(format!("{:.1}%", app.metrics.cpu_usage));
    frame.render_widget(cpu_gauge, layout[0]);

    let mem_percent =
        (app.metrics.memory_used as f64 / app.metrics.memory_total.max(1) as f64 * 100.0) as u16;
    let mem_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Memory "))
        .gauge_style(Style::default().fg(Color::Magenta))
        .percent(mem_percent.min(100))
        .label(format!(
            "{} / {}",
            format_bytes(app.metrics.memory_used),
            format_bytes(app.metrics.memory_total)
        ));
    frame.render_widget(mem_gauge, layout[1]);

    let upload_block = Block::default().borders(Borders::ALL).title(" Upload ↑ ");
    let upload_text = vec![
        Line::from(format!(
            "Rate: {}",
            format_rate(app.metrics.bytes_sent_rate)
        )),
        Line::from(format!("Total: {}", format_bytes(app.metrics.bytes_sent))),
        Line::from(format!(
            "Max: {}",
            format_rate(app.metrics.max_bytes_sent_rate)
        )),
    ];
    let upload = Paragraph::new(upload_text)
        .block(upload_block)
        .style(Style::default().fg(Color::Green));
    frame.render_widget(upload, layout[2]);

    let download_block = Block::default().borders(Borders::ALL).title(" Download ↓ ");
    let download_text = vec![
        Line::from(format!(
            "Rate: {}",
            format_rate(app.metrics.bytes_received_rate)
        )),
        Line::from(format!(
            "Total: {}",
            format_bytes(app.metrics.bytes_received)
        )),
        Line::from(format!(
            "Max: {}",
            format_rate(app.metrics.max_bytes_received_rate)
        )),
    ];
    let download = Paragraph::new(download_text)
        .block(download_block)
        .style(Style::default().fg(Color::Blue));
    frame.render_widget(download, layout[3]);
}

fn render_cpu_sparkline(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width.saturating_sub(2) as usize;
    let data: Vec<u64> = if width > 0 && !app.cpu_history.is_empty() {
        let step = app.cpu_history.len() / width.max(1);
        let step = step.max(1);
        app.cpu_history
            .iter()
            .rev()
            .step_by(step)
            .take(width)
            .map(|&v| (v * 10.0) as u64)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    } else {
        vec![0; width]
    };

    let total_samples = app.cpu_history.iter().filter(|&&v| v > 0.0).count();
    let time_span = format_time_span(total_samples);

    let sparkline = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(format!(
            " CPU: {:.1}% (max: {:.1}%) | {} ",
            app.metrics.cpu_usage, app.max_cpu, time_span
        )))
        .data(&data)
        .max(1000)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(sparkline, area);
}

fn render_memory_sparkline(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width.saturating_sub(2) as usize;
    let data: Vec<u64> = if width > 0 && !app.memory_used_history.is_empty() {
        let step = app.memory_used_history.len() / width.max(1);
        let step = step.max(1);
        app.memory_used_history
            .iter()
            .rev()
            .step_by(step)
            .take(width)
            .map(|&v| v / (1024 * 1024))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    } else {
        vec![0; width]
    };

    let total_samples = app.memory_used_history.iter().filter(|&&v| v > 0).count();
    let time_span = format_time_span(total_samples);

    let max_mb = (app.max_memory_used / (1024 * 1024)).max(1);
    let sparkline = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Memory: {} / {} (max: {}) | {} ",
            format_bytes(app.metrics.memory_used),
            format_bytes(app.metrics.memory_total),
            format_bytes(app.max_memory_used),
            time_span
        )))
        .data(&data)
        .max(max_mb)
        .style(Style::default().fg(Color::Magenta));
    frame.render_widget(sparkline, area);
}

fn render_cpu_memory_sparklines(frame: &mut Frame, area: Rect, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_cpu_sparkline(frame, layout[0], app);
    render_memory_sparkline(frame, layout[1], app);
}

fn render_qps_sparkline(frame: &mut Frame, area: Rect, app: &App) {
    let data: Vec<u64> = app.qps_history.iter().map(|&v| v as u64).collect();
    let max_qps = app.metrics.max_qps.max(1.0);

    let sparkline = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(format!(
            " QPS: {:.1} (max: {:.1}) | last 60s ",
            app.metrics.qps, max_qps
        )))
        .data(&data)
        .max(max_qps as u64)
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(sparkline, area);
}

fn config_lines(app: &App) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    lines.push(Line::from("Proxy:"));
    if let Some(config) = &app.config {
        lines.push(Line::from(format!(
            "  Listen: {}:{}",
            config.host, config.port
        )));
        lines.push(Line::from(format!(
            "  TLS Interception: {}",
            if config.tls.enable_tls_interception {
                "Enabled"
            } else {
                "Disabled"
            }
        )));
        lines.push(Line::from(format!(
            "  Unsafe SSL: {}",
            config.tls.unsafe_ssl
        )));
        lines.push(Line::from("  Intercept Domains:"));
        lines.push(Line::from(format!(
            "    {}",
            if config.tls.intercept_include.is_empty() {
                "(none)".to_string()
            } else {
                config.tls.intercept_include.join(", ")
            }
        )));
        lines.push(Line::from("  Intercept Apps:"));
        lines.push(Line::from(format!(
            "    {}",
            if config.tls.app_intercept_include.is_empty() {
                "(none)".to_string()
            } else {
                config.tls.app_intercept_include.join(", ")
            }
        )));
    } else {
        lines.push(Line::from("  Loading..."));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("CLI Proxy (ENV):"));
    if let Some(cli) = &app.cli_proxy {
        lines.push(Line::from(format!(
            "  Status: {}",
            if cli.enabled { "Enabled" } else { "Disabled" }
        )));
        lines.push(Line::from(format!("  Proxy URL: {}", cli.proxy_url)));
        lines.push(Line::from(format!("  Shell: {}", cli.shell)));
        lines.push(Line::from(format!(
            "  Config Files: {}",
            cli.config_files.len()
        )));
    } else {
        lines.push(Line::from("  Loading..."));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Performance:"));
    if let Some(perf) = &app.performance_config {
        lines.push(Line::from(format!(
            "  Max Records: {}",
            perf.traffic.max_records
        )));
        lines.push(Line::from(format!(
            "  Max DB Size: {}",
            format_bytes(perf.traffic.max_db_size_bytes)
        )));
        lines.push(Line::from(format!(
            "  Max Body Inline (DB): {}",
            format_bytes(perf.traffic.max_body_memory_size as u64)
        )));
        lines.push(Line::from(format!(
            "  Max Body Buffer: {}",
            format_bytes(perf.traffic.max_body_buffer_size as u64)
        )));
        lines.push(Line::from(format!(
            "  Retention Days: {}",
            perf.traffic.file_retention_days
        )));
        lines.push(Line::from(format!(
            "  SSE Flush: {} / {}ms",
            format_bytes(perf.traffic.sse_stream_flush_bytes as u64),
            perf.traffic.sse_stream_flush_interval_ms
        )));
        lines.push(Line::from(format!(
            "  WS Flush: {} / {}ms",
            format_bytes(perf.traffic.ws_payload_flush_bytes as u64),
            perf.traffic.ws_payload_flush_interval_ms
        )));
        lines.push(Line::from(format!(
            "  WS Max Files: {}",
            perf.traffic.ws_payload_max_open_files
        )));
        if let Some(stats) = &perf.body_store_stats {
            lines.push(Line::from(format!(
                "  Body Store: {} files, {}",
                stats.file_count,
                format_bytes(stats.total_size)
            )));
        }
        if let Some(stats) = &perf.frame_store_stats {
            lines.push(Line::from(format!(
                "  Frame Store: {} conns, {}",
                stats.connection_count,
                format_bytes(stats.total_size)
            )));
        }
    } else {
        lines.push(Line::from("  Loading..."));
    }

    lines
}

fn render_connection_stats(frame: &mut Frame, area: Rect, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(area);

    let stats_items = vec![
        ListItem::new(format!("Total Requests: {}", app.metrics.total_requests)),
        ListItem::new(format!(
            "Active Connections: {}",
            app.metrics.active_connections
        )),
        ListItem::new(""),
        ListItem::new(format!("HTTP:   {} reqs", app.metrics.http.requests)),
        ListItem::new(format!("HTTPS:  {} reqs", app.metrics.https.requests)),
        ListItem::new(format!("Tunnel: {} reqs", app.metrics.tunnel.requests)),
        ListItem::new(format!("WS:     {} reqs", app.metrics.ws.requests)),
        ListItem::new(format!("WSS:    {} reqs", app.metrics.wss.requests)),
        ListItem::new(format!("SOCKS5: {} reqs", app.metrics.socks5.requests)),
    ];

    let stats_list =
        List::new(stats_items).block(Block::default().borders(Borders::ALL).title(" Statistics "));
    frame.render_widget(stats_list, layout[0]);

    let enabled_rules: Vec<_> = app.rules.iter().filter(|r| r.enabled).collect();
    let total_rules: usize = enabled_rules.iter().map(|r| r.rule_count).sum();

    let summary_items = vec![
        ListItem::new(format!("Rule Groups: {}", app.rules.len())),
        ListItem::new(format!(
            "  Enabled: {} ({} rules)",
            enabled_rules.len(),
            total_rules
        )),
        ListItem::new(format!(
            "  Disabled: {}",
            app.rules.len() - enabled_rules.len()
        )),
        ListItem::new(""),
        ListItem::new(format!("Values: {}", app.values.len())),
        ListItem::new(format!(
            "Scripts: {} req / {} res",
            app.scripts.request.len(),
            app.scripts.response.len()
        )),
        ListItem::new(""),
        ListItem::new(format!(
            "TLS Interception: {}",
            app.config
                .as_ref()
                .map(|c| if c.tls.enable_tls_interception {
                    "Enabled"
                } else {
                    "Disabled"
                })
                .unwrap_or("N/A")
        )),
        ListItem::new(format!(
            "CLI Proxy: {}",
            app.cli_proxy
                .as_ref()
                .map(|s| if s.enabled { "Enabled" } else { "Disabled" })
                .unwrap_or("N/A")
        )),
    ];

    let summary_list =
        List::new(summary_items).block(Block::default().borders(Borders::ALL).title(" Summary "));
    frame.render_widget(summary_list, layout[1]);

    let config_para = Paragraph::new(config_lines(app)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Configuration "),
    );
    frame.render_widget(config_para, layout[2]);
}

fn render_rules_config(frame: &mut Frame, area: Rect, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(layout[0]);

    let rules_rows: Vec<Row> = app
        .rules
        .iter()
        .map(|r| {
            let status = if r.enabled { "●" } else { "○" };
            let style = if r.enabled {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Row::new(vec![
                status.to_string(),
                r.name.clone(),
                r.rule_count.to_string(),
            ])
            .style(style)
        })
        .collect();

    let rules_table = Table::new(
        rules_rows,
        [
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["", "Name", "Rules"])
            .style(Style::default().fg(Color::Yellow).bold())
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Rule Groups "),
    );
    frame.render_widget(rules_table, left_layout[0]);

    let values_items: Vec<ListItem> = app
        .values
        .iter()
        .take(10)
        .map(|v| ListItem::new(format!("{}: {}", v.name, v.value)))
        .collect();

    let values_list = List::new(values_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Values ({}) ", app.values.len())),
    );
    frame.render_widget(values_list, left_layout[1]);

    let right_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(layout[1]);

    let mut script_items: Vec<ListItem> = Vec::new();
    if !app.scripts.request.is_empty() {
        script_items.push(ListItem::new("Request Scripts:").style(Style::default().bold()));
        for s in &app.scripts.request {
            script_items.push(ListItem::new(format!("  • {}", s.name)));
        }
    }
    if !app.scripts.response.is_empty() {
        script_items.push(ListItem::new("Response Scripts:").style(Style::default().bold()));
        for s in &app.scripts.response {
            script_items.push(ListItem::new(format!("  • {}", s.name)));
        }
    }
    if script_items.is_empty() {
        script_items.push(ListItem::new("No scripts configured"));
    }

    let scripts_list =
        List::new(script_items).block(Block::default().borders(Borders::ALL).title(" Scripts "));
    frame.render_widget(scripts_list, right_layout[0]);

    let config_para = Paragraph::new(config_lines(app)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Configuration "),
    );
    frame.render_widget(config_para, right_layout[1]);
}

fn render_traffic_details(frame: &mut Frame, area: Rect, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let protocols = [
        ("HTTP", &app.metrics.http, Color::Blue),
        ("HTTPS", &app.metrics.https, Color::Green),
        ("Tunnel", &app.metrics.tunnel, Color::Yellow),
        ("WebSocket", &app.metrics.ws, Color::Magenta),
        ("WSS", &app.metrics.wss, Color::Cyan),
        ("SOCKS5", &app.metrics.socks5, Color::Red),
    ];

    let rows: Vec<Row> = protocols
        .iter()
        .map(|(name, m, color)| {
            Row::new(vec![
                name.to_string(),
                m.requests.to_string(),
                m.active_connections.to_string(),
                format_bytes(m.bytes_sent),
                format_bytes(m.bytes_received),
            ])
            .style(Style::default().fg(*color))
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(15),
            Constraint::Length(15),
        ],
    )
    .header(
        Row::new(vec!["Protocol", "Requests", "Active", "Sent", "Received"])
            .style(Style::default().fg(Color::Yellow).bold())
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Traffic by Protocol "),
    );

    frame.render_widget(table, layout[0]);

    let detail_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[1]);

    let app_rows: Vec<Row> = app
        .app_metrics
        .iter()
        .take(8)
        .map(|m| {
            Row::new(vec![
                m.app_name.clone(),
                m.requests.to_string(),
                m.active_connections.to_string(),
                m.socks5_requests.to_string(),
            ])
        })
        .collect();

    let apps_table = Table::new(
        app_rows,
        [
            Constraint::Min(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Application", "Requests", "Active", "SOCKS5"])
            .style(Style::default().fg(Color::Yellow).bold())
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Top Applications "),
    );
    frame.render_widget(apps_table, detail_layout[0]);

    let host_rows: Vec<Row> = app
        .host_metrics
        .iter()
        .take(8)
        .map(|m| {
            Row::new(vec![
                m.host.clone(),
                m.requests.to_string(),
                m.active_connections.to_string(),
                m.socks5_requests.to_string(),
            ])
        })
        .collect();

    let hosts_table = Table::new(
        host_rows,
        [
            Constraint::Min(12),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Host", "Requests", "Active", "SOCKS5"])
            .style(Style::default().fg(Color::Yellow).bold())
            .bottom_margin(1),
    )
    .block(Block::default().borders(Borders::ALL).title(" Top Hosts "));
    frame.render_widget(hosts_table, detail_layout[1]);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled(" q ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Quit  "),
        Span::styled(" ←/→ ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Switch Tab  "),
        Span::styled(" r ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("Refresh  "),
    ]);

    let footer = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, area);
}
