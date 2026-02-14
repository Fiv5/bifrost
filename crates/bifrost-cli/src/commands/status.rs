use bifrost_storage::StateManager;

use crate::process::{is_process_running, read_pid};

pub fn run_status() -> bifrost_core::Result<()> {
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
