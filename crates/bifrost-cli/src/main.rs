use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use bifrost_admin::{start_metrics_collector_task, AdminState, BodyStore};
use bifrost_core::{
    init_logging, Protocol, RequestContext, Rule, RulesResolver as CoreRulesResolver,
};
use bifrost_proxy::{
    AccessMode, ProxyConfig, ProxyServer, ResolvedRules as ProxyResolvedRules, RuleValue,
    RulesResolver as ProxyRulesResolverTrait, TlsConfig,
};
use bifrost_storage::{BifrostConfig, RuleFile, RulesStorage, StateManager, ValuesStorage};
use bifrost_tls::{
    ensure_valid_ca, generate_root_ca, get_platform_name, init_crypto_provider, load_root_ca,
    parse_cert_info, save_root_ca, CertInstaller, CertStatus, DynamicCertGenerator, SniResolver,
};
use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Select};
use parking_lot::RwLock as ParkingRwLock;
use tracing::info;

#[derive(Parser)]
#[command(name = "bifrost")]
#[command(version = "1.0.0")]
#[command(about = "High-performance HTTP/HTTPS proxy written in Rust")]
#[command(
    long_about = "High-performance HTTP/HTTPS proxy written in Rust with TLS interception support.\n\n\
Running 'bifrost' without a subcommand is equivalent to 'bifrost start'."
)]
#[command(after_help = "\
EXAMPLES:
    bifrost                      Start proxy with defaults (port 9900, TLS enabled)
    bifrost -p 8080              Start proxy on port 8080
    bifrost start --daemon       Start proxy as background daemon
    bifrost start --no-intercept Start proxy without TLS interception
    bifrost status               Show proxy status
    bifrost stop                 Stop the running proxy

DEFAULT BEHAVIOR:
    When no subcommand is provided, bifrost starts in foreground mode with:
      • HTTP proxy on 0.0.0.0:9900
      • TLS/HTTPS interception enabled
      • Access restricted to localhost only
      • CA certificate auto-generated if missing

────────────────────────────────────────────────────────────────────────────
SUBCOMMAND REFERENCE
────────────────────────────────────────────────────────────────────────────

start [OPTIONS]                   Start the proxy server (default)
  -d, --daemon                      Run as background daemon
  --skip-cert-check                 Skip CA certificate check
  --access-mode <MODE>              Access mode: local_only|whitelist|interactive|allow_all
  --whitelist <IPS>                 Client IP whitelist (comma-separated, supports CIDR)
  --allow-lan                       Allow LAN (private network) clients
  --no-intercept                    Disable TLS/HTTPS interception
  --intercept-exclude <DOMAINS>     Exclude domains from interception (wildcards supported)
  --unsafe-ssl                      Skip upstream TLS verification (dangerous)
  --rules <RULE>                    Proxy rule (can be repeated)
  --rules-file <PATH>               Path to rules file
  --system-proxy                    Enable system proxy
  --proxy-bypass <LIST>             System proxy bypass list

stop                              Stop the running proxy

status                            Show proxy status

rule <ACTION>                     Manage rules
  list                              List all rules
  add <name> [-c content|-f file]   Add a new rule
  enable <name>                     Enable a rule
  disable <name>                    Disable a rule
  show <name>                       Show rule content
  delete <name>                     Delete a rule

ca <ACTION>                       Manage CA certificates
  info                              Show CA certificate info
  export [-o path]                  Export CA certificate
  generate [-f]                     Generate CA certificate

system-proxy <ACTION>             Manage system proxy
  status                            Show system proxy status
  enable [--host h] [--port p] [--bypass list]
                                    Enable system proxy
  disable                           Disable system proxy

whitelist <ACTION>                Manage access control
  list                              List whitelist entries
  add <ip>                          Add IP/CIDR to whitelist
  remove <ip>                       Remove IP/CIDR from whitelist
  allow-lan <true|false>            Enable/disable LAN access
  status                            Show access control settings

value <ACTION>                    Manage values for variable expansion
  list                              List all values
  get <name>                        Get a value
  set <name> <value>                Set a value
  delete <name>                     Delete a value
  import <file>                     Import from file (.txt/.kv/.json)

────────────────────────────────────────────────────────────────────────────
ENVIRONMENT VARIABLES
────────────────────────────────────────────────────────────────────────────

BIFROST_DATA_DIR                  Custom data directory path
                                  Default: ~/.bifrost (platform-specific)
                                  Contains: config, rules, values, certs, logs
                                  Example: BIFROST_DATA_DIR=/tmp/bifrost-test bifrost

RUST_LOG                          Control logging output level and filters
                                  Default: info (set via -l/--log-level)
                                  Example: RUST_LOG=debug bifrost
                                  Advanced: RUST_LOG=bifrost_proxy=debug,info

────────────────────────────────────────────────────────────────────────────
RULE TEMPLATE VARIABLES
────────────────────────────────────────────────────────────────────────────

Rules support variable expansion using ${...} syntax:

  ${name}                         Expand to value stored via 'bifrost value set'
  ${env.VAR_NAME}                 Expand to environment variable VAR_NAME

Example rule with variables:
  example.com host://${LOCAL_SERVER}
  api.example.com reqHeaders://(Authorization: ${env.API_TOKEN})

Manage values:
  bifrost value set LOCAL_SERVER 127.0.0.1:3000
  bifrost value list
")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, default_value = "9900", help = "HTTP proxy port")]
    port: u16,

    #[arg(short = 'H', long, default_value = "0.0.0.0", help = "Listen address")]
    host: String,

    #[arg(long, help = "SOCKS5 proxy port (disabled by default)")]
    socks5_port: Option<u16>,

    #[arg(
        short,
        long,
        default_value = "info",
        help = "Log level [trace|debug|info|warn|error]"
    )]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start the proxy server (default when no subcommand provided)")]
    Start {
        #[arg(short, long, help = "Run as daemon")]
        daemon: bool,
        #[arg(long, help = "Skip CA certificate installation check")]
        skip_cert_check: bool,
        #[arg(
            long,
            help = "Access control mode: local_only (default), whitelist, interactive, allow_all"
        )]
        access_mode: Option<String>,
        #[arg(
            long,
            help = "Client IP whitelist (comma-separated, supports CIDR notation)"
        )]
        whitelist: Option<String>,
        #[arg(long, help = "Allow LAN (private network) clients")]
        allow_lan: bool,
        #[arg(long, help = "Disable TLS/HTTPS interception (default: enabled)")]
        no_intercept: bool,
        #[arg(
            long,
            help = "Domains to exclude from TLS interception (comma-separated, supports wildcards like *.example.com)"
        )]
        intercept_exclude: Option<String>,
        #[arg(
            long,
            help = "Skip upstream server TLS certificate verification (dangerous, for testing only)"
        )]
        unsafe_ssl: bool,
        #[arg(
            long,
            help = "Proxy rules (e.g., 'example.com host://127.0.0.1:3000'). Can be specified multiple times."
        )]
        rules: Vec<String>,
        #[arg(long, help = "Path to rules file (one rule per line)")]
        rules_file: Option<PathBuf>,
        #[arg(long, help = "Enable system proxy configuration")]
        system_proxy: bool,
        #[arg(
            long,
            help = "System proxy bypass list (comma-separated, e.g., 'localhost,127.0.0.1,*.local')"
        )]
        proxy_bypass: Option<String>,
    },
    #[command(about = "Stop the proxy server")]
    Stop,
    #[command(about = "Show proxy server status")]
    Status,
    #[command(about = "Manage rules")]
    Rule {
        #[command(subcommand)]
        action: RuleCommands,
    },
    #[command(about = "Manage CA certificates")]
    Ca {
        #[command(subcommand)]
        action: CaCommands,
    },
    #[command(about = "Manage client IP whitelist")]
    Whitelist {
        #[command(subcommand)]
        action: WhitelistCommands,
    },
    #[command(about = "Toggle system proxy (enable/disable/status)")]
    SystemProxy {
        #[command(subcommand)]
        action: SystemProxyCommands,
    },
    #[command(about = "Manage values for rule variable expansion")]
    Value {
        #[command(subcommand)]
        action: ValueCommands,
    },
}

