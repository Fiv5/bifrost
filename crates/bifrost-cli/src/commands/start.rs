use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use bifrost_admin::{
    start_metrics_collector_task, status_printer::TlsStatusInfo, AdminState, BodyStore,
    RuntimeConfig,
};
use bifrost_core::Rule;
use bifrost_proxy::{AccessMode, ProxyConfig, ProxyServer};
use bifrost_storage::{set_data_dir, ConfigChangeEvent, ConfigManager};
use bifrost_tls::{get_platform_name, CertInstaller, CertStatus};
use parking_lot::RwLock as ParkingRwLock;
use tracing::info;

use crate::cli::Cli;
use crate::commands::ca::{check_and_install_certificate, load_tls_config};
use crate::config::get_bifrost_dir;
use crate::help::print_startup_help;
use crate::parsing::{parse_cli_rules, DynamicRulesResolver, SharedDynamicRulesResolver};
use crate::process::{is_process_running, read_pid, remove_pid, write_pid};

#[allow(clippy::too_many_arguments)]
pub fn run_start(
    cli: &Cli,
    daemon: bool,
    skip_cert_check: bool,
    access_mode: Option<String>,
    whitelist: Option<String>,
    allow_lan: bool,
    no_intercept: bool,
    intercept_exclude: Option<String>,
    intercept_include: Option<String>,
    unsafe_ssl: bool,
    no_disconnect_on_config_change: bool,
    rules: Vec<String>,
    rules_file: Option<PathBuf>,
    system_proxy: bool,
    proxy_bypass: Option<String>,
) -> bifrost_core::Result<()> {
    if let Some(pid) = read_pid() {
        if is_process_running(pid) {
            return Err(bifrost_core::BifrostError::Config(format!(
                "Bifrost proxy is already running (PID: {})",
                pid
            )));
        }
        remove_pid()?;
    }

    if !daemon && !skip_cert_check {
        check_and_install_certificate()?;
    }

    let bifrost_dir = get_bifrost_dir()?;
    set_data_dir(bifrost_dir.clone());

    let config_manager = ConfigManager::new(bifrost_dir.clone())?;
    let stored_config = futures::executor::block_on(config_manager.config());

    let parsed_access_mode = match &access_mode {
        Some(mode) => mode
            .parse::<AccessMode>()
            .map_err(bifrost_core::BifrostError::Config)?,
        None => stored_config.access.mode,
    };

    let client_whitelist: Vec<String> = match whitelist {
        Some(wl) => wl.split(',').map(|s| s.trim().to_string()).collect(),
        None => stored_config.access.whitelist.clone(),
    };

    let allow_lan_final = if allow_lan {
        true
    } else {
        stored_config.access.allow_lan
    };

    let enable_tls_interception = if no_intercept {
        false
    } else {
        stored_config.tls.enable_interception
    };

    let exclude_list: Vec<String> = match intercept_exclude {
        Some(list) => list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        None => stored_config.tls.intercept_exclude.clone(),
    };

    let include_list: Vec<String> = match intercept_include {
        Some(list) => list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        None => stored_config.tls.intercept_include.clone(),
    };

    let unsafe_ssl_final = if unsafe_ssl {
        true
    } else {
        stored_config.tls.unsafe_ssl
    };

    let verbose_logging = matches!(cli.log_level.as_str(), "debug" | "trace");
    let proxy_config = ProxyConfig {
        port: cli.port,
        host: cli.host.clone(),
        socks5_port: cli.socks5_port,
        access_mode: parsed_access_mode,
        client_whitelist,
        allow_lan: allow_lan_final,
        enable_tls_interception,
        intercept_exclude: exclude_list.clone(),
        intercept_include: include_list.clone(),
        unsafe_ssl: unsafe_ssl_final,
        verbose_logging,
        max_body_buffer_size: stored_config.traffic.max_body_buffer_size,
        ..Default::default()
    };

    println!("Access control mode: {}", proxy_config.access_mode);
    if !proxy_config.client_whitelist.is_empty() {
        println!("Client whitelist: {:?}", proxy_config.client_whitelist);
    }
    if proxy_config.allow_lan {
        println!("LAN (private network) access: enabled");
    }
    if enable_tls_interception {
        println!("TLS interception: enabled");
        if !exclude_list.is_empty() {
            println!("  Excluded domains: {:?}", exclude_list);
        }
    } else {
        println!("TLS interception: disabled");
    }
    if !include_list.is_empty() {
        println!("  Force intercept domains: {:?}", include_list);
    }
    if unsafe_ssl_final {
        println!("⚠️  WARNING: Upstream TLS certificate verification is DISABLED (--unsafe-ssl)");
    }

    let early_values = futures::executor::block_on(config_manager.values_as_hashmap());
    let values_dir = stored_config.paths.values_dir.clone();
    if !early_values.is_empty() {
        println!(
            "Loaded {} values from {}",
            early_values.len(),
            values_dir.display()
        );
    }

    let (parsed_rules, inline_values) = parse_cli_rules(&rules, &rules_file, &early_values)?;
    let mut all_values = early_values.clone();
    for (k, v) in inline_values {
        all_values.entry(k).or_insert(v);
    }
    if !parsed_rules.is_empty() {
        println!("Loaded {} rules from command line", parsed_rules.len());
        for rule in &parsed_rules {
            println!(
                "  {} {}://{}",
                rule.pattern,
                rule.protocol.to_str(),
                rule.value
            );
        }
    }

    let enable_system_proxy = if system_proxy {
        true
    } else {
        stored_config.system_proxy.enabled
    };

    let system_proxy_bypass =
        proxy_bypass.unwrap_or_else(|| stored_config.system_proxy.bypass.clone());

    if enable_system_proxy {
        if bifrost_core::SystemProxyManager::is_supported() {
            println!("System proxy: enabled (bypass: {})", system_proxy_bypass);
        } else {
            println!("⚠️  WARNING: System proxy is not supported on this platform");
        }
    }

    let disconnect_on_config_change = if no_disconnect_on_config_change {
        false
    } else {
        stored_config.tls.disconnect_on_change
    };

    if daemon {
        #[cfg(unix)]
        {
            run_daemon(
                proxy_config,
                parsed_rules,
                all_values.clone(),
                enable_system_proxy,
                system_proxy_bypass.clone(),
                config_manager,
            )?;
        }
        #[cfg(not(unix))]
        {
            return Err(bifrost_core::BifrostError::Config(
                "Daemon mode is not supported on this platform".to_string(),
            ));
        }
    } else {
        run_foreground(
            proxy_config,
            parsed_rules,
            all_values,
            enable_system_proxy,
            system_proxy_bypass,
            disconnect_on_config_change,
            config_manager,
        )?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_foreground(
    config: ProxyConfig,
    cli_rules: Vec<Rule>,
    cli_values: HashMap<String, String>,
    enable_system_proxy: bool,
    system_proxy_bypass: String,
    disconnect_on_config_change: bool,
    config_manager: ConfigManager,
) -> bifrost_core::Result<()> {
    let pid = std::process::id();
    write_pid(pid)?;

    print_startup_help(config.port);

    println!("════════════════════════════════════════════════════════════════════════");
    println!("                           SERVER STATUS");
    println!("════════════════════════════════════════════════════════════════════════");

    let tls_config = load_tls_config(&config)?;

    let bifrost_dir = config_manager.data_dir().to_path_buf();
    let system_proxy_manager = std::sync::Arc::new(tokio::sync::RwLock::new(
        bifrost_core::SystemProxyManager::new(bifrost_dir.clone()),
    ));

    if let Err(e) = bifrost_core::SystemProxyManager::recover_from_crash(&bifrost_dir) {
        tracing::warn!("Failed to recover system proxy from previous crash: {}", e);
    }

    let mut system_proxy_enabled = false;
    if enable_system_proxy {
        let proxy_host = if config.host == "0.0.0.0" {
            "127.0.0.1".to_string()
        } else {
            config.host.clone()
        };
        let mut manager = system_proxy_manager.blocking_write();
        let result = manager.enable(&proxy_host, config.port, Some(&system_proxy_bypass));

        let final_result = match &result {
            Ok(()) => result,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("RequiresAdmin") {
                    println!(
                        "  ⚠ System proxy requires admin privileges, requesting authorization..."
                    );
                    #[cfg(target_os = "macos")]
                    {
                        manager.enable_with_gui_auth(
                            &proxy_host,
                            config.port,
                            Some(&system_proxy_bypass),
                        )
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        result
                    }
                } else {
                    result
                }
            }
        };

        match final_result {
            Ok(()) => {
                system_proxy_enabled = true;
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("UserCancelled") {
                    println!("  ⚠ System proxy not enabled (authorization cancelled)");
                } else if msg.contains("RequiresAdmin") {
                    println!("  ⚠ System proxy requires admin privileges (not enabled)");
                } else {
                    eprintln!("  ✗ Failed to enable system proxy: {}", e);
                }
            }
        }
    }

    let admin_host = if config.host == "0.0.0.0" {
        "127.0.0.1"
    } else {
        &config.host
    };

    println!();
    println!("📡 NETWORK");
    println!("   HTTP Proxy:    {}:{}", config.host, config.port);
    if let Some(socks5_port) = config.socks5_port {
        println!("   SOCKS5 Proxy:  {}:{}", config.host, socks5_port);
    }
    println!("   Admin UI:      http://{}:{}/", admin_host, config.port);

    println!();
    let tls_status = TlsStatusInfo {
        enable_tls_interception: config.enable_tls_interception,
        intercept_exclude: config.intercept_exclude.clone(),
        intercept_include: config.intercept_include.clone(),
        unsafe_ssl: config.unsafe_ssl,
        disconnect_on_config_change,
        active_connections: 0,
    };
    tls_status.print_status();

    println!();
    println!("🔐 CA CERTIFICATE");
    let cert_dir = bifrost_dir.join("certs");
    let ca_cert_path = cert_dir.join("ca.crt");
    if ca_cert_path.exists() {
        let installer = CertInstaller::new(&ca_cert_path);
        match installer.check_status() {
            Ok(CertStatus::InstalledAndTrusted) => {
                println!("   Status:        ✓ Installed and trusted");
            }
            Ok(CertStatus::InstalledNotTrusted) => {
                println!("   Status:        ⚠ Installed but NOT trusted");
                println!("   Action:        Run 'bifrost ca info' for details");
            }
            Ok(CertStatus::NotInstalled) => {
                println!("   Status:        ✗ Not installed in system trust store");
                println!("   Action:        Run 'bifrost ca info' to install");
            }
            Err(_) => {
                println!("   Status:        ? Unable to check");
            }
        }
        println!("   Certificate:   {}", ca_cert_path.display());
    } else {
        println!("   Status:        Not generated");
    }

    println!();
    println!("🌐 SYSTEM PROXY");
    if system_proxy_enabled {
        println!("   Status:        ✓ Enabled");
        println!("   Bypass:        {}", system_proxy_bypass);
    } else if enable_system_proxy {
        println!("   Status:        ⚠ Requested but not enabled");
    } else {
        println!("   Status:        Disabled");
    }

    println!();
    println!("🛡️  ACCESS CONTROL");
    println!("   Mode:          {}", config.access_mode);
    if !config.client_whitelist.is_empty() {
        println!("   Whitelist:     {:?}", config.client_whitelist);
    }
    println!(
        "   LAN Access:    {}",
        if config.allow_lan {
            "enabled"
        } else {
            "disabled"
        }
    );

    println!();
    println!("📂 DATA DIRECTORY");
    println!("   Path:          {}", bifrost_dir.display());
    let custom_dir = std::env::var("BIFROST_DATA_DIR").ok();
    if custom_dir.is_some() {
        println!("   Source:        BIFROST_DATA_DIR environment variable");
    } else {
        println!("   Source:        Default (~/.bifrost)");
    }

    println!();
    println!("⚙️  PROCESS");
    println!("   PID:           {}", pid);
    println!("   Platform:      {}", get_platform_name());

    println!();
    println!("────────────────────────────────────────────────────────────────────────");
    println!("Press Ctrl+C to stop");
    println!("────────────────────────────────────────────────────────────────────────");
    println!();

    let rt = tokio::runtime::Runtime::new().map_err(|e| {
        bifrost_core::BifrostError::Config(format!("Failed to create runtime: {}", e))
    })?;

    rt.block_on(async {
        let stored_config = config_manager.config().await;
        let body_temp_dir = bifrost_dir.join("body_cache");
        let body_store = Arc::new(ParkingRwLock::new(BodyStore::new(
            body_temp_dir,
            stored_config.traffic.max_body_memory_size,
            stored_config.traffic.file_retention_days,
        )));

        let values_storage = config_manager.values_storage().await;
        let rules_storage = config_manager.rules_storage().await;
        let mut values = {
            use bifrost_core::ValueStore;
            values_storage.as_hashmap()
        };
        for (k, v) in cli_values {
            values.entry(k).or_insert(v);
        }

        let ca_cert_path = bifrost_dir.join("certs").join("ca.crt");

        let runtime_config = RuntimeConfig {
            enable_tls_interception: config.enable_tls_interception,
            intercept_exclude: config.intercept_exclude.clone(),
            intercept_include: config.intercept_include.clone(),
            unsafe_ssl: config.unsafe_ssl,
            disconnect_on_config_change,
        };
        let connection_registry =
            bifrost_admin::ConnectionRegistry::new(disconnect_on_config_change);

        let admin_state = AdminState::new(config.port)
            .with_body_store(body_store)
            .with_runtime_config(runtime_config)
            .with_connection_registry(connection_registry)
            .with_values_storage(values_storage)
            .with_rules_storage(rules_storage)
            .with_ca_cert_path(ca_cert_path)
            .with_system_proxy_manager_shared(system_proxy_manager.clone())
            .with_config_manager(config_manager)
            .with_max_body_buffer_size(stored_config.traffic.max_body_buffer_size);

        let metrics_collector = admin_state.metrics_collector.clone();
        let rules_storage_for_resolver = admin_state.rules_storage.clone();
        let config_manager_for_resolver = admin_state.config_manager.clone();
        let values_storage_for_resolver = admin_state
            .values_storage
            .clone()
            .expect("values_storage should be set");
        let connection_registry_for_resolver = admin_state.connection_registry.clone();
        let runtime_config_for_resolver = admin_state.runtime_config.clone();

        let stored_rules = load_stored_rules(&rules_storage_for_resolver);
        let resolver: SharedDynamicRulesResolver = Arc::new(DynamicRulesResolver::new(
            cli_rules,
            stored_rules,
            values.clone(),
        ));

        log_resolver_rules(&resolver);

        let server = ProxyServer::new(config)
            .with_tls_config(tls_config)
            .with_admin_state(admin_state)
            .with_rules(resolver.clone());

        let _metrics_task = start_metrics_collector_task(metrics_collector, 1);

        let rules_watcher_task = spawn_rules_watcher_task(
            config_manager_for_resolver,
            rules_storage_for_resolver,
            values_storage_for_resolver,
            resolver.clone(),
            connection_registry_for_resolver,
            runtime_config_for_resolver,
        );

        tokio::select! {
            result = server.run() => {
                if let Err(e) = result {
                    eprintln!("Server error: {}", e);
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal");
                println!("\nShutting down...");
            }
        }

        rules_watcher_task.abort();
    });

    if let Err(e) = system_proxy_manager.blocking_write().restore() {
        eprintln!("Failed to restore system proxy: {}", e);
    }

    remove_pid()?;
    println!("Bifrost proxy stopped.");
    Ok(())
}

#[cfg(unix)]
pub fn run_daemon(
    config: ProxyConfig,
    cli_rules: Vec<Rule>,
    cli_values: HashMap<String, String>,
    enable_system_proxy: bool,
    system_proxy_bypass: String,
    config_manager: ConfigManager,
) -> bifrost_core::Result<()> {
    use nix::unistd::{chdir, dup2, fork, setsid, ForkResult};
    use std::os::unix::io::AsRawFd;

    use crate::process::get_pid_file;

    let bifrost_dir = config_manager.data_dir().to_path_buf();
    std::fs::create_dir_all(&bifrost_dir)?;

    println!("Starting Bifrost proxy in daemon mode...");
    println!("HTTP proxy: {}:{}", config.host, config.port);
    if let Some(socks5_port) = config.socks5_port {
        println!("SOCKS5 proxy: {}:{}", config.host, socks5_port);
    }
    println!("PID file: {}", get_pid_file()?.display());
    println!("Log file: {}", bifrost_dir.join("bifrost.log").display());

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            println!("Daemon started with PID: {}", child);
            Ok(())
        }
        Ok(ForkResult::Child) => {
            setsid().map_err(|e| {
                bifrost_core::BifrostError::Config(format!("Failed to create new session: {}", e))
            })?;

            let _ = chdir(&bifrost_dir);

            let log_file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(bifrost_dir.join("bifrost.log"))
                .map_err(|e| {
                    bifrost_core::BifrostError::Config(format!("Failed to open log file: {}", e))
                })?;
            let err_file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(bifrost_dir.join("bifrost.err"))
                .map_err(|e| {
                    bifrost_core::BifrostError::Config(format!(
                        "Failed to open error log file: {}",
                        e
                    ))
                })?;

            let _ = dup2(log_file.as_raw_fd(), 1);
            let _ = dup2(err_file.as_raw_fd(), 2);

            let pid = std::process::id();
            write_pid(pid)?;

            let tls_config = load_tls_config(&config)?;

            let system_proxy_manager = std::sync::Arc::new(tokio::sync::RwLock::new(
                bifrost_core::SystemProxyManager::new(bifrost_dir.clone()),
            ));

            if let Err(e) = bifrost_core::SystemProxyManager::recover_from_crash(&bifrost_dir) {
                tracing::warn!("Failed to recover system proxy from previous crash: {}", e);
            }

            if enable_system_proxy {
                let proxy_host = if config.host == "0.0.0.0" {
                    "127.0.0.1".to_string()
                } else {
                    config.host.clone()
                };
                let mut manager = system_proxy_manager.blocking_write();
                let result = manager.enable(&proxy_host, config.port, Some(&system_proxy_bypass));

                let final_result = match &result {
                    Ok(()) => result,
                    Err(e) => {
                        let msg = e.to_string();
                        if msg.contains("RequiresAdmin") {
                            println!("System proxy requires admin privileges, requesting authorization...");
                            #[cfg(target_os = "macos")]
                            {
                                manager.enable_with_gui_auth(
                                    &proxy_host,
                                    config.port,
                                    Some(&system_proxy_bypass),
                                )
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                result
                            }
                        } else {
                            result
                        }
                    }
                };

                if let Err(e) = final_result {
                    let msg = e.to_string();
                    if msg.contains("UserCancelled") {
                        println!("System proxy not enabled (authorization cancelled)");
                    } else if msg.contains("RequiresAdmin") {
                        println!("System proxy requires administrator privileges; daemon will continue without changing system proxy. You can toggle it later via CLI or Admin UI.");
                    } else {
                        eprintln!("Failed to enable system proxy: {}", e);
                    }
                }
            }

            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                bifrost_core::BifrostError::Config(format!("Failed to create runtime: {}", e))
            })?;

            rt.block_on(async {
                let stored_config = config_manager.config().await;
                let body_temp_dir = bifrost_dir.join("body_cache");
                let body_store = Arc::new(ParkingRwLock::new(BodyStore::new(
                    body_temp_dir,
                    stored_config.traffic.max_body_memory_size,
                    stored_config.traffic.file_retention_days,
                )));

                let values_storage = config_manager.values_storage().await;
                let rules_storage = config_manager.rules_storage().await;
                let mut values = {
                    use bifrost_core::ValueStore;
                    values_storage.as_hashmap()
                };
                for (k, v) in cli_values {
                    values.entry(k).or_insert(v);
                }

                let ca_cert_path = bifrost_dir.join("certs").join("ca.crt");

                let runtime_config = RuntimeConfig {
                    enable_tls_interception: config.enable_tls_interception,
                    intercept_exclude: config.intercept_exclude.clone(),
                    intercept_include: config.intercept_include.clone(),
                    unsafe_ssl: config.unsafe_ssl,
                    disconnect_on_config_change: true,
                };
                let connection_registry = bifrost_admin::ConnectionRegistry::new(true);

                let admin_state = AdminState::new(config.port)
                    .with_body_store(body_store)
                    .with_runtime_config(runtime_config)
                    .with_connection_registry(connection_registry)
                    .with_values_storage(values_storage)
                    .with_rules_storage(rules_storage)
                    .with_ca_cert_path(ca_cert_path)
                    .with_system_proxy_manager_shared(system_proxy_manager.clone())
                    .with_config_manager(config_manager)
                    .with_max_body_buffer_size(stored_config.traffic.max_body_buffer_size);

                let metrics_collector = admin_state.metrics_collector.clone();
                let rules_storage_for_resolver = admin_state.rules_storage.clone();
                let config_manager_for_resolver = admin_state.config_manager.clone();
                let values_storage_for_resolver = admin_state
                    .values_storage
                    .clone()
                    .expect("values_storage should be set");
                let connection_registry_for_resolver = admin_state.connection_registry.clone();
                let runtime_config_for_resolver = admin_state.runtime_config.clone();

                let stored_rules = load_stored_rules(&rules_storage_for_resolver);
                let resolver: SharedDynamicRulesResolver = Arc::new(DynamicRulesResolver::new(
                    cli_rules,
                    stored_rules,
                    values.clone(),
                ));

                log_resolver_rules(&resolver);

                let server = ProxyServer::new(config)
                    .with_tls_config(tls_config)
                    .with_admin_state(admin_state)
                    .with_rules(resolver.clone());

                let _metrics_task = start_metrics_collector_task(metrics_collector, 1);

                let rules_watcher_task = spawn_rules_watcher_task(
                    config_manager_for_resolver,
                    rules_storage_for_resolver,
                    values_storage_for_resolver,
                    resolver.clone(),
                    connection_registry_for_resolver,
                    runtime_config_for_resolver,
                );

                if let Err(e) = server.run().await {
                    eprintln!("Server error: {}", e);
                }

                rules_watcher_task.abort();
            });

            if let Err(e) = system_proxy_manager.blocking_write().restore() {
                eprintln!("Failed to restore system proxy: {}", e);
            }

            remove_pid()?;
            std::process::exit(0);
        }
        Err(e) => Err(bifrost_core::BifrostError::Config(format!(
            "Failed to fork: {}",
            e
        ))),
    }
}

