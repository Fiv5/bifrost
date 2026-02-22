use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use bifrost_admin::{
    start_async_traffic_processor, start_push_tasks, AdminState, AsyncTrafficWriter, BodyStore,
    PushManager,
};
use bifrost_core::{
    parse_rules, system_proxy::SystemProxyManager, Protocol, RequestContext, Rule,
    RulesResolver as CoreRulesResolver, ValueStore,
};
use bifrost_proxy::{
    AccessMode, ProxyConfig, ProxyServer, ResolvedRules as ProxyResolvedRules, RuleValue,
    RulesResolver as ProxyRulesResolverTrait, TlsConfig,
};
use bifrost_storage::{ConfigChangeEvent, ConfigManager, RulesStorage, ValuesStorage};
use bifrost_tls::{
    ensure_valid_ca, generate_root_ca, load_root_ca, save_root_ca, CertInstaller, CertStatus,
    DynamicCertGenerator, SniResolver,
};
use chrono::Local;
use parking_lot::{Mutex, RwLock as ParkingRwLock};
use tracing::{error, info, warn};

use crate::state::{AppState, ProxySettings, ProxyStatus, RuleEntry, ValueEntry};

struct DynamicRulesResolver {
    inner: ParkingRwLock<CoreRulesResolver>,
}

impl DynamicRulesResolver {
    fn new(rules: Vec<Rule>, values: HashMap<String, String>) -> Self {
        let inner = CoreRulesResolver::new(rules).with_values(values);
        Self {
            inner: ParkingRwLock::new(inner),
        }
    }

    fn update(&self, rules: Vec<Rule>, values: HashMap<String, String>) {
        let new_resolver = CoreRulesResolver::new(rules).with_values(values);
        let mut inner = self.inner.write();
        *inner = new_resolver;

        info!(
            target: "bifrost_gui::rules",
            "rules resolver updated with new rules"
        );
    }
}

impl ProxyRulesResolverTrait for DynamicRulesResolver {
    fn resolve_with_context(
        &self,
        url: &str,
        method: &str,
        req_headers: &std::collections::HashMap<String, String>,
        req_cookies: &std::collections::HashMap<String, String>,
    ) -> ProxyResolvedRules {
        let mut ctx = RequestContext::from_url(url);
        ctx.method = method.to_string();
        ctx.client_ip = "127.0.0.1".to_string();
        ctx.req_headers = req_headers.clone();
        ctx.req_cookies = req_cookies.clone();

        let inner = self.inner.read();
        let core_result = inner.resolve(&ctx);
        let mut result = ProxyResolvedRules::default();

        for resolved_rule in &core_result.rules {
            let protocol = resolved_rule.rule.protocol;
            let value = &resolved_rule.resolved_value;
            let pattern = &resolved_rule.rule.pattern;

            result.rules.push(RuleValue {
                pattern: pattern.clone(),
                protocol,
                value: value.clone(),
                options: HashMap::new(),
                rule_name: resolved_rule.rule.file.clone(),
                raw: Some(resolved_rule.rule.raw.clone()),
                line: resolved_rule.rule.line,
            });

            match protocol {
                Protocol::Host
                | Protocol::XHost
                | Protocol::Http
                | Protocol::Https
                | Protocol::Ws
                | Protocol::Wss => {
                    result.host = Some(value.to_string());
                    result.host_protocol = Some(protocol);
                }
                Protocol::Redirect => {
                    result.redirect = Some(value.to_string());
                }
                Protocol::StatusCode => {
                    if let Ok(code) = value.parse::<u16>() {
                        result.status_code = Some(code);
                    }
                }
                Protocol::ResBody => {
                    result.res_body = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::Ignore => {
                    result.ignored = true;
                }
                Protocol::ReqCors => {
                    result.req_cors = bifrost_proxy::CorsConfig::enable_all();
                }
                Protocol::ResCors => {
                    result.res_cors = bifrost_proxy::CorsConfig::enable_all();
                }
                Protocol::File => {
                    result.mock_file = Some(value.to_string());
                }
                Protocol::Dns => {
                    result.dns_servers.push(value.to_string());
                }
                Protocol::TlsIntercept => {
                    result.tls_intercept = Some(true);
                }
                Protocol::TlsPassthrough => {
                    result.tls_intercept = Some(false);
                }
                _ => {}
            }
        }

        result
    }
}

type SharedDynamicRulesResolver = Arc<DynamicRulesResolver>;

pub struct ProxyController {
    state: Arc<Mutex<AppState>>,
    runtime: Option<tokio::runtime::Runtime>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    system_proxy_manager: SystemProxyManager,
}

impl ProxyController {
    pub fn new(state: Arc<Mutex<AppState>>) -> Self {
        let data_dir = bifrost_storage::data_dir();
        Self {
            state,
            runtime: None,
            shutdown_tx: None,
            system_proxy_manager: SystemProxyManager::new(data_dir),
        }
    }

