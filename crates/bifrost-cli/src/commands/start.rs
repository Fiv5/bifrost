use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use bifrost_admin::{start_metrics_collector_task, AdminState, BodyStore};
use bifrost_core::{Rule, RulesResolver as CoreRulesResolver};
use bifrost_proxy::{AccessMode, ProxyConfig, ProxyServer};
use bifrost_storage::{RulesStorage, ValuesStorage};
use bifrost_tls::{get_platform_name, CertInstaller, CertStatus};
use parking_lot::RwLock as ParkingRwLock;
use tracing::info;

use crate::cli::Cli;
use crate::commands::ca::{check_and_install_certificate, load_tls_config};
use crate::config::{get_bifrost_dir, init_config_dir, load_config};
use crate::help::print_startup_help;
use crate::parsing::{parse_cli_rules, RulesResolverAdapter};
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

    init_config_dir()?;

    let parsed_access_mode = match &access_mode {
        Some(mode) => mode
            .parse::<AccessMode>()
            .map_err(bifrost_core::BifrostError::Config)?,
        None => {
            let config = load_config();
            if config.access.mode.is_empty() {
                AccessMode::LocalOnly
            } else {
                config.access.mode.parse().unwrap_or(AccessMode::LocalOnly)
            }
        }
    };

    let client_whitelist: Vec<String> = match whitelist {
        Some(wl) => wl.split(',').map(|s| s.trim().to_string()).collect(),
        None => {
            let config = load_config();
            config.access.whitelist
        }
    };

    let allow_lan_final = if allow_lan {
        true
    } else {
        let config = load_config();
        config.access.allow_lan
    };

    let enable_tls_interception = !no_intercept;

    let exclude_list: Vec<String> = match intercept_exclude {
        Some(list) => list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        None => {
            let config = load_config();
            config.intercept_exclude.clone()
        }
    };

    let include_list: Vec<String> = match intercept_include {
        Some(list) => list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        None => {
            let config = load_config();
            config.intercept_include.clone()
        }
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
        unsafe_ssl,
        verbose_logging,
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
    if unsafe_ssl {
        println!("⚠️  WARNING: Upstream TLS certificate verification is DISABLED (--unsafe-ssl)");
    }

    let values_dir = get_bifrost_dir()
        .map(|p| p.join("values"))
        .unwrap_or_else(|_| std::env::temp_dir().join("bifrost_values"));
    let early_values_storage = ValuesStorage::with_dir(values_dir.clone()).ok();
    let early_values = early_values_storage
        .as_ref()
        .map(|s| {
            use bifrost_core::ValueStore;
            s.as_hashmap()
        })
        .unwrap_or_default();
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
        let config = load_config();
        config.system_proxy.enabled
    };

    let system_proxy_bypass = proxy_bypass.unwrap_or_else(|| {
        let config = load_config();
        config.system_proxy.bypass.clone()
    });

    if enable_system_proxy {
        if bifrost_core::SystemProxyManager::is_supported() {
            println!("System proxy: enabled (bypass: {})", system_proxy_bypass);
        } else {
            println!("⚠️  WARNING: System proxy is not supported on this platform");
        }
    }

    if daemon {
        #[cfg(unix)]
        {
            run_daemon(
                proxy_config,
                parsed_rules,
                all_values.clone(),
                enable_system_proxy,
                system_proxy_bypass.clone(),
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
            no_disconnect_on_config_change,
        )?;
    }

    Ok(())
}