#[derive(Subcommand, Clone)]
enum RuleCommands {
    #[command(about = "List all rules")]
    List,
    #[command(about = "Add a new rule")]
    Add {
        #[arg(help = "Rule name")]
        name: String,
        #[arg(short, long, help = "Rule content")]
        content: Option<String>,
        #[arg(short, long, help = "Rule file path")]
        file: Option<PathBuf>,
    },
    #[command(about = "Delete a rule")]
    Delete {
        #[arg(help = "Rule name")]
        name: String,
    },
    #[command(about = "Enable a rule")]
    Enable {
        #[arg(help = "Rule name")]
        name: String,
    },
    #[command(about = "Disable a rule")]
    Disable {
        #[arg(help = "Rule name")]
        name: String,
    },
    #[command(about = "Show rule content")]
    Show {
        #[arg(help = "Rule name")]
        name: String,
    },
}

#[derive(Subcommand, Clone)]
enum CaCommands {
    #[command(about = "Generate CA certificate")]
    Generate {
        #[arg(short, long, help = "Force regenerate")]
        force: bool,
    },
    #[command(about = "Export CA certificate")]
    Export {
        #[arg(short, long, help = "Output path")]
        output: Option<PathBuf>,
    },
    #[command(about = "Show CA certificate info")]
    Info,
}

#[derive(Subcommand, Clone)]
enum WhitelistCommands {
    #[command(about = "List current whitelist entries")]
    List,
    #[command(about = "Add IP or CIDR to whitelist")]
    Add {
        #[arg(help = "IP address or CIDR (e.g., 192.168.1.100 or 192.168.1.0/24)")]
        ip_or_cidr: String,
    },
    #[command(about = "Remove IP or CIDR from whitelist")]
    Remove {
        #[arg(help = "IP address or CIDR to remove")]
        ip_or_cidr: String,
    },
    #[command(about = "Enable or disable LAN (private network) access")]
    AllowLan {
        #[arg(help = "Enable (true) or disable (false) LAN access")]
        enable: String,
    },
    #[command(about = "Show current access control settings")]
    Status,
}

#[derive(Subcommand, Clone)]
enum SystemProxyCommands {
    #[command(about = "Show system proxy status")]
    Status,
    #[command(about = "Enable system proxy")]
    Enable {
        #[arg(long, help = "Bypass list (comma-separated)")]
        bypass: Option<String>,
        #[arg(long, help = "Proxy host (default: 127.0.0.1)")]
        host: Option<String>,
        #[arg(long, help = "Proxy port (default: global -p)")]
        port: Option<u16>,
    },
    #[command(about = "Disable system proxy")]
    Disable,
}

#[derive(Subcommand, Clone)]
enum ValueCommands {
    #[command(about = "List all values")]
    List,
    #[command(about = "Get a value by name")]
    Get {
        #[arg(help = "Value name")]
        name: String,
    },
    #[command(about = "Set a value")]
    Set {
        #[arg(help = "Value name")]
        name: String,
        #[arg(help = "Value content")]
        value: String,
    },
    #[command(about = "Delete a value")]
    Delete {
        #[arg(help = "Value name")]
        name: String,
    },
    #[command(about = "Import values from file")]
    Import {
        #[arg(help = "File path (supports .txt, .kv, .json)")]
        file: PathBuf,
    },
}

fn print_startup_help(port: u16) {
    println!(
        r#"
╭────────────────────────────────────────────────────────────────────────╮
│                       BIFROST PROXY COMMANDS                           │
╰────────────────────────────────────────────────────────────────────────╯

PROXY CONTROL
  bifrost status                    Show proxy status
  bifrost stop                      Stop the running proxy

RULE MANAGEMENT
  bifrost rule list                 List all rules
  bifrost rule add <name>           Add a new rule
    -c, --content <CONTENT>           Rule content (inline)
    -f, --file <FILE>                 Rule file path
  bifrost rule enable <name>        Enable a rule
  bifrost rule disable <name>       Disable a rule
  bifrost rule show <name>          Show rule content
  bifrost rule delete <name>        Delete a rule

CA CERTIFICATE
  bifrost ca info                   Show CA certificate info
  bifrost ca export                 Export CA certificate
    -o, --output <PATH>               Output file path
  bifrost ca generate               (Re)generate CA certificate
    -f, --force                       Force regenerate

SYSTEM PROXY
  bifrost system-proxy status       Show system proxy status
  bifrost system-proxy enable       Enable system proxy
    --host <HOST>                     Proxy host (default: 127.0.0.1)
    --port <PORT>                     Proxy port
    --bypass <LIST>                   Bypass list (comma-separated)
  bifrost system-proxy disable      Disable system proxy

VALUES (Variable Expansion)
  bifrost value list                List all values
  bifrost value get <name>          Get a value
  bifrost value set <name> <value>  Set a value
  bifrost value delete <name>       Delete a value
  bifrost value import <file>       Import from file (.txt/.kv/.json)

ACCESS CONTROL
  bifrost whitelist list            List whitelist entries
  bifrost whitelist add <ip>        Add IP/CIDR to whitelist
  bifrost whitelist remove <ip>     Remove IP/CIDR from whitelist
  bifrost whitelist allow-lan <bool> Enable/disable LAN access
  bifrost whitelist status          Show access control settings

ADMIN UI
  http://127.0.0.1:{port}/          Web-based admin interface

Use 'bifrost <command> --help' for more details."#,
        port = port
    );
    println!();
}