    pub fn start(&mut self) {
        if self.runtime.is_some() {
            return;
        }

        let settings = self.state.lock().settings.clone();
        self.state.lock().proxy_status = ProxyStatus::Starting;
        self.state.lock().error_message = None;

        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
        let state = Arc::clone(&self.state);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        rt.spawn(async move {
            if let Err(e) = run_proxy_server(settings, state.clone(), shutdown_rx).await {
                error!("Proxy server error: {}", e);
                let mut s = state.lock();
                s.proxy_status = ProxyStatus::Error;
                s.error_message = Some(e.to_string());
            }
        });

        self.runtime = Some(rt);
        self.shutdown_tx = Some(shutdown_tx);

        let mut s = self.state.lock();
        s.proxy_status = ProxyStatus::Running;
        s.started_at = Some(Local::now());
    }

    pub fn stop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        if let Some(rt) = self.runtime.take() {
            rt.shutdown_timeout(std::time::Duration::from_secs(5));
        }

        let mut s = self.state.lock();
        s.proxy_status = ProxyStatus::Stopped;
        s.started_at = None;
    }

    pub fn is_running(&self) -> bool {
        self.runtime.is_some()
    }

    pub fn check_ca_status(&self) -> Option<bool> {
        let cert_dir = get_bifrost_dir().ok()?.join("certs");
        let ca_cert_path = cert_dir.join("ca.crt");

        if !ca_cert_path.exists() {
            return Some(false);
        }

        let installer = CertInstaller::new(&ca_cert_path);
        match installer.check_status() {
            Ok(CertStatus::InstalledAndTrusted) => Some(true),
            _ => Some(false),
        }
    }

