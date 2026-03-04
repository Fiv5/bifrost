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

use cli::{Cli, Commands, TrafficCommands};
use commands::{
    check_and_print_update_notice, handle_ca_command, handle_config_command, handle_rule_command,
    handle_system_proxy_command, handle_upgrade, handle_value_command, handle_whitelist_command,
    run_search, run_start, run_status, run_status_tui, run_stop, run_traffic_get, run_traffic_list,
    OutputFormat, SearchOptions, TrafficGetOptions, TrafficListOptions,
};
use process::read_runtime_port;

const DEFAULT_PORT: u16 = 9900;

fn get_effective_port(cli_port: u16) -> u16 {
    if cli_port != DEFAULT_PORT {
        return cli_port;
    }
    read_runtime_port().unwrap_or(DEFAULT_PORT)
}

fn main() {
    install_panic_hook();
    init_crypto_provider();

    let cli = Cli::parse();

    let is_daemon_mode = matches!(&cli.command, Some(Commands::Start { daemon: true, .. }));

    let _log_guard = if is_daemon_mode {
        None
    } else {
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

        match init_logging_with_config(&log_config) {
            Ok(guard) => Some(guard),
            Err(e) => {
                eprintln!("Failed to initialize logging: {}", e);
                std::process::exit(1);
            }
        }
    };

    check_and_print_update_notice();

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
        Some(Commands::Status { tui }) => {
            if tui {
                run_status_tui()
            } else {
                run_status()
            }
        }
        Some(Commands::Rule { action }) => handle_rule_command(action),
        Some(Commands::Ca { action }) => handle_ca_command(action),
        Some(Commands::Whitelist { action }) => handle_whitelist_command(action),
        Some(Commands::SystemProxy { ref action }) => {
            handle_system_proxy_command(&cli, action.clone())
        }
        Some(Commands::Value { action }) => handle_value_command(action),
        Some(Commands::Upgrade { yes }) => handle_upgrade(yes),
        Some(Commands::Config { action }) => {
            handle_config_command(action, "127.0.0.1", get_effective_port(cli.port))
        }
        Some(Commands::Search {
            keyword,
            interactive,
            limit,
            format,
            url,
            headers,
            body,
            status,
            method,
            protocol,
            content_type,
            domain,
            no_color,
        }) => {
            let is_interactive = interactive || keyword.is_none();
            let options = SearchOptions {
                keyword: keyword.unwrap_or_default(),
                port: get_effective_port(cli.port),
                limit,
                format: format.parse().unwrap_or(OutputFormat::Table),
                interactive: is_interactive,
                scope_url: url,
                scope_headers: headers,
                scope_body: body,
                filter_status: status,
                filter_method: method,
                filter_protocol: protocol,
                filter_content_type: content_type,
                filter_domain: domain,
                no_color,
            };
            let exit_code = run_search(options);
            std::process::exit(exit_code);
        }
        Some(Commands::Traffic { action }) => match action {
            TrafficCommands::List {
                limit,
                cursor,
                direction,
                method,
                status,
                status_min,
                status_max,
                protocol,
                host,
                url,
                path,
                content_type,
                client_ip,
                client_app,
                has_rule_hit,
                is_websocket,
                is_sse,
                is_tunnel,
                format,
                no_color,
            } => {
                let options = TrafficListOptions {
                    port: get_effective_port(cli.port),
                    limit,
                    cursor,
                    direction,
                    method,
                    status,
                    status_min,
                    status_max,
                    protocol,
                    host,
                    url,
                    path,
                    content_type,
                    client_ip,
                    client_app,
                    has_rule_hit,
                    is_websocket,
                    is_sse,
                    is_tunnel,
                    format: format.parse().unwrap_or(OutputFormat::Table),
                    no_color,
                };
                run_traffic_list(options)
            }
            TrafficCommands::Get {
                id,
                request_body,
                response_body,
                format,
            } => {
                let options = TrafficGetOptions {
                    port: get_effective_port(cli.port),
                    id,
                    request_body,
                    response_body,
                    format: format.parse().unwrap_or(OutputFormat::JsonPretty),
                };
                run_traffic_get(options)
            }
            TrafficCommands::Search {
                keyword,
                interactive,
                limit,
                format,
                url,
                headers,
                body,
                status,
                method,
                protocol,
                content_type,
                domain,
                no_color,
            } => {
                let is_interactive = interactive || keyword.is_none();
                let options = SearchOptions {
                    keyword: keyword.unwrap_or_default(),
                    port: get_effective_port(cli.port),
                    limit,
                    format: format.parse().unwrap_or(OutputFormat::Table),
                    interactive: is_interactive,
                    scope_url: url,
                    scope_headers: headers,
                    scope_body: body,
                    filter_status: status,
                    filter_method: method,
                    filter_protocol: protocol,
                    filter_content_type: content_type,
                    filter_domain: domain,
                    no_color,
                };
                let exit_code = run_search(options);
                std::process::exit(exit_code);
            }
        },
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