fn get_bifrost_dir() -> bifrost_core::Result<PathBuf> {
    Ok(bifrost_storage::data_dir())
}

fn get_pid_file() -> bifrost_core::Result<PathBuf> {
    Ok(get_bifrost_dir()?.join("bifrost.pid"))
}

fn read_pid() -> Option<u32> {
    let pid_file = get_pid_file().ok()?;
    std::fs::read_to_string(&pid_file)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

fn write_pid(pid: u32) -> bifrost_core::Result<()> {
    let pid_file = get_pid_file()?;
    if let Some(parent) = pid_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&pid_file, pid.to_string())?;
    Ok(())
}

fn remove_pid() -> bifrost_core::Result<()> {
    let pid_file = get_pid_file()?;
    if pid_file.exists() {
        std::fs::remove_file(&pid_file)?;
    }
    Ok(())
}

#[cfg(unix)]
fn is_process_running(pid: u32) -> bool {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), Signal::SIGCONT).is_ok()
}

#[cfg(not(unix))]
fn is_process_running(_pid: u32) -> bool {
    false
}

fn main() {
    init_crypto_provider();

    let cli = Cli::parse();

    if let Err(e) = init_logging(&cli.log_level) {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    let result = match cli.command {
        Some(Commands::Start {
            daemon,
            skip_cert_check,
            ref access_mode,
            ref whitelist,
            allow_lan,
            no_intercept,
            ref intercept_exclude,
            unsafe_ssl,
            ref rules,
            ref rules_file,
            system_proxy,
            ref proxy_bypass,
        }) => run_start(
            &cli,
            daemon,
            skip_cert_check,
            access_mode.clone(),
            whitelist.clone(),
            allow_lan,
            no_intercept,
            intercept_exclude.clone(),
            unsafe_ssl,
            rules.clone(),
            rules_file.clone(),
            system_proxy,
            proxy_bypass.clone(),
        ),
        Some(Commands::Stop) => run_stop(),
        Some(Commands::Status) => run_status(),
        Some(Commands::Rule { action }) => handle_rule_command(action),
        Some(Commands::Ca { action }) => handle_ca_command(action),
        Some(Commands::Whitelist { action }) => handle_whitelist_command(action),
        Some(Commands::SystemProxy { ref action }) => {
            handle_system_proxy_command(&cli, action.clone())
        }
        Some(Commands::Value { action }) => handle_value_command(action),
        None => run_start(
            &cli,
            false,
            false,
            None,
            None,
            false,
            false,
            None,
            false,
            vec![],
            None,
            false,
            None,
        ),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_start(
    cli: &Cli,
    daemon: bool,
    skip_cert_check: bool,
    access_mode: Option<String>,
    whitelist: Option<String>,
    allow_lan: bool,
    no_intercept: bool,
    intercept_exclude: Option<String>,
    unsafe_ssl: bool,
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

    let proxy_config = ProxyConfig {
        port: cli.port,
        host: cli.host.clone(),
        socks5_port: cli.socks5_port,
        access_mode: parsed_access_mode,
        client_whitelist,
        allow_lan: allow_lan_final,
        enable_tls_interception,
        intercept_exclude: exclude_list.clone(),
        unsafe_ssl,
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
    if unsafe_ssl {
        println!("⚠️  WARNING: Upstream TLS certificate verification is DISABLED (--unsafe-ssl)");
    }

    let values_dir = get_bifrost_dir()
        .map(|p| p.join(".bifrost").join("values"))
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

    let parsed_rules = parse_cli_rules(&rules, &rules_file, &early_values)?;
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
                enable_system_proxy,
                system_proxy_bypass,
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
            enable_system_proxy,
            system_proxy_bypass,
        )?;
    }

    Ok(())
}

fn handle_system_proxy_command(cli: &Cli, action: SystemProxyCommands) -> bifrost_core::Result<()> {
    let bifrost_dir = get_bifrost_dir()?;
    let mut manager = bifrost_core::SystemProxyManager::new(bifrost_dir.clone());
    match action {
        SystemProxyCommands::Status => {
            if !bifrost_core::SystemProxyManager::is_supported() {
                println!("System proxy not supported on this platform");
                return Ok(());
            }
            match bifrost_core::SystemProxyManager::get_current() {
                Ok(status) => {
                    println!("Supported: true");
                    println!("Enabled:  {}", status.enable);
                    println!("Host:     {}", status.host);
                    println!("Port:     {}", status.port);
                    println!("Bypass:   {}", status.bypass);
                }
                Err(e) => {
                    eprintln!("Failed to get system proxy: {}", e);
                }
            }
        }
        SystemProxyCommands::Enable { bypass, host, port } => {
            if !bifrost_core::SystemProxyManager::is_supported() {
                println!("System proxy not supported on this platform");
                return Ok(());
            }
            let proxy_host = host.unwrap_or_else(|| "127.0.0.1".to_string());
            let proxy_port = port.unwrap_or(cli.port);
            let bypass_str = bypass.unwrap_or_else(|| {
                let cfg = load_config();
                cfg.system_proxy.bypass
            });
            if let Err(e) = manager.enable(&proxy_host, proxy_port, Some(&bypass_str)) {
                let msg = e.to_string();
                if msg.contains("RequiresAdmin") {
                    println!("System proxy requires administrator privileges.");
                    let proceed = dialoguer::Confirm::new()
                        .with_prompt("Try enabling via sudo now?")
                        .default(true)
                        .interact();
                    match proceed {
                        Ok(true) => {
                            #[cfg(target_os = "macos")]
                            {
                                if let Err(se) = manager.enable_with_privilege(
                                    &proxy_host,
                                    proxy_port,
                                    Some(&bypass_str),
                                ) {
                                    eprintln!("Failed to enable with sudo: {}", se);
                                } else {
                                    println!("✓ System proxy enabled via sudo");
                                }
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                eprintln!("Privilege escalation is only applicable on macOS.");
                            }
                        }
                        _ => {
                            println!("Cancelled.");
                        }
                    }
                } else {
                    eprintln!("Failed to enable system proxy: {}", e);
                }
            } else {
                println!(
                    "✓ System proxy enabled: {}:{} (bypass: {})",
                    proxy_host, proxy_port, bypass_str
                );
            }
        }
        SystemProxyCommands::Disable => {
            if !bifrost_core::SystemProxyManager::is_supported() {
                println!("System proxy not supported on this platform");
                return Ok(());
            }
            if let Err(e) = manager.disable() {
                let msg = e.to_string();
                if msg.contains("RequiresAdmin") {
                    println!("System proxy disable requires administrator privileges.");
                    let proceed = dialoguer::Confirm::new()
                        .with_prompt("Try disabling via sudo now?")
                        .default(true)
                        .interact();
                    match proceed {
                        Ok(true) => {
                            #[cfg(target_os = "macos")]
                            {
                                if let Err(se) = manager.disable_with_privilege() {
                                    eprintln!("Failed to disable with sudo: {}", se);
                                } else {
                                    println!("✓ System proxy disabled via sudo");
                                }
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                eprintln!("Privilege escalation is only applicable on macOS.");
                            }
                        }
                        _ => {
                            println!("Cancelled.");
                        }
                    }
                } else {
                    eprintln!("Failed to disable system proxy: {}", e);
                }
            } else {
                println!("✓ System proxy disabled");
            }
        }
    }
    Ok(())
}
fn init_config_dir() -> bifrost_core::Result<()> {
    let bifrost_dir = get_bifrost_dir()?;

    let config_path = bifrost_dir.join("config.toml");
    if !config_path.exists() {
        println!("Initializing configuration directory: {:?}", bifrost_dir);

        std::fs::create_dir_all(&bifrost_dir)?;

        let subdirs = ["rules", "values", "plugins", "certs"];
        for subdir in &subdirs {
            let path = bifrost_dir.join(subdir);
            std::fs::create_dir_all(&path)?;
        }

        let default_config = BifrostConfig::default();
        let config_content = toml::to_string_pretty(&default_config).map_err(|e| {
            bifrost_core::BifrostError::Config(format!("Failed to serialize config: {}", e))
        })?;
        std::fs::write(&config_path, &config_content)?;

        println!("  Created config file: {:?}", config_path);
        println!("  Created subdirectories: {:?}", subdirs);
        println!("Configuration initialized successfully.");
    }

    Ok(())
}

fn load_config() -> BifrostConfig {
    let config_path = get_bifrost_dir()
        .map(|p| p.join("config.toml"))
        .unwrap_or_default();
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str(&content) {
                return config;
            }
        }
    }
    BifrostConfig::default()
}

