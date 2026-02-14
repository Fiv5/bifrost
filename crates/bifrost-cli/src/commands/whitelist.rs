use crate::cli::WhitelistCommands;
use crate::config::{load_config, save_config};

pub fn handle_whitelist_command(action: WhitelistCommands) -> bifrost_core::Result<()> {
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