fn load_stored_rules(rules_storage: &bifrost_storage::RulesStorage) -> Vec<Rule> {
    let mut stored_rules = Vec::new();
    match rules_storage.load_enabled() {
        Ok(rule_files) => {
            let stored_count = rule_files.len();
            for rule_file in rule_files {
                match bifrost_core::parse_rules(&rule_file.content) {
                    Ok(parsed) => {
                        tracing::info!(
                            target: "bifrost_cli::rules",
                            file = %rule_file.name,
                            enabled = rule_file.enabled,
                            parsed_count = parsed.len(),
                            "loaded rule file"
                        );
                        for mut rule in parsed {
                            rule.file = Some(rule_file.name.clone());
                            stored_rules.push(rule);
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "bifrost_cli::rules",
                            file = %rule_file.name,
                            error = %e,
                            "failed to parse rule file"
                        );
                    }
                }
            }
            if stored_count > 0 {
                tracing::info!(
                    target: "bifrost_cli::rules",
                    stored_files = stored_count,
                    total_rules = stored_rules.len(),
                    "loaded rules from storage"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                target: "bifrost_cli::rules",
                error = %e,
                "failed to load rules from storage"
            );
        }
    }
    stored_rules
}

fn log_resolver_rules(resolver: &DynamicRulesResolver) {
    let cli_count = resolver.cli_rules().len();
    if cli_count > 0 {
        tracing::info!(
            target: "bifrost_cli::rules",
            cli_rules = cli_count,
            "CLI rules loaded as default configuration (always active)"
        );
        for (idx, rule) in resolver.cli_rules().iter().enumerate() {
            tracing::debug!(
                target: "bifrost_cli::rules",
                index = idx + 1,
                pattern = %rule.pattern,
                protocol = %rule.protocol.to_str(),
                value = %rule.value,
                "CLI rule"
            );
        }
    }
}

fn spawn_rules_watcher_task(
    config_manager: Option<Arc<ConfigManager>>,
    rules_storage: bifrost_storage::RulesStorage,
    values_storage: bifrost_admin::SharedValuesStorage,
    resolver: SharedDynamicRulesResolver,
    connection_registry: bifrost_admin::SharedConnectionRegistry,
    runtime_config: bifrost_admin::SharedRuntimeConfig,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let Some(config_manager) = config_manager else {
            tracing::warn!(
                target: "bifrost_cli::rules",
                "ConfigManager not available, rules hot-reload disabled"
            );
            return;
        };

        let mut receiver = config_manager.subscribe();
        tracing::info!(
            target: "bifrost_cli::rules",
            "rules hot-reload watcher started"
        );

        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if matches!(
                        event,
                        ConfigChangeEvent::RulesChanged | ConfigChangeEvent::ValuesChanged(_)
                    ) {
                        tracing::info!(
                            target: "bifrost_cli::rules",
                            event = ?event,
                            "config change event received, reloading rules"
                        );

                        let new_stored_rules = load_stored_rules(&rules_storage);
                        let new_values = {
                            use bifrost_core::ValueStore;
                            values_storage.read().as_hashmap()
                        };

                        resolver.update_stored_rules(new_stored_rules, new_values);

                        if matches!(event, ConfigChangeEvent::RulesChanged) {
                            let should_disconnect = {
                                let config = runtime_config.read().await;
                                config.disconnect_on_config_change
                            };
                            if should_disconnect {
                                let disconnected =
                                    connection_registry.disconnect_all_with_mode(false);
                                if !disconnected.is_empty() {
                                    tracing::info!(
                                        target: "bifrost_cli::rules",
                                        count = disconnected.len(),
                                        "disconnected non-intercept connections due to rules change"
                                    );
                                }
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                    tracing::warn!(
                        target: "bifrost_cli::rules",
                        count = count,
                        "rules watcher lagged, some events may have been missed"
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!(
                        target: "bifrost_cli::rules",
                        "config change channel closed, stopping rules watcher"
                    );
                    break;
                }
            }
        }
    })
}