    pub fn load_rules(&self) -> Vec<RuleEntry> {
        let storage = match RulesStorage::new() {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let names = match storage.list() {
            Ok(n) => n,
            Err(_) => return Vec::new(),
        };

        names
            .into_iter()
            .filter_map(|name| {
                storage.load(&name).ok().map(|rule| RuleEntry {
                    name: rule.name,
                    enabled: rule.enabled,
                    content: rule.content,
                })
            })
            .collect()
    }

    pub fn save_rule(&self, rule: &RuleEntry) -> Result<(), String> {
        let storage = RulesStorage::new().map_err(|e| e.to_string())?;
        let rule_file = bifrost_storage::RuleFile::new(&rule.name, rule.content.clone());
        storage.save(&rule_file).map_err(|e| e.to_string())?;
        if !rule.enabled {
            storage
                .set_enabled(&rule.name, false)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn delete_rule(&self, name: &str) -> Result<(), String> {
        let storage = RulesStorage::new().map_err(|e| e.to_string())?;
        storage.delete(name).map_err(|e| e.to_string())
    }

    pub fn toggle_rule(&self, name: &str, enabled: bool) -> Result<(), String> {
        let storage = RulesStorage::new().map_err(|e| e.to_string())?;
        storage
            .set_enabled(name, enabled)
            .map_err(|e| e.to_string())
    }

    pub fn load_values(&self) -> Vec<ValueEntry> {
        let storage = match ValuesStorage::new() {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        match storage.list_entries() {
            Ok(entries) => entries
                .into_iter()
                .map(|e| ValueEntry {
                    name: e.name,
                    value: e.value,
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn save_value(&self, entry: &ValueEntry) -> Result<(), String> {
        let mut storage = ValuesStorage::new().map_err(|e| e.to_string())?;
        storage
            .set_value(&entry.name, &entry.value)
            .map_err(|e| e.to_string())
    }

    pub fn delete_value(&self, name: &str) -> Result<(), String> {
        let mut storage = ValuesStorage::new().map_err(|e| e.to_string())?;
        storage.remove_value(name).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn is_system_proxy_supported(&self) -> bool {
        SystemProxyManager::is_supported()
    }

    #[allow(dead_code)]
    pub fn is_system_proxy_enabled(&self) -> bool {
        self.system_proxy_manager.is_set()
    }

    pub fn enable_system_proxy(&mut self) -> Result<(), String> {
        let settings = self.state.lock().settings.clone();
        let mut bypass_list: Vec<String> = settings.intercept_exclude.clone();
        bypass_list.extend(
            ["localhost", "127.0.0.1", "::1", "*.local"]
                .iter()
                .map(|s| s.to_string()),
        );
        let bypass = bypass_list.join(",");

        info!(
            "Enabling system proxy: {}:{}, bypass: {}",
            settings.host, settings.port, bypass
        );

        self.system_proxy_manager
            .enable(&settings.host, settings.port, Some(&bypass))
            .map_err(|e| {
                warn!("Failed to enable system proxy: {}", e);
                e.to_string()
            })?;

        self.state.lock().system_proxy_enabled = true;
        info!("System proxy enabled successfully");
        Ok(())
    }

    pub fn disable_system_proxy(&mut self) -> Result<(), String> {
        info!("Disabling system proxy");
        self.system_proxy_manager.disable().map_err(|e| {
            warn!("Failed to disable system proxy: {}", e);
            e.to_string()
        })?;

        self.state.lock().system_proxy_enabled = false;
        info!("System proxy disabled successfully");
        Ok(())
    }

    pub fn restore_system_proxy(&mut self) -> Result<(), String> {
        if self.system_proxy_manager.is_set() {
            info!("Restoring original system proxy settings");
            self.system_proxy_manager.restore().map_err(|e| {
                warn!("Failed to restore system proxy: {}", e);
                e.to_string()
            })?;
            self.state.lock().system_proxy_enabled = false;
        }
        Ok(())
    }
}

async fn run_proxy_server(
    settings: ProxySettings,
    state: Arc<Mutex<AppState>>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let log_level = std::env::var("BIFROST_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let verbose_logging = matches!(log_level.as_str(), "debug" | "trace");

    let proxy_config = ProxyConfig {
        port: settings.port,
        host: settings.host.clone(),
        socks5_port: settings.socks5_port,
        access_mode: if settings.allow_lan {
            AccessMode::AllowAll
        } else {
            AccessMode::LocalOnly
        },
        client_whitelist: Vec::new(),
        allow_lan: settings.allow_lan,
        enable_tls_interception: settings.enable_tls_interception,
        intercept_exclude: settings.intercept_exclude.clone(),
        intercept_include: settings.intercept_include.clone(),
        unsafe_ssl: settings.unsafe_ssl,
        verbose_logging,
        ..Default::default()
    };

    let tls_config = load_tls_config(&proxy_config)?;

    let rules_storage = RulesStorage::new()?;
    let rules = load_all_rules(&rules_storage)?;

    let values_storage = ValuesStorage::new().ok();
    let values = values_storage
        .as_ref()
        .map(|s| s.as_hashmap())
        .unwrap_or_default();

    let resolver: SharedDynamicRulesResolver = Arc::new(DynamicRulesResolver::new(rules, values));

    let bifrost_dir = get_bifrost_dir()?;
    let body_temp_dir = bifrost_dir.join("body_cache");
    let body_store = Arc::new(ParkingRwLock::new(BodyStore::new(
        body_temp_dir,
        2 * 1024 * 1024,
        7,
    )));

    let traffic_dir = bifrost_dir.join("traffic");
    let traffic_store = Arc::new(bifrost_admin::TrafficStore::new(
        traffic_dir,
        5000,
        Some(24 * 7),
    ));

    let frame_store = bifrost_admin::FrameStore::new(bifrost_dir.clone(), Some(24 * 7));

    let ca_cert_path = get_bifrost_dir()
        .map(|p| p.join("certs").join("ca.crt"))
        .ok();

    let config_manager = ConfigManager::new(bifrost_dir.clone()).ok();

    let traffic_recorder = std::sync::Arc::new(bifrost_admin::TrafficRecorder::default());
    let (async_traffic_writer, async_traffic_rx) = AsyncTrafficWriter::new(10000);
    let _async_traffic_task = start_async_traffic_processor(
        async_traffic_rx,
        traffic_recorder.clone(),
        Some(traffic_store.clone()),
    );

    let mut admin_state = AdminState::new(settings.port)
        .with_body_store(body_store)
        .with_traffic_store_shared(traffic_store.clone())
        .with_traffic_recorder_shared(traffic_recorder)
        .with_async_traffic_writer(async_traffic_writer)
        .with_frame_store(frame_store);

    bifrost_admin::start_traffic_cleanup_task(traffic_store);
    if let Some(vs) = values_storage {
        admin_state = admin_state.with_values_storage(vs);
    }
    if let Some(cert_path) = ca_cert_path {
        admin_state = admin_state.with_ca_cert_path(cert_path);
    }
    if let Some(ref cm) = config_manager {
        admin_state = admin_state.with_config_manager(cm.clone());
    }

    let values_storage_for_watcher = admin_state.values_storage.clone();

    let server = ProxyServer::new(proxy_config)
        .with_tls_config(tls_config)
        .with_rules(resolver.clone())
        .with_admin_state(admin_state);

    let admin_state_arc = server
        .admin_state()
        .cloned()
        .expect("admin_state should be set");
    let push_manager = Arc::new(PushManager::new(admin_state_arc.clone()));
    let _push_tasks = start_push_tasks(push_manager.clone());
    let server = server.with_push_manager(push_manager);

    info!(
        "GUI: Proxy server starting on {}:{}",
        settings.host, settings.port
    );

    let rules_watcher_task = spawn_rules_watcher_task(
        config_manager,
        rules_storage,
        values_storage_for_watcher,
        resolver.clone(),
    );

    tokio::select! {
        result = server.run() => {
            if let Err(e) = result {
                let mut s = state.lock();
                s.proxy_status = ProxyStatus::Error;
                s.error_message = Some(e.to_string());
            }
        }
        _ = &mut shutdown_rx => {
            info!("GUI: Proxy server shutting down");
        }
    }

    rules_watcher_task.abort();

    Ok(())
}

fn get_bifrost_dir() -> Result<PathBuf, String> {
    Ok(bifrost_storage::data_dir())
}

fn load_tls_config(
    config: &ProxyConfig,
) -> Result<Arc<TlsConfig>, Box<dyn std::error::Error + Send + Sync>> {
    if !config.enable_tls_interception {
        return Ok(Arc::new(TlsConfig::default()));
    }

    let cert_dir = get_bifrost_dir()?.join("certs");
    let ca_key_path = cert_dir.join("ca.key");
    let ca_cert_path = cert_dir.join("ca.crt");

    let ca_valid = ensure_valid_ca(&ca_cert_path, &ca_key_path)?;
    if !ca_valid {
        std::fs::create_dir_all(&cert_dir)?;
        let ca = generate_root_ca()?;
        save_root_ca(&ca_cert_path, &ca_key_path, &ca)?;
    }

    let ca = load_root_ca(&ca_cert_path, &ca_key_path)?;
    let ca_cert_bytes = std::fs::read(&ca_cert_path)?;
    let ca_key_bytes = std::fs::read(&ca_key_path)?;
    let ca_arc = Arc::new(ca);
    let sni_resolver = SniResolver::new(ca_arc.clone());
    let cert_generator = DynamicCertGenerator::new(ca_arc);

    Ok(Arc::new(TlsConfig {
        ca_cert: Some(ca_cert_bytes),
        ca_key: Some(ca_key_bytes),
        cert_generator: Some(Arc::new(cert_generator)),
        sni_resolver: Some(Arc::new(sni_resolver)),
    }))
}

fn load_all_rules(
    storage: &RulesStorage,
) -> Result<Vec<Rule>, Box<dyn std::error::Error + Send + Sync>> {
    let names = storage.list()?;
    let mut all_rules = Vec::new();

    for name in names {
        if let Ok(rule_file) = storage.load(&name) {
            if rule_file.enabled {
                match parse_rules(&rule_file.content) {
                    Ok(parsed) => {
                        info!(
                            target: "bifrost_gui::rules",
                            file = %rule_file.name,
                            enabled = rule_file.enabled,
                            parsed_count = parsed.len(),
                            "loaded rule file"
                        );
                        for mut rule in parsed {
                            rule.file = Some(rule_file.name.clone());
                            all_rules.push(rule);
                        }
                    }
                    Err(e) => {
                        warn!(
                            target: "bifrost_gui::rules",
                            file = %rule_file.name,
                            error = %e,
                            "failed to parse rule file"
                        );
                    }
                }
            }
        }
    }

    if !all_rules.is_empty() {
        info!(
            target: "bifrost_gui::rules",
            total_rules = all_rules.len(),
            "loaded rules from storage"
        );
    }

    Ok(all_rules)
}

type SharedValuesStorage = Arc<ParkingRwLock<ValuesStorage>>;

fn spawn_rules_watcher_task(
    config_manager: Option<ConfigManager>,
    rules_storage: RulesStorage,
    values_storage: Option<SharedValuesStorage>,
    resolver: SharedDynamicRulesResolver,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let Some(config_manager) = config_manager else {
            warn!(
                target: "bifrost_gui::rules",
                "ConfigManager not available, rules hot-reload disabled"
            );
            return;
        };

        let mut receiver = config_manager.subscribe();
        info!(
            target: "bifrost_gui::rules",
            "rules hot-reload watcher started"
        );

        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if matches!(
                        event,
                        ConfigChangeEvent::RulesChanged | ConfigChangeEvent::ValuesChanged(_)
                    ) {
                        info!(
                            target: "bifrost_gui::rules",
                            event = ?event,
                            "config change event received, reloading rules"
                        );

                        match load_all_rules(&rules_storage) {
                            Ok(new_rules) => {
                                let new_values = values_storage
                                    .as_ref()
                                    .map(|vs| {
                                        use bifrost_core::ValueStore;
                                        vs.read().as_hashmap()
                                    })
                                    .unwrap_or_default();
                                resolver.update(new_rules, new_values);
                            }
                            Err(e) => {
                                error!(
                                    target: "bifrost_gui::rules",
                                    error = %e,
                                    "failed to reload rules"
                                );
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                    warn!(
                        target: "bifrost_gui::rules",
                        count = count,
                        "rules watcher lagged, some events may have been missed"
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!(
                        target: "bifrost_gui::rules",
                        "config change channel closed, stopping rules watcher"
                    );
                    break;
                }
            }
        }
    })
}
