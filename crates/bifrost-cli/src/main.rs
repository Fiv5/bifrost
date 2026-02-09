use std::path::PathBuf;

use bifrost_admin::AdminState;
use bifrost_core::init_logging;
use bifrost_proxy::{ProxyConfig, ProxyServer};
use bifrost_storage::{RuleFile, RulesStorage, StateManager};
use bifrost_tls::{
    generate_root_ca, get_platform_name, load_root_ca, save_root_ca, CertInstaller, CertStatus,
};
use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Select};
use tracing::info;

#[derive(Parser)]
#[command(name = "bifrost")]
#[command(version = "1.0.0")]
#[command(about = "High-performance HTTP proxy written in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, default_value = "8899")]
    port: u16,

    #[arg(short = 'H', long, default_value = "0.0.0.0")]
    host: String,

    #[arg(long, help = "SOCKS5 proxy port")]
    socks5_port: Option<u16>,

    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start the proxy server")]
    Start {
        #[arg(short, long, help = "Run as daemon")]
        daemon: bool,
        #[arg(long, help = "Skip CA certificate installation check")]
        skip_cert_check: bool,
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
}

#[derive(Subcommand)]
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

#[derive(Subcommand)]
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

fn get_bifrost_dir() -> bifrost_core::Result<PathBuf> {
    dirs::home_dir()
        .map(|p| p.join(".bifrost"))
        .ok_or_else(|| bifrost_core::BifrostError::Config("Cannot find home directory".to_string()))
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
    let cli = Cli::parse();

    if let Err(e) = init_logging(&cli.log_level) {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    let result = match cli.command {
        Some(Commands::Start {
            daemon,
            skip_cert_check,
        }) => run_start(&cli, daemon, skip_cert_check),
        Some(Commands::Stop) => run_stop(),
        Some(Commands::Status) => run_status(),
        Some(Commands::Rule { action }) => handle_rule_command(action),
        Some(Commands::Ca { action }) => handle_ca_command(action),
        None => run_start(&cli, false, false),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_start(cli: &Cli, daemon: bool, skip_cert_check: bool) -> bifrost_core::Result<()> {
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

    let proxy_config = ProxyConfig {
        port: cli.port,
        host: cli.host.clone(),
        socks5_port: cli.socks5_port,
        ..Default::default()
    };

    if daemon {
        #[cfg(unix)]
        {
            run_daemon(proxy_config)?;
        }
        #[cfg(not(unix))]
        {
            return Err(bifrost_core::BifrostError::Config(
                "Daemon mode is not supported on this platform".to_string(),
            ));
        }
    } else {
        run_foreground(proxy_config)?;
    }

    Ok(())
}

fn run_foreground(config: ProxyConfig) -> bifrost_core::Result<()> {
    let pid = std::process::id();
    write_pid(pid)?;

    println!(
        "Bifrost proxy server starting on {}:{}",
        config.host, config.port
    );
    if let Some(socks5_port) = config.socks5_port {
        println!("SOCKS5 proxy enabled on port {}", socks5_port);
    }
    println!("Press Ctrl+C to stop");
    println!("PID: {}", pid);

    let rt = tokio::runtime::Runtime::new().map_err(|e| {
        bifrost_core::BifrostError::Config(format!("Failed to create runtime: {}", e))
    })?;

    rt.block_on(async {
        let admin_state = AdminState::new(config.port);
        let server = ProxyServer::new(config).with_admin_state(admin_state);

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

    remove_pid()?;
    println!("Bifrost proxy stopped.");
    Ok(())
}

#[cfg(unix)]
fn run_daemon(config: ProxyConfig) -> bifrost_core::Result<()> {
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

            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                bifrost_core::BifrostError::Config(format!("Failed to create runtime: {}", e))
            })?;

            rt.block_on(async {
                let admin_state = AdminState::new(config.port);
                let server = ProxyServer::new(config).with_admin_state(admin_state);
                if let Err(e) = server.run().await {
                    eprintln!("Server error: {}", e);
                }
            });

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
            println!("Certificate: {}", ca_cert_path.display());
            println!("Private Key: {}", ca_key_path.display());

            let cert_meta = std::fs::metadata(&ca_cert_path)?;
            if let Ok(modified) = cert_meta.modified() {
                if let Ok(duration) = modified.elapsed() {
                    let days = duration.as_secs() / 86400;
                    println!("Created: {} days ago", days);
                }
            }

            let installer = CertInstaller::new(&ca_cert_path);
            match installer.check_status() {
                Ok(status) => {
                    println!("System trust status: {}", status);
                }
                Err(e) => {
                    println!("Could not check trust status: {}", e);
                }
            }
        }
    }

    Ok(())
}

fn check_and_install_certificate() -> bifrost_core::Result<()> {
    let cert_dir = get_bifrost_dir()?.join("certs");
    let ca_key_path = cert_dir.join("ca.key");
    let ca_cert_path = cert_dir.join("ca.crt");

    if !ca_cert_path.exists() || !ca_key_path.exists() {
        println!("CA certificate not found. Generating...");
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
