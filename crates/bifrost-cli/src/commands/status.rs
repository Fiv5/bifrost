use serde::Deserialize;

use crate::process::{is_process_running, read_pid};

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

pub fn run_status() -> bifrost_core::Result<()> {
    println!("Bifrost Proxy Status");
    println!("====================");

    let is_running = match read_pid() {
        Some(pid) => {
            if is_process_running(pid) {
                println!("Status: Running");
                println!("PID: {}", pid);
                true
            } else {
                println!("Status: Stopped (stale PID file exists)");
                println!("Stale PID: {}", pid);
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

    if is_running {
        let port = 9900;
        match fetch_rules_from_api(port) {
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

    Ok(())
}