fn save_config(config: &BifrostConfig) -> bifrost_core::Result<()> {
    let config_dir = get_bifrost_dir()?;
    std::fs::create_dir_all(&config_dir)?;
    let config_path = config_dir.join("config.toml");
    let content = toml::to_string_pretty(config).map_err(|e| {
        bifrost_core::BifrostError::Config(format!("Failed to serialize config: {}", e))
    })?;
    std::fs::write(&config_path, content)?;
    Ok(())
}

fn parse_cli_rules(
    rules: &[String],
    rules_file: &Option<PathBuf>,
    values: &HashMap<String, String>,
) -> bifrost_core::Result<Vec<Rule>> {
    let mut all_rules = Vec::new();

    let parser = bifrost_core::RuleParser::with_values(values.clone());

    for rule_str in rules {
        match parser.parse_rules(rule_str) {
            Ok(parsed) => all_rules.extend(parsed),
            Err(e) => {
                return Err(bifrost_core::BifrostError::Config(format!(
                    "Failed to parse rule '{}': {}",
                    rule_str, e
                )));
            }
        }
    }

    if let Some(file_path) = rules_file {
        let content = std::fs::read_to_string(file_path).map_err(|e| {
            bifrost_core::BifrostError::Config(format!(
                "Failed to read rules file '{}': {}",
                file_path.display(),
                e
            ))
        })?;
        match parser.parse_rules(&content) {
            Ok(parsed) => all_rules.extend(parsed),
            Err(e) => {
                return Err(bifrost_core::BifrostError::Config(format!(
                    "Failed to parse rules file '{}': {}",
                    file_path.display(),
                    e
                )));
            }
        }
    }

    Ok(all_rules)
}

struct RulesResolverAdapter {
    inner: CoreRulesResolver,
}

impl ProxyRulesResolverTrait for RulesResolverAdapter {
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

        let core_result = self.inner.resolve(&ctx);
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
                Protocol::ReqHeaders => {
                    if let Some(headers) = parse_header_value(value) {
                        for (k, v) in headers {
                            let key_lower = k.to_lowercase();
                            if !result
                                .req_headers
                                .iter()
                                .any(|(existing, _)| existing.to_lowercase() == key_lower)
                            {
                                result.req_headers.push((k, v));
                            }
                        }
                    }
                }
                Protocol::ResHeaders => {
                    if let Some(headers) = parse_header_value(value) {
                        for (k, v) in headers {
                            let key_lower = k.to_lowercase();
                            if !result
                                .res_headers
                                .iter()
                                .any(|(existing, _)| existing.to_lowercase() == key_lower)
                            {
                                result.res_headers.push((k, v));
                            }
                        }
                    }
                }
                Protocol::StatusCode => {
                    if let Ok(code) = value.parse::<u16>() {
                        result.status_code = Some(code);
                    }
                }
                Protocol::ReplaceStatus => {
                    if let Ok(code) = value.parse::<u16>() {
                        result.replace_status = Some(code);
                    }
                }
                Protocol::ResBody => {
                    result.res_body = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ReqBody => {
                    result.req_body = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::Proxy => {
                    result.proxy = Some(value.to_string());
                }
                Protocol::Ignore => {
                    result.ignored = true;
                }
                Protocol::ResCors => {
                    result.enable_cors = true;
                }
                Protocol::File => {
                    result.mock_file = Some(value.to_string());
                }
                Protocol::Tpl => {
                    result.mock_template = Some(value.to_string());
                }
                Protocol::RawFile => {
                    result.mock_rawfile = Some(value.to_string());
                }
                Protocol::Ua => {
                    result.ua = Some(value.to_string());
                }
                Protocol::Referer => {
                    result.referer = Some(value.to_string());
                }
                Protocol::Method => {
                    result.method = Some(value.to_string());
                }
                Protocol::ReqDelay => {
                    if let Ok(delay) = value.parse::<u64>() {
                        result.req_delay = Some(delay);
                    }
                }
                Protocol::ResDelay => {
                    if let Ok(delay) = value.parse::<u64>() {
                        result.res_delay = Some(delay);
                    }
                }
                Protocol::ReqCookies => {
                    if let Some(cookies) = parse_header_value(value) {
                        for (k, v) in cookies {
                            result.req_cookies.push((k, v));
                        }
                    }
                }
                Protocol::ResCookies => {
                    if let Some(cookies) = parse_header_value(value) {
                        for (k, v) in cookies {
                            result.res_cookies.push((k, v));
                        }
                    }
                }
                Protocol::ReqPrepend => {
                    result.req_prepend = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ReqAppend => {
                    result.req_append = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ResPrepend => {
                    result.res_prepend = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ResAppend => {
                    result.res_append = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ReqReplace => {
                    if let Some((from, to)) = parse_replace_value(value) {
                        result.req_replace.push((from, to));
                    }
                }
                Protocol::ResReplace => {
                    if let Some((from, to)) = parse_replace_value(value) {
                        result.res_replace.push((from, to));
                    }
                }
                Protocol::Params => {
                    if let Ok(json_value) = serde_json::from_str(value) {
                        result.req_merge = Some(json_value);
                    }
                }
                Protocol::ResMerge => {
                    if let Ok(json_value) = serde_json::from_str(value) {
                        result.res_merge = Some(json_value);
                    }
                }
                Protocol::UrlParams => {
                    if let Some(params) = parse_header_value(value) {
                        for (k, v) in params {
                            result.url_params.push((k, v));
                        }
                    }
                }
                Protocol::UrlReplace => {
                    if let Some((from, to)) = parse_replace_value(value) {
                        result.url_replace.push((from, to));
                    }
                }
                Protocol::ForwardedFor => {
                    result.forwarded_for = Some(value.to_string());
                }
                Protocol::ReqType => {
                    result.req_type = Some(value.to_string());
                }
                Protocol::ReqCharset => {
                    result.req_charset = Some(value.to_string());
                }
                Protocol::ResType => {
                    result.res_type = Some(value.to_string());
                }
                Protocol::ResCharset => {
                    result.res_charset = Some(value.to_string());
                }
                Protocol::Cache => {
                    result.cache = Some(value.to_string());
                }
                Protocol::Attachment => {
                    result.attachment = Some(value.to_string());
                }
                Protocol::HtmlAppend => {
                    result.html_append = Some(value.to_string());
                }
                Protocol::HtmlPrepend => {
                    result.html_prepend = Some(value.to_string());
                }
                Protocol::HtmlBody => {
                    result.html_body = Some(value.to_string());
                }
                Protocol::JsAppend => {
                    result.js_append = Some(value.to_string());
                }
                Protocol::JsPrepend => {
                    result.js_prepend = Some(value.to_string());
                }
                Protocol::JsBody => {
                    result.js_body = Some(value.to_string());
                }
                Protocol::CssAppend => {
                    result.css_append = Some(value.to_string());
                }
                Protocol::CssPrepend => {
                    result.css_prepend = Some(value.to_string());
                }
                Protocol::CssBody => {
                    result.css_body = Some(value.to_string());
                }
                Protocol::ReqSpeed => {
                    if let Ok(speed) = value.parse::<u64>() {
                        result.req_speed = Some(speed);
                    }
                }
                Protocol::ResSpeed => {
                    if let Ok(speed) = value.parse::<u64>() {
                        result.res_speed = Some(speed);
                    }
                }
                _ => {}
            }
        }

        result
    }
}

fn parse_header_value(value: &str) -> Option<Vec<(String, String)>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (content, use_colon) = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        (&trimmed[1..trimmed.len() - 1], true)
    } else {
        (trimmed, trimmed.contains('\n') || trimmed.contains(':'))
    };

    let mut headers = Vec::new();

    let delimiter = if content.contains('\n') { '\n' } else { ',' };
    for part in content.split(delimiter) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let separator = if use_colon { ':' } else { '=' };
        if let Some(pos) = part.find(separator) {
            let key = part[..pos].trim().to_string();
            let val = part[pos + 1..].trim().to_string();
            if !key.is_empty() {
                headers.push((key, val));
            }
        }
    }