pub fn run_foreground(
    config: ProxyConfig,
    cli_rules: Vec<Rule>,
    cli_values: HashMap<String, String>,
    enable_system_proxy: bool,
    system_proxy_bypass: String,
    no_disconnect_on_config_change: bool,
) -> bifrost_core::Result<()> {
    let pid = std::process::id();
    write_pid(pid)?;

    print_startup_help(config.port);

    println!("════════════════════════════════════════════════════════════════════════");
    println!("                           SERVER STATUS");
    println!("════════════════════════════════════════════════════════════════════════");

    let tls_config = load_tls_config(&config)?;

    let bifrost_dir = get_bifrost_dir()?;
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
    println!("🔒 TLS/HTTPS INTERCEPTION");
    if config.enable_tls_interception {
        println!("   Status:        enabled");
        if !config.intercept_exclude.is_empty() {
            println!("   Excluded:      {:?}", config.intercept_exclude);
        }
        if config.unsafe_ssl {
            println!("   ⚠️  Upstream TLS verification: DISABLED (--unsafe-ssl)");
        }
    } else {
        println!("   Status:        disabled (--no-intercept)");
    }

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
        let body_temp_dir = get_bifrost_dir()
            .map(|p| p.join("body_cache"))
            .unwrap_or_else(|_| std::env::temp_dir().join("bifrost_body_cache"));
        let body_store = Arc::new(ParkingRwLock::new(BodyStore::new(
            body_temp_dir,
            64 * 1024,
            7,
        )));

        let values_dir = get_bifrost_dir()
            .map(|p| p.join("values"))
            .unwrap_or_else(|_| std::env::temp_dir().join("bifrost_values"));
        let values_storage = ValuesStorage::with_dir(values_dir.clone()).ok();
        let rules_dir = get_bifrost_dir()
            .map(|p| p.join("rules"))
            .unwrap_or_else(|_| std::env::temp_dir().join("bifrost_rules"));
        let rules_storage = RulesStorage::with_dir(rules_dir).ok();
        let mut values = values_storage
            .as_ref()
            .map(|s| {
                use bifrost_core::ValueStore;
                s.as_hashmap()
            })
            .unwrap_or_default();
        for (k, v) in cli_values {
            values.entry(k).or_insert(v);
        }

        let ca_cert_path = get_bifrost_dir()
            .map(|p| p.join("certs").join("ca.crt"))
            .ok();

        let runtime_config = bifrost_admin::RuntimeConfig {
            enable_tls_interception: config.enable_tls_interception,
            intercept_exclude: config.intercept_exclude.clone(),
            intercept_include: config.intercept_include.clone(),
            unsafe_ssl: config.unsafe_ssl,
            disconnect_on_config_change: !no_disconnect_on_config_change,
        };
        let connection_registry =
            bifrost_admin::ConnectionRegistry::new(!no_disconnect_on_config_change);

        let mut admin_state = AdminState::new(config.port)
            .with_body_store(body_store)
            .with_runtime_config(runtime_config)
            .with_connection_registry(connection_registry);
        if let Some(vs) = values_storage {
            admin_state = admin_state.with_values_storage(vs);
        }
        if let Some(rs) = rules_storage {
            admin_state = admin_state.with_rules_storage(rs);
        }
        if let Some(cert_path) = ca_cert_path {
            admin_state = admin_state.with_ca_cert_path(cert_path);
        }
        admin_state = admin_state.with_system_proxy_manager_shared(system_proxy_manager.clone());

        let metrics_collector = admin_state.metrics_collector.clone();
        let mut server = ProxyServer::new(config)
            .with_tls_config(tls_config)
            .with_admin_state(admin_state);

        if !cli_rules.is_empty() {
            let resolver = Arc::new(RulesResolverAdapter {
                inner: CoreRulesResolver::new(cli_rules).with_values(values),
            });
            server = server.with_rules(resolver);
        }

        let _metrics_task = start_metrics_collector_task(metrics_collector, 1);

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
) -> bifrost_core::Result<()> {
    use nix::unistd::{chdir, dup2, fork, setsid, ForkResult};
    use std::os::unix::io::AsRawFd;

    use crate::process::get_pid_file;

    let bifrost_dir = get_bifrost_dir()?;
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
                let body_temp_dir = bifrost_dir.join("body_cache");
                let body_store = Arc::new(ParkingRwLock::new(BodyStore::new(
                    body_temp_dir,
                    64 * 1024,
                    7,
                )));

                let values_dir = bifrost_dir.join("values");
                let values_storage = ValuesStorage::with_dir(values_dir).ok();
                let rules_dir = bifrost_dir.join("rules");
                let rules_storage = RulesStorage::with_dir(rules_dir).ok();
                let mut values = values_storage
                    .as_ref()
                    .map(|s| {
                        use bifrost_core::ValueStore;
                        s.as_hashmap()
                    })
                    .unwrap_or_default();
                for (k, v) in cli_values {
                    values.entry(k).or_insert(v);
                }

                let ca_cert_path = bifrost_dir.join("certs").join("ca.crt");

                let mut admin_state = AdminState::new(config.port).with_body_store(body_store);
                if let Some(vs) = values_storage {
                    admin_state = admin_state.with_values_storage(vs);
                }
                if let Some(rs) = rules_storage {
                    admin_state = admin_state.with_rules_storage(rs);
                }
                admin_state = admin_state.with_ca_cert_path(ca_cert_path);
                admin_state =
                    admin_state.with_system_proxy_manager_shared(system_proxy_manager.clone());

                let metrics_collector = admin_state.metrics_collector.clone();
                let mut server = ProxyServer::new(config)
                    .with_tls_config(tls_config)
                    .with_admin_state(admin_state);

                if !cli_rules.is_empty() {
                    let resolver = Arc::new(RulesResolverAdapter {
                        inner: CoreRulesResolver::new(cli_rules).with_values(values),
                    });
                    server = server.with_rules(resolver);
                }

                let _metrics_task = start_metrics_collector_task(metrics_collector, 1);
                if let Err(e) = server.run().await {
                    eprintln!("Server error: {}", e);
                }
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
