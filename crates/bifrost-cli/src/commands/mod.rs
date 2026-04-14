mod admin;
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

use serde_json::Value;

pub use admin::*;
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

    for line in format_version_check_lines(&info) {
        println!("{}", line);
    }

    Ok(())
}

fn format_version_check_lines(info: &Value) -> Vec<String> {
    let current = info.get("current_version").and_then(|v| v.as_str());
    let latest = info.get("latest_version").and_then(|v| v.as_str());
    let has_update = info.get("has_update").and_then(|v| v.as_bool());
    let mut lines = Vec::new();

    if let Some(current) = current {
        lines.push(format!("Current version: {}", current));
    }

    if let Some(latest) = latest {
        lines.push(format!("Latest version: {}", latest));
    }

    match (has_update, latest) {
        (Some(true), Some(_)) => {
            lines.push("Update available! Run 'bifrost upgrade' to update.".to_string())
        }
        (Some(false), Some(_)) => lines.push("You are running the latest version.".to_string()),
        (_, None) => lines.push(
            "Could not determine the latest version. Check your network connection and try again."
                .to_string(),
        ),
        _ => {}
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::format_version_check_lines;
    use serde_json::json;

    #[test]
    fn version_check_formats_update_available_response() {
        let lines = format_version_check_lines(&json!({
            "current_version": "1.0.0",
            "latest_version": "1.1.0",
            "has_update": true,
        }));

        assert_eq!(
            lines,
            vec![
                "Current version: 1.0.0".to_string(),
                "Latest version: 1.1.0".to_string(),
                "Update available! Run 'bifrost upgrade' to update.".to_string(),
            ]
        );
    }

    #[test]
    fn version_check_formats_latest_response() {
        let lines = format_version_check_lines(&json!({
            "current_version": "1.1.0",
            "latest_version": "1.1.0",
            "has_update": false,
        }));

        assert_eq!(
            lines,
            vec![
                "Current version: 1.1.0".to_string(),
                "Latest version: 1.1.0".to_string(),
                "You are running the latest version.".to_string(),
            ]
        );
    }

    #[test]
    fn version_check_formats_missing_latest_version_response() {
        let lines = format_version_check_lines(&json!({
            "current_version": "1.1.0",
            "latest_version": null,
            "has_update": false,
        }));

        assert_eq!(
            lines,
            vec![
                "Current version: 1.1.0".to_string(),
                "Could not determine the latest version. Check your network connection and try again."
                    .to_string(),
            ]
        );
    }
}
