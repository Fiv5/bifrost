use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bifrost")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(disable_version_flag = true)]
#[command(about = "High-performance HTTP/HTTPS/SOCKS5/HTTP3 proxy written in Rust")]
#[command(
    long_about = "High-performance HTTP/HTTPS/SOCKS5/HTTP3 proxy written in Rust with TLS interception support.\n\n\
Supported Protocols:\n\
  • HTTP/1.1, HTTP/2, HTTP/3 (QUIC)\n\
  • HTTPS (TLS 1.2/1.3, MITM interception)\n\
  • SOCKS5 TCP/UDP (with authentication)\n\
  • WebSocket (ws/wss)\n\
  • CONNECT-UDP (MASQUE, RFC 9298)\n\
  • gRPC, SSE\n\n\
Running 'bifrost' without a subcommand is equivalent to 'bifrost start'."
)]
#[command(after_help = "\
SUPPORTED PROTOCOLS:
  HTTP/1.1          Full support
  HTTP/2            Frame-level processing, multiplexing
  HTTP/3 (QUIC)     Based on Quinn, supports 0-RTT
  HTTPS             TLS 1.2/1.3, MITM interception
  SOCKS5 TCP        Username/password authentication
  SOCKS5 UDP        Full UDP ASSOCIATE support
  WebSocket         ws:// and wss:// protocols
  CONNECT-UDP       MASQUE protocol (RFC 9298)
  gRPC              Based on HTTP/2
  SSE               Server-Sent Events

EXAMPLES:
    bifrost                      Start proxy with defaults (port 9900, TLS enabled)
    bifrost -p 8080              Start proxy on port 8080
    bifrost start --daemon       Start proxy as background daemon
    bifrost start --no-intercept Start proxy without TLS interception
    bifrost start --intercept-exclude '*.apple.com,*.microsoft.com'
                                 Exclude domains from TLS interception
    bifrost start --intercept-include '*.api.local'
                                 Force intercept specific domains (works even with --no-intercept)
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
  -p, --port <PORT>                   Unified proxy port for HTTP/HTTPS/SOCKS5
  -H, --host <HOST>                   Listen address (overrides global -H)
  --socks5-port <PORT>                Separate SOCKS5 port (optional; default: share main port)
  -d, --daemon                        Run as background daemon
  --skip-cert-check                   Skip CA certificate check
  --access-mode <MODE>                Access mode: local_only|whitelist|interactive|allow_all
  --whitelist <IPS>                   Client IP whitelist (comma-separated, supports CIDR)
  --allow-lan                         Allow LAN (private network) clients
  --no-intercept                      Disable TLS/HTTPS interception
  --intercept-exclude <DOMAINS>       Exclude domains from interception (supports wildcards)
  --intercept-include <DOMAINS>       Force intercept domains (highest priority)
  --app-intercept-exclude <APPS>      Exclude apps from TLS interception (supports wildcards)
  --app-intercept-include <APPS>      Force intercept apps (highest priority)
  --unsafe-ssl                        Skip upstream TLS verification (dangerous)
  --no-disconnect-on-config-change    Disable auto-disconnect when TLS config changes
  --rules <RULE>                      Proxy rule (can be repeated)
  --rules-file <PATH>                 Path to rules file
  --system-proxy                      Enable system proxy
  --proxy-bypass <LIST>               System proxy bypass list

  TLS Interception Priority (highest to lowest):
    1. Rule-based (tlsIntercept://, tlsPassthrough://)
    2. --intercept-include / --app-intercept-include: Always intercept
    3. --intercept-exclude / --app-intercept-exclude: Never intercept
    4. --no-intercept flag: Global switch (default: enabled)

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
pub struct Cli {
    #[arg(short = 'v', short_alias = 'V', long, action = ArgAction::Version, help = "Print version")]
    pub version: (),

    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short, long, default_value = "9900", help = "HTTP proxy port")]
    pub port: u16,

    #[arg(short = 'H', long, default_value = "0.0.0.0", help = "Listen address")]
    pub host: String,

    #[arg(
        long,
        help = "Separate SOCKS5 proxy port (by default SOCKS5 shares the main port)"
    )]
    pub socks5_port: Option<u16>,

    #[arg(
        short,
        long,
        default_value = "info",
        help = "Log level [trace|debug|info|warn|error]"
    )]
    pub log_level: String,

    #[arg(
        long,
        default_value = "console,file",
        help = "Log output targets: console, file, or both (comma-separated)"
    )]
    pub log_output: String,

    #[arg(long, help = "Log file directory (default: <data_dir>/logs)")]
    pub log_dir: Option<PathBuf>,

    #[arg(long, default_value = "7", help = "Number of days to retain log files")]
    pub log_retention_days: u32,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Start the proxy server (default when no subcommand provided)")]
    Start {
        #[arg(short, long, help = "HTTP proxy port (overrides global -p)")]
        port: Option<u16>,
        #[arg(short = 'H', long, help = "Listen address (overrides global -H)")]
        host: Option<String>,
        #[arg(
            long,
            help = "Separate SOCKS5 port (overrides global; omit to share main port)"
        )]
        socks5_port: Option<u16>,
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
            help = "Domains to exclude from TLS interception (comma-separated, supports wildcards like *.example.com). Has higher priority than global switch."
        )]
        intercept_exclude: Option<String>,
        #[arg(
            long,
            help = "Domains to force TLS interception (comma-separated, supports wildcards). Has highest priority, works even when interception is disabled."
        )]
        intercept_include: Option<String>,
        #[arg(
            long,
            help = "Applications to exclude from TLS interception (comma-separated, supports wildcards like *Safari). Traffic from these apps will not be intercepted."
        )]
        app_intercept_exclude: Option<String>,
        #[arg(
            long,
            help = "Applications to force TLS interception (comma-separated, supports wildcards). Traffic from these apps will always be intercepted."
        )]
        app_intercept_include: Option<String>,
        #[arg(
            long,
            help = "Skip upstream server TLS certificate verification (dangerous, for testing only)"
        )]
        unsafe_ssl: bool,
        #[arg(
            long,
            help = "Disable automatic disconnect of affected connections when TLS config changes"
        )]
        no_disconnect_on_config_change: bool,
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
    #[command(about = "Upgrade bifrost to the latest version")]
    Upgrade {
        #[arg(short = 'y', long, help = "Skip confirmation prompt")]
        yes: bool,
    },
}

#[derive(Subcommand, Clone)]
pub enum RuleCommands {
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
pub enum CaCommands {
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
pub enum WhitelistCommands {
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
pub enum SystemProxyCommands {
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
pub enum ValueCommands {
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