    if headers.is_empty() {
        None
    } else {
        Some(headers)
    }
}

fn parse_replace_value(value: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = value.splitn(2, ' ').collect();
    if parts.len() == 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else if parts.len() == 1 && !parts[0].is_empty() {
        Some((parts[0].to_string(), String::new()))
    } else {
        None
    }
}

fn run_foreground(
    config: ProxyConfig,
    cli_rules: Vec<Rule>,
    enable_system_proxy: bool,
    system_proxy_bypass: String,
) -> bifrost_core::Result<()> {
    let pid = std::process::id();
    write_pid(pid)?;

    print_startup_help(config.port);

    println!("════════════════════════════════════════════════════════════════════════");
    println!("                           SERVER STATUS");
    println!("════════════════════════════════════════════════════════════════════════");

    let tls_config = load_tls_config(&config)?;

    let bifrost_dir = get_bifrost_dir()?;
    let mut system_proxy_manager = bifrost_core::SystemProxyManager::new(bifrost_dir.clone());

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
        if let Err(e) =
            system_proxy_manager.enable(&proxy_host, config.port, Some(&system_proxy_bypass))
        {
            let msg = e.to_string();
            if msg.contains("RequiresAdmin") {
                println!("  ⚠ System proxy requires admin privileges (not enabled)");
            } else {
                eprintln!("  ✗ Failed to enable system proxy: {}", e);
            }
        } else {
            system_proxy_enabled = true;
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
            .map(|p| p.join(".bifrost").join("values"))
            .unwrap_or_else(|_| std::env::temp_dir().join("bifrost_values"));
        let values_storage = ValuesStorage::with_dir(values_dir.clone()).ok();
        let values = values_storage
            .as_ref()
            .map(|s| {
                use bifrost_core::ValueStore;
                s.as_hashmap()
            })
            .unwrap_or_default();

        let ca_cert_path = get_bifrost_dir()
            .map(|p| p.join("certs").join("ca.crt"))
            .ok();

        let mut admin_state = AdminState::new(config.port).with_body_store(body_store);
        if let Some(vs) = values_storage {
            admin_state = admin_state.with_values_storage(vs);
        }
        if let Some(cert_path) = ca_cert_path {
            admin_state = admin_state.with_ca_cert_path(cert_path);
        }

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

    if let Err(e) = system_proxy_manager.restore() {
        eprintln!("Failed to restore system proxy: {}", e);
    }

    remove_pid()?;
    println!("Bifrost proxy stopped.");
    Ok(())
}

#[cfg(unix)]
fn run_daemon(
    config: ProxyConfig,
    cli_rules: Vec<Rule>,
    enable_system_proxy: bool,
    system_proxy_bypass: String,
) -> bifrost_core::Result<()> {
    use nix::unistd::{chdir, dup2, fork, setsid, ForkResult};
    use std::os::unix::io::AsRawFd;

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

            let mut system_proxy_manager =
                bifrost_core::SystemProxyManager::new(bifrost_dir.clone());

            if let Err(e) = bifrost_core::SystemProxyManager::recover_from_crash(&bifrost_dir) {
                tracing::warn!("Failed to recover system proxy from previous crash: {}", e);
            }

            if enable_system_proxy {
                let proxy_host = if config.host == "0.0.0.0" {
                    "127.0.0.1".to_string()
                } else {
                    config.host.clone()
                };
                if let Err(e) = system_proxy_manager.enable(
                    &proxy_host,
                    config.port,
                    Some(&system_proxy_bypass),
                ) {
                    let msg = e.to_string();
                    if msg.contains("RequiresAdmin") {
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

                let values_dir = bifrost_dir.join(".bifrost").join("values");
                let values_storage = ValuesStorage::with_dir(values_dir).ok();
                let values = values_storage
                    .as_ref()
                    .map(|s| {
                        use bifrost_core::ValueStore;
                        s.as_hashmap()
                    })
                    .unwrap_or_default();

                let ca_cert_path = bifrost_dir.join("certs").join("ca.crt");

                let mut admin_state = AdminState::new(config.port).with_body_store(body_store);
                if let Some(vs) = values_storage {
                    admin_state = admin_state.with_values_storage(vs);
                }
                admin_state = admin_state.with_ca_cert_path(ca_cert_path);

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

            if let Err(e) = system_proxy_manager.restore() {
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

fn run_stop() -> bifrost_core::Result<()> {
    let pid = read_pid().ok_or_else(|| {
        bifrost_core::BifrostError::NotFound("No PID file found. Is the proxy running?".to_string())
    })?;

    if !is_process_running(pid) {
        remove_pid()?;
        println!("Bifrost proxy is not running (stale PID file removed).");
        return Ok(());
    }

    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        println!("Stopping Bifrost proxy (PID: {})...", pid);
        kill(Pid::from_raw(pid as i32), Signal::SIGTERM).map_err(|e| {
            bifrost_core::BifrostError::Config(format!("Failed to send SIGTERM: {}", e))
        })?;

        for i in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if !is_process_running(pid) {
                remove_pid()?;
                println!("Bifrost proxy stopped.");
                return Ok(());
            }
            if i == 30 {
                println!("Sending SIGKILL...");
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
            }
        }

        remove_pid()?;
        println!("Bifrost proxy stopped (forced).");
    }

    #[cfg(not(unix))]
    {
        println!("Stop command is not supported on this platform.");
        println!("Please terminate the process manually (PID: {}).", pid);
    }

    Ok(())
}

fn run_status() -> bifrost_core::Result<()> {
    println!("Bifrost Proxy Status");
    println!("====================");

    match read_pid() {
        Some(pid) => {
            if is_process_running(pid) {
                println!("Status: Running");
                println!("PID: {}", pid);
            } else {
                println!("Status: Stopped (stale PID file exists)");
                println!("Stale PID: {}", pid);
            }
        }
        None => {
            println!("Status: Stopped");
        }
    }

    println!();

    let state = StateManager::new()?;
    let enabled_groups = state.enabled_groups();

    println!("Rule Groups");
    println!("-----------");
    println!("Enabled rule groups: {}", enabled_groups.len());
    for group in enabled_groups {
        println!("  - {}", group);
    }

    let disabled_rules = state.disabled_rules();
    if !disabled_rules.is_empty() {
        println!("Disabled rules: {}", disabled_rules.len());
        for rule in disabled_rules {
            println!("  - {}", rule);
        }
    }

    Ok(())
}

fn handle_rule_command(action: RuleCommands) -> bifrost_core::Result<()> {
    let storage = RulesStorage::new()?;

    match action {
        RuleCommands::List => {
            let rules = storage.list()?;
            if rules.is_empty() {
                println!("No rules found.");
            } else {
                println!("Rules ({}):", rules.len());
                for name in rules {
                    let rule = storage.load(&name)?;
                    let status = if rule.enabled { "enabled" } else { "disabled" };
                    println!("  {} [{}]", name, status);
                }
            }
        }
        RuleCommands::Add {
            name,
            content,
            file,
        } => {
            let rule_content = if let Some(c) = content {
                c
            } else if let Some(path) = file {
                std::fs::read_to_string(&path)?
            } else {
                return Err(bifrost_core::BifrostError::Config(
                    "Either --content or --file must be provided".to_string(),
                ));
            };

            let rule = RuleFile::new(&name, rule_content);
            storage.save(&rule)?;
            println!("Rule '{}' added successfully.", name);
        }
        RuleCommands::Delete { name } => {
            storage.delete(&name)?;
            println!("Rule '{}' deleted successfully.", name);
        }
        RuleCommands::Enable { name } => {
            storage.set_enabled(&name, true)?;
            println!("Rule '{}' enabled.", name);
        }
        RuleCommands::Disable { name } => {
            storage.set_enabled(&name, false)?;
            println!("Rule '{}' disabled.", name);
        }
        RuleCommands::Show { name } => {
            let rule = storage.load(&name)?;
            println!("Rule: {}", rule.name);
            println!(
                "Status: {}",
                if rule.enabled { "enabled" } else { "disabled" }
            );
            println!("Content:");
            println!("{}", rule.content);
        }
    }

    Ok(())
}

fn handle_ca_command(action: CaCommands) -> bifrost_core::Result<()> {
    let cert_dir = get_bifrost_dir()?.join("certs");
    std::fs::create_dir_all(&cert_dir)?;

    let ca_key_path = cert_dir.join("ca.key");
    let ca_cert_path = cert_dir.join("ca.crt");

    match action {
        CaCommands::Generate { force } => {
            if ca_cert_path.exists() && !force {
                println!("CA certificate already exists.");
                println!("Use --force to regenerate.");
                return Ok(());
            }

            let ca = generate_root_ca()?;
            save_root_ca(&ca_cert_path, &ca_key_path, &ca)?;
            println!("CA certificate generated successfully.");
            println!("Certificate: {}", ca_cert_path.display());
            println!("Private key: {}", ca_key_path.display());
            println!();
            println!(
                "To use HTTPS interception, install the CA certificate in your browser or system."
            );
        }
        CaCommands::Export { output } => {
            if !ca_cert_path.exists() {
                return Err(bifrost_core::BifrostError::NotFound(
                    "CA certificate not found. Run 'bifrost ca generate' first.".to_string(),
                ));
            }

            let output_path = output.unwrap_or_else(|| PathBuf::from("bifrost-ca.crt"));
            std::fs::copy(&ca_cert_path, &output_path)?;
            println!("CA certificate exported to: {}", output_path.display());
        }
        CaCommands::Info => {
            if !ca_cert_path.exists() {
                return Err(bifrost_core::BifrostError::NotFound(
                    "CA certificate not found. Run 'bifrost ca generate' first.".to_string(),
                ));
            }

            let _ca = load_root_ca(&ca_cert_path, &ca_key_path)?;

            println!("CA Certificate Information");
            println!("==========================");
            println!();

            match parse_cert_info(&ca_cert_path) {
                Ok(cert_info) => {
                    println!("📜 Certificate Details");
                    println!("  Subject:           {}", cert_info.subject);
                    println!("  Issuer:            {}", cert_info.issuer);
                    println!("  Serial Number:     {}", cert_info.serial_number);
                    println!("  Signature Algo:    {}", cert_info.signature_algorithm);
                    println!(
                        "  Is CA:             {}",
                        if cert_info.is_ca { "Yes" } else { "No" }
                    );
                    println!();

                    println!("🔑 Key Information");
                    print!("  Algorithm:         {}", cert_info.key_type);
                    if let Some(size) = cert_info.key_size {
                        print!(" ({} bits)", size);
                    }
                    println!();
                    if !cert_info.key_usages.is_empty() {
                        println!("  Key Usage:         {}", cert_info.key_usages.join(", "));
                    }
                    if !cert_info.extended_key_usages.is_empty() {
                        println!(
                            "  Extended Usage:    {}",
                            cert_info.extended_key_usages.join(", ")
                        );
                    }
                    println!();

                    println!("📅 Validity Period");
                    println!(
                        "  Not Before:        {}",
                        cert_info.not_before.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    println!(
                        "  Not After:         {}",
                        cert_info.not_after.format("%Y-%m-%d %H:%M:%S UTC")
                    );

                    let days = cert_info.days_remaining();
                    if cert_info.is_expired() {
                        println!("  Status:            ❌ EXPIRED ({} days ago)", -days);
                    } else if cert_info.is_not_yet_valid() {
                        println!("  Status:            ⏳ Not yet valid");
                    } else {
                        let years = days / 365;
                        let remaining_days = days % 365;
                        if years > 0 {
                            println!(
                                "  Remaining:         {} days ({} years, {} days)",
                                days, years, remaining_days
                            );
                        } else {
                            println!("  Remaining:         {} days", days);
                        }
                        if days < 30 {
                            println!("  ⚠️  Certificate will expire soon!");
                        }
                    }
                    println!();

                    println!("🔐 Fingerprint");
                    println!("  SHA-256:           {}", cert_info.fingerprint_sha256);
                    println!();
                }
                Err(e) => {
                    println!("⚠️  Could not parse certificate details: {}", e);
                    println!();
                }
            }

            println!("📂 File Paths");
            println!("  Certificate:       {}", ca_cert_path.display());
            println!("  Private Key:       {}", ca_key_path.display());
            let cert_meta = std::fs::metadata(&ca_cert_path)?;
            if let Ok(modified) = cert_meta.modified() {
                if let Ok(duration) = modified.elapsed() {
                    let days = duration.as_secs() / 86400;
                    if days == 0 {
                        let hours = duration.as_secs() / 3600;
                        if hours == 0 {
                            let mins = duration.as_secs() / 60;
                            println!("  File Modified:     {} minutes ago", mins);
                        } else {
                            println!("  File Modified:     {} hours ago", hours);
                        }
                    } else {
                        println!("  File Modified:     {} days ago", days);
                    }
                }
            }
            println!();

            println!("💻 System Trust Status ({})", get_platform_name());
            let installer = CertInstaller::new(&ca_cert_path);
            match installer.get_detailed_status() {
                Ok(system_info) => {
                    let status_icon = match system_info.status {
                        CertStatus::InstalledAndTrusted => "✓",
                        CertStatus::InstalledNotTrusted => "⚠",
                        CertStatus::NotInstalled => "✗",
                    };
                    println!(
                        "  Status:            {} {}",
                        status_icon, system_info.status
                    );
                    if let Some(location) = system_info.keychain_location {
                        println!("  Location:          {}", location);
                    }
                    if let Some(path) = system_info.system_cert_path {
                        println!("  System Path:       {}", path.display());
                    }

                    if system_info.status != CertStatus::InstalledAndTrusted {
                        println!();
                        println!(
                            "  💡 Run 'bifrost ca install' to install and trust the certificate."
                        );
                    }
                }
                Err(e) => {
                    println!("  Could not check trust status: {}", e);
                }
            }
        }
    }

    Ok(())
}

fn load_tls_config(config: &ProxyConfig) -> bifrost_core::Result<Arc<TlsConfig>> {
    if !config.enable_tls_interception {
        return Ok(Arc::new(TlsConfig::default()));
    }

    let cert_dir = get_bifrost_dir()?.join("certs");
    let ca_key_path = cert_dir.join("ca.key");
    let ca_cert_path = cert_dir.join("ca.crt");

    let ca_valid = ensure_valid_ca(&ca_cert_path, &ca_key_path)?;
    if !ca_valid {
        println!("TLS interception enabled but valid CA certificate not found.");
        println!("Generating CA certificate...");
        std::fs::create_dir_all(&cert_dir)?;
        let ca = generate_root_ca()?;
        save_root_ca(&ca_cert_path, &ca_key_path, &ca)?;
        println!("✓ CA certificate generated: {}", ca_cert_path.display());
    }

    let ca = load_root_ca(&ca_cert_path, &ca_key_path)?;
    let ca_cert_bytes = std::fs::read(&ca_cert_path)?;
    let ca_key_bytes = std::fs::read(&ca_key_path)?;
    let ca_arc = Arc::new(ca);
    let sni_resolver = SniResolver::new(ca_arc.clone());
    let cert_generator = DynamicCertGenerator::new(ca_arc);

    println!("✓ TLS interception enabled");

    Ok(Arc::new(TlsConfig {
        ca_cert: Some(ca_cert_bytes),
        ca_key: Some(ca_key_bytes),
        cert_generator: Some(Arc::new(cert_generator)),
        sni_resolver: Some(Arc::new(sni_resolver)),
    }))
}

fn check_and_install_certificate() -> bifrost_core::Result<()> {
    let cert_dir = get_bifrost_dir()?.join("certs");
    let ca_key_path = cert_dir.join("ca.key");
    let ca_cert_path = cert_dir.join("ca.crt");

    let ca_valid = ensure_valid_ca(&ca_cert_path, &ca_key_path)?;
    if !ca_valid {
        println!("Valid CA certificate not found. Generating...");
        std::fs::create_dir_all(&cert_dir)?;
        let ca = generate_root_ca()?;
        save_root_ca(&ca_cert_path, &ca_key_path, &ca)?;
        println!("✓ CA certificate generated.");
        println!("  Certificate: {}", ca_cert_path.display());
        println!();
    }

    let installer = CertInstaller::new(&ca_cert_path);
    let status = installer.check_status()?;

    match status {
        CertStatus::InstalledAndTrusted => {
            println!("✓ CA certificate is installed and trusted.");
            Ok(())
        }
        CertStatus::InstalledNotTrusted => {
            println!("⚠ CA certificate is installed but not trusted.");
            println!();
            prompt_trust_certificate(&installer)
        }
        CertStatus::NotInstalled => {
            println!("⚠ CA certificate is not installed in system trust store.");
            println!();
            prompt_install_certificate(&installer)
        }
    }
}

fn prompt_install_certificate(installer: &CertInstaller) -> bifrost_core::Result<()> {
    println!("HTTPS interception requires the CA certificate to be trusted by the system.");
    println!("Without it, browsers will show security warnings for HTTPS sites.");
    println!();
    println!("Platform: {}", get_platform_name());
    println!();

    let options = vec![
        "Yes, install and trust (requires sudo/admin)",
        "No, skip (HTTPS interception may not work properly)",
        "Show manual installation instructions",
    ];

    let selection = Select::new()
        .with_prompt("Would you like to install and trust the CA certificate?")
        .items(&options)
        .default(0)
        .interact();

    match selection {
        Ok(0) => {
            installer.install_and_trust()?;
            Ok(())
        }
        Ok(1) => {
            println!("Skipping certificate installation.");
            println!("You can install it later using 'bifrost ca install' or manually.");
            Ok(())
        }
        Ok(2) => {
            println!();
            println!("{}", installer.get_install_instructions());
            println!();

            let proceed = Confirm::new()
                .with_prompt("Continue without installing?")
                .default(true)
                .interact();

            match proceed {
                Ok(true) => Ok(()),
                Ok(false) => prompt_install_certificate(installer),
                Err(_) => Ok(()),
            }
        }
        _ => Ok(()),
    }
}

fn prompt_trust_certificate(installer: &CertInstaller) -> bifrost_core::Result<()> {
    println!("The CA certificate is installed but not trusted by the system.");
    println!("HTTPS interception may not work properly without trust.");
    println!();

    let proceed = Confirm::new()
        .with_prompt("Would you like to trust the CA certificate now? (requires sudo/admin)")
        .default(true)
        .interact();

    match proceed {
        Ok(true) => {
            installer.install_and_trust()?;
            Ok(())
        }
        Ok(false) => {
            println!("Skipping. You can trust it later manually.");
            Ok(())
        }
        Err(_) => Ok(()),
    }
}

fn handle_whitelist_command(action: WhitelistCommands) -> bifrost_core::Result<()> {
    let mut config = load_config();

    match action {
        WhitelistCommands::List => {
            println!("Client IP Whitelist");
            println!("===================");
            if config.access.whitelist.is_empty() {
                println!("No entries in whitelist.");
            } else {
                for entry in &config.access.whitelist {
                    println!("  - {}", entry);
                }
            }
            println!();
            println!(
                "LAN (private network) access: {}",
                if config.access.allow_lan {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
        WhitelistCommands::Add { ip_or_cidr } => {
            if ip_or_cidr.contains('/') {
                if ip_or_cidr.parse::<ipnet::IpNet>().is_err() {
                    return Err(bifrost_core::BifrostError::Config(format!(
                        "Invalid CIDR notation: {}",
                        ip_or_cidr
                    )));
                }
            } else if ip_or_cidr.parse::<std::net::IpAddr>().is_err() {
                return Err(bifrost_core::BifrostError::Config(format!(
                    "Invalid IP address: {}",
                    ip_or_cidr
                )));
            }

            if config.access.whitelist.contains(&ip_or_cidr) {
                println!("'{}' is already in the whitelist.", ip_or_cidr);
            } else {
                config.access.whitelist.push(ip_or_cidr.clone());
                save_config(&config)?;
                println!("Added '{}' to whitelist.", ip_or_cidr);
                println!("Note: Restart the proxy server for changes to take effect.");
            }
        }
        WhitelistCommands::Remove { ip_or_cidr } => {
            if let Some(pos) = config
                .access
                .whitelist
                .iter()
                .position(|x| x == &ip_or_cidr)
            {
                config.access.whitelist.remove(pos);
                save_config(&config)?;
                println!("Removed '{}' from whitelist.", ip_or_cidr);
                println!("Note: Restart the proxy server for changes to take effect.");
            } else {
                println!("'{}' is not in the whitelist.", ip_or_cidr);
            }
        }
        WhitelistCommands::AllowLan { enable } => {
            let enable_bool = match enable.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => true,
                "false" | "0" | "no" | "off" => false,
                _ => {
                    return Err(bifrost_core::BifrostError::Config(format!(
                        "Invalid value '{}'. Use 'true' or 'false'.",
                        enable
                    )));
                }
            };
            config.access.allow_lan = enable_bool;
            save_config(&config)?;
            if enable_bool {
                println!("LAN (private network) access enabled.");
            } else {
                println!("LAN (private network) access disabled.");
            }
            println!("Note: Restart the proxy server for changes to take effect.");
        }
        WhitelistCommands::Status => {
            println!("Access Control Settings");
            println!("=======================");
            println!(
                "Mode: {}",
                if config.access.mode.is_empty() {
                    "local_only (default)"
                } else {
                    &config.access.mode
                }
            );
            println!(
                "LAN access: {}",
                if config.access.allow_lan {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!();
            println!("Whitelist entries: {}", config.access.whitelist.len());
            if !config.access.whitelist.is_empty() {
                for entry in &config.access.whitelist {
                    println!("  - {}", entry);
                }
            }
            println!();
            println!("Access mode options:");
            println!("  local_only  - Only allow connections from localhost (default)");
            println!("  whitelist   - Allow localhost + whitelisted IPs/CIDRs");
            println!("  interactive - Prompt for confirmation on unknown IPs");
            println!("  allow_all   - Allow all connections (not recommended)");
        }
    }

    Ok(())
}

fn handle_value_command(action: ValueCommands) -> bifrost_core::Result<()> {
    let values_dir = bifrost_storage::data_dir().join(".bifrost").join("values");
    let mut storage = ValuesStorage::with_dir(values_dir.clone())?;

    match action {
        ValueCommands::List => {
            let entries = storage.list_entries()?;
            if entries.is_empty() {
                println!("No values defined.");
                println!();
                println!("Values directory: {}", values_dir.display());
            } else {
                println!("Values ({}):", entries.len());
                println!("====================");
                for entry in entries {
                    let preview = if entry.value.len() > 50 {
                        format!("{}...", &entry.value[..47])
                    } else {
                        entry.value.clone()
                    };
                    let preview = preview.replace('\n', "\\n");
                    println!("  {} = {}", entry.name, preview);
                }
                println!();
                println!("Values directory: {}", values_dir.display());
            }
        }
        ValueCommands::Get { name } => {
            if let Some(value) = storage.get_value(&name) {
                println!("{}", value);
            } else {
                return Err(bifrost_core::BifrostError::NotFound(format!(
                    "Value '{}' not found",
                    name
                )));
            }
        }
        ValueCommands::Set { name, value } => {
            storage.set_value(&name, &value)?;
            println!("Value '{}' has been set.", name);
        }
        ValueCommands::Delete { name } => {
            if storage.exists(&name) {
                storage.remove_value(&name)?;
                println!("Value '{}' has been deleted.", name);
            } else {
                return Err(bifrost_core::BifrostError::NotFound(format!(
                    "Value '{}' not found",
                    name
                )));
            }
        }
        ValueCommands::Import { file } => {
            if !file.exists() {
                return Err(bifrost_core::BifrostError::NotFound(format!(
                    "File not found: {}",
                    file.display()
                )));
            }
            let count = storage.load_from_file(&file)?;
            println!("Imported {} value(s) from '{}'.", count, file.display());
        }
    }

    Ok(())
}
