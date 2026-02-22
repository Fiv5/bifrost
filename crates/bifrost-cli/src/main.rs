use bifrost_core::{init_logging_with_config, install_panic_hook, LogConfig, LogOutput};
use bifrost_storage::data_dir;
use bifrost_tls::init_crypto_provider;
use clap::Parser;

mod cli;
mod commands;
mod config;
mod help;
mod parsing;
mod process;

use cli::{Cli, Commands};
use commands::{
    handle_ca_command, handle_rule_command, handle_system_proxy_command, handle_value_command,
    handle_whitelist_command, run_start, run_status, run_stop,
};

fn main() {
    install_panic_hook();
    init_crypto_provider();

    let cli = Cli::parse();

    let log_dir = cli
        .log_dir
        .clone()
        .unwrap_or_else(|| data_dir().join("logs"));

    let log_outputs = LogOutput::parse(&cli.log_output);
    let log_outputs = if log_outputs.is_empty() {
        vec![LogOutput::Console, LogOutput::File]
    } else {
        log_outputs
    };

    let log_config = LogConfig::new(cli.log_level.clone(), log_dir)
        .with_outputs(log_outputs)
        .with_retention_days(cli.log_retention_days);

    let _log_guard = match init_logging_with_config(&log_config) {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to initialize logging: {}", e);
            std::process::exit(1);
        }
    };

    let result = match cli.command {
        Some(Commands::Start {
            port,
            host,
            socks5_port,
            daemon,
            skip_cert_check,
            ref access_mode,
            ref whitelist,
            allow_lan,
            no_intercept,
            ref intercept_exclude,
            ref intercept_include,
            ref app_intercept_exclude,
            ref app_intercept_include,
            unsafe_ssl,
            no_disconnect_on_config_change,
            ref rules,
            ref rules_file,
            system_proxy,
            ref proxy_bypass,
        }) => {
            let effective_port = port.unwrap_or(cli.port);
            let effective_host = host.clone().unwrap_or_else(|| cli.host.clone());
            let effective_socks5_port = socks5_port.or(cli.socks5_port);
            run_start(
                effective_port,
                effective_host,
                effective_socks5_port,
                &cli.log_level,
                daemon,
                skip_cert_check,
                access_mode.clone(),
                whitelist.clone(),
                allow_lan,
                no_intercept,
                intercept_exclude.clone(),
                intercept_include.clone(),
                app_intercept_exclude.clone(),
                app_intercept_include.clone(),
                unsafe_ssl,
                no_disconnect_on_config_change,
                rules.clone(),
                rules_file.clone(),
                system_proxy,
                proxy_bypass.clone(),
            )
        }
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
            cli.port,
            cli.host.clone(),
            cli.socks5_port,
            &cli.log_level,
            false,
            false,
            None,
            None,
            false,
            false,
            None,
            None,
            None,
            None,
            false,
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
