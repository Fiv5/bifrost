use bifrost_core::init_logging;
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
            ref intercept_include,
            ref app_intercept_exclude,
            ref app_intercept_include,
            unsafe_ssl,
            no_disconnect_on_config_change,
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
            intercept_include.clone(),
            app_intercept_exclude.clone(),
            app_intercept_include.clone(),
            unsafe_ssl,
            no_disconnect_on_config_change,
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
