use serde::Deserialize;

use super::rule::{
    fetch_active_summary_from_api, format_active_summary_lines, ActiveSummaryResponse,
};
use crate::process::{is_process_running, read_runtime_info};

#[derive(Debug, Deserialize)]
struct RuleGroup {
    name: String,
    enabled: bool,
    rule_count: usize,
}

fn fetch_rules_from_api(port: u16) -> Option<Vec<RuleGroup>> {
    let url = format!("http://127.0.0.1:{}/_bifrost/api/rules", port);
    let response = bifrost_core::direct_ureq_agent_builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .get(&url)
        .call();
    match response {
        Ok(resp) => resp.into_json().ok(),
        Err(_) => None,
    }
}

fn format_active_summary_status_block(
    is_running: bool,
    active_summary: Result<&ActiveSummaryResponse, &str>,
) -> Vec<String> {
    if !is_running {
        return Vec::new();
    }

    let mut lines = vec![String::new()];
    match active_summary {
        Ok(summary) => lines.extend(format_active_summary_lines(summary)),
        Err(message) => {
            lines.push("Active Rules Summary".to_string());
            lines.push("====================".to_string());
            lines.push(String::new());
            lines.push(format!(
                "(Unable to fetch active rule summary from running server: {message})"
            ));
        }
    }

    lines
}

pub fn run_status() -> bifrost_core::Result<()> {
    println!("Bifrost Proxy Status");
    println!("====================");

    let runtime_info = read_runtime_info();

    let is_running = match &runtime_info {
        Some(info) => {
            if is_process_running(info.pid) {
                println!("Status: Running");
                println!("PID: {}", info.pid);
                println!("Port: {}", info.port);
                if let Some(ref host) = info.host {
                    println!("Host: {}", host);
                }
                if let Some(socks5_port) = info.socks5_port {
                    println!("SOCKS5 Port: {}", socks5_port);
                }
                true
            } else {
                println!("Status: Stopped (stale PID file exists)");
                println!("Stale PID: {}", info.pid);
                false
            }
        }
        None => {
            println!("Status: Stopped");
            false
        }
    };

    println!();

    println!("Rule Groups");
    println!("-----------");
    let runtime_port = runtime_info.as_ref().map(|info| info.port).unwrap_or(9900);

    if is_running {
        match fetch_rules_from_api(runtime_port) {
            Some(groups) => {
                let enabled_groups: Vec<_> = groups.iter().filter(|g| g.enabled).collect();
                let disabled_groups: Vec<_> = groups.iter().filter(|g| !g.enabled).collect();

                println!("Enabled rule groups: {}", enabled_groups.len());
                for group in &enabled_groups {
                    println!("  - {} ({} rules)", group.name, group.rule_count);
                }

                if !disabled_groups.is_empty() {
                    println!("Disabled rule groups: {}", disabled_groups.len());
                    for group in &disabled_groups {
                        println!("  - {} ({} rules)", group.name, group.rule_count);
                    }
                }
            }
            None => {
                println!("(Unable to fetch rule information from running server)");
            }
        }
    } else {
        println!("(Server not running, rule information unavailable)");
    }

    if is_running {
        let active_summary_lines = match fetch_active_summary_from_api(runtime_port) {
            Ok(summary) => format_active_summary_status_block(true, Ok(&summary)),
            Err(err) => {
                let err_message = err.to_string();
                format_active_summary_status_block(true, Err(&err_message))
            }
        };

        for line in active_summary_lines {
            println!("{}", line);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::format_active_summary_status_block;
    use crate::commands::rule::{ActiveRuleItem, ActiveSummaryResponse};

    #[test]
    fn status_running_includes_active_summary_block() {
        let summary = ActiveSummaryResponse {
            total: 1,
            rules: vec![ActiveRuleItem {
                name: "demo".to_string(),
                rule_count: 2,
                group_id: None,
                group_name: None,
            }],
            variable_conflicts: Vec::new(),
            merged_content: "example.com statusCode://200".to_string(),
        };

        let lines = format_active_summary_status_block(true, Ok(&summary));
        let output = lines.join("\n");

        assert!(output.contains("Active Rules Summary"));
        assert!(output.contains("Merged Rules (in parsing order)"));
        assert!(output.contains("example.com statusCode://200"));
    }

    #[test]
    fn status_stopped_does_not_include_active_summary_block() {
        let summary = ActiveSummaryResponse {
            total: 1,
            rules: vec![ActiveRuleItem {
                name: "demo".to_string(),
                rule_count: 1,
                group_id: None,
                group_name: None,
            }],
            variable_conflicts: Vec::new(),
            merged_content: "example.com statusCode://200".to_string(),
        };

        let lines = format_active_summary_status_block(false, Ok(&summary));

        assert!(lines.is_empty());
    }
}
