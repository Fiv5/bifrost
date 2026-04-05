mod bifrost_file;
mod ca;
mod completions;
pub(crate) mod config;
mod group;
mod install_skill;
mod metrics;
mod rule;
mod script;
mod search;
mod start;
mod status;
mod status_tui;
mod stop;
mod sync_cmd;
mod system_proxy;
mod traffic;
mod update_check;
mod upgrade;
mod value;
mod whitelist;

pub use ca::*;
pub use config::handle_config_command;
pub use group::handle_group_command;
pub use install_skill::handle_install_skill;
pub use rule::*;
pub use script::*;
pub use search::{run_search, OutputFormat, SearchOptions};
pub use start::*;
pub use status::*;
pub use status_tui::*;
pub use stop::*;
pub use system_proxy::*;
pub use traffic::{
    run_traffic_clear, run_traffic_get, run_traffic_list, TrafficGetOptions, TrafficListOptions,
};
pub use update_check::*;
pub use upgrade::*;
pub use value::*;
pub use whitelist::*;

pub use bifrost_file::{handle_export_command, handle_import_command};
pub use metrics::handle_metrics_command;
pub use sync_cmd::handle_sync_command;

pub fn handle_version_check(host: &str, port: u16) -> bifrost_core::Result<()> {
    let client = config::client::ConfigApiClient::new(host, port);
    let info = client
        .version_check()
        .map_err(bifrost_core::BifrostError::Config)?;

    if let Some(current) = info.get("current").and_then(|v| v.as_str()) {
        println!("Current version: {}", current);
    }
    if let Some(latest) = info.get("latest").and_then(|v| v.as_str()) {
        println!("Latest version: {}", latest);
    }
    if let Some(update) = info.get("update_available").and_then(|v| v.as_bool()) {
        if update {
            println!("Update available! Run 'bifrost upgrade' to update.");
        } else {
            println!("You are running the latest version.");
        }
    }
    Ok(())
}
