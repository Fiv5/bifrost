use std::sync::Arc;

use serde::Deserialize;

use bifrost_storage::{ConfigManager, RuleFile, RulesStorage};
use bifrost_sync::{SyncAction, SyncManager};

use crate::cli::RuleCommands;

pub fn handle_rule_command(action: RuleCommands) -> bifrost_core::Result<()> {
    match action {
        RuleCommands::Sync => handle_rule_sync(),
        RuleCommands::Rename { name, new_name } => handle_rule_rename(&name, &new_name),
        RuleCommands::Reorder { names } => handle_rule_reorder(&names),
        RuleCommands::Active => handle_rule_active(),
        other => handle_rule_local(other),
    }
}

fn handle_rule_local(action: RuleCommands) -> bifrost_core::Result<()> {
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
            let rule_content = load_rule_content(content, file)?;

            let rule = RuleFile::new(&name, rule_content);
            storage.save(&rule)?;
            println!("Rule '{}' added successfully.", name);
        }
        RuleCommands::Update {
            name,
            content,
            file,
        } => {
            let rule_content = load_rule_content(content, file)?;
            storage.update_content(&name, rule_content)?;
            println!("Rule '{}' updated successfully.", name);
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
        RuleCommands::Sync
        | RuleCommands::Rename { .. }
        | RuleCommands::Reorder { .. }
        | RuleCommands::Active => {
            unreachable!()
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_summary_deserialization_full() {
        let json = r#"{
            "total": 3,
            "rules": [
                {"name": "dev-proxy", "rule_count": 5, "group_id": null, "group_name": null},
                {"name": "shared-mock", "rule_count": 2, "group_id": "g1", "group_name": "Team A"},
                {"name": "team-headers", "rule_count": 1, "group_id": "g1", "group_name": "Team A"}
            ],
            "variable_conflicts": [
                {
                    "variable_name": "API_HOST",
                    "definitions": [
                        {"rule_name": "dev-proxy", "value_preview": "localhost:3000"},
                        {"rule_name": "shared-mock", "value_preview": "mock.example.com"}
                    ]
                }
            ],
            "merged_content": "example.com 127.0.0.1:3000\napi.example.com proxy://proxy:8080"
        }"#;
        let resp: ActiveSummaryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.total, 3);
        assert_eq!(resp.rules.len(), 3);
        assert!(resp.rules[0].group_id.is_none());
        assert_eq!(resp.rules[1].group_id.as_deref(), Some("g1"));
        assert_eq!(resp.rules[1].group_name.as_deref(), Some("Team A"));
        assert_eq!(resp.variable_conflicts.len(), 1);
        assert_eq!(resp.variable_conflicts[0].variable_name, "API_HOST");
        assert_eq!(resp.variable_conflicts[0].definitions.len(), 2);
        assert!(resp.merged_content.contains("example.com"));
        assert!(resp.merged_content.contains("proxy://proxy:8080"));
    }

    #[test]
    fn test_active_summary_deserialization_empty() {
        let json = r#"{"total": 0, "rules": [], "variable_conflicts": [], "merged_content": ""}"#;
        let resp: ActiveSummaryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.total, 0);
        assert!(resp.rules.is_empty());
        assert!(resp.variable_conflicts.is_empty());
        assert!(resp.merged_content.is_empty());
    }

    #[test]
    fn test_active_summary_deserialization_missing_optional_fields() {
        let json = r#"{"total": 1, "rules": [{"name": "test", "rule_count": 3}]}"#;
        let resp: ActiveSummaryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.total, 1);
        assert!(resp.rules[0].group_id.is_none());
        assert!(resp.rules[0].group_name.is_none());
        assert!(resp.variable_conflicts.is_empty());
        assert!(resp.merged_content.is_empty());
    }
}

fn load_rule_content(
    content: Option<String>,
    file: Option<std::path::PathBuf>,
) -> bifrost_core::Result<String> {
    if let Some(c) = content {
        Ok(c)
    } else if let Some(path) = file {
        Ok(std::fs::read_to_string(&path)?)
    } else {
        Err(bifrost_core::BifrostError::Config(
            "Either --content or --file must be provided".to_string(),
        ))
    }
}

fn handle_rule_sync() -> bifrost_core::Result<()> {
    println!("🔄 Starting rules sync...");

    let data_dir = bifrost_storage::data_dir();
    let config_manager = Arc::new(ConfigManager::new(data_dir)?);
    let config = futures::executor::block_on(config_manager.config());

    println!("   Remote: {}", config.sync.remote_base_url);
    println!("   Enabled: {}", config.sync.enabled);

    let sync_manager = SyncManager::new(config_manager, 0)?;

    let rt = tokio::runtime::Runtime::new().map_err(|e| {
        bifrost_core::BifrostError::Config(format!("failed to create tokio runtime: {e}"))
    })?;

    let result = rt.block_on(sync_manager.sync_once())?;

    if result.success {
        println!("✅ {}", result.message);
        if let Some(user) = &result.user {
            println!("   User: {} ({})", user.nickname, user.user_id);
        }
        println!("   Local rules: {}", result.local_rules);
        println!("   Remote rules: {}", result.remote_rules);
        if let Some(action) = &result.action {
            let action_str = match action {
                SyncAction::LocalPushed => "Local → Remote (pushed local changes)",
                SyncAction::RemotePulled => "Remote → Local (pulled remote changes)",
                SyncAction::Bidirectional => "Bidirectional (both pushed and pulled)",
                SyncAction::NoChange => "No changes needed",
            };
            println!("   Action: {}", action_str);
        }
    } else {
        println!("❌ {}", result.message);
        if let Some(user) = &result.user {
            println!("   User: {} ({})", user.nickname, user.user_id);
        }
    }

    Ok(())
}

fn handle_rule_rename(name: &str, new_name: &str) -> bifrost_core::Result<()> {
    let port = crate::process::read_runtime_port().unwrap_or(9900);
    let client = super::config::client::ConfigApiClient::new("127.0.0.1", port);

    client
        .rename_rule(name, new_name)
        .map_err(bifrost_core::BifrostError::Config)?;

    println!("Rule '{}' renamed to '{}'.", name, new_name);
    Ok(())
}

fn handle_rule_reorder(names: &[String]) -> bifrost_core::Result<()> {
    let port = crate::process::read_runtime_port().unwrap_or(9900);
    let client = super::config::client::ConfigApiClient::new("127.0.0.1", port);

    client
        .reorder_rules(names)
        .map_err(bifrost_core::BifrostError::Config)?;

    println!("Rules reordered successfully:");
    for (i, name) in names.iter().enumerate() {
        println!("  {}. {}", i + 1, name);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ActiveRuleItem {
    name: String,
    rule_count: usize,
    group_id: Option<String>,
    group_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VariableDefinition {
    rule_name: String,
    value_preview: String,
}

#[derive(Debug, Deserialize)]
struct VariableConflict {
    variable_name: String,
    definitions: Vec<VariableDefinition>,
}

#[derive(Debug, Deserialize)]
struct ActiveSummaryResponse {
    total: usize,
    rules: Vec<ActiveRuleItem>,
    #[serde(default)]
    variable_conflicts: Vec<VariableConflict>,
    #[serde(default)]
    merged_content: String,
}

fn handle_rule_active() -> bifrost_core::Result<()> {
    let port = crate::process::read_runtime_port().unwrap_or(9900);
    let url = format!(
        "http://127.0.0.1:{}/_bifrost/api/rules/active-summary",
        port
    );
    let response = bifrost_core::direct_ureq_agent_builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .get(&url)
        .call();

    let resp: ActiveSummaryResponse = match response {
        Ok(r) => r.into_json().map_err(|e| {
            bifrost_core::BifrostError::Config(format!("Failed to parse response: {e}"))
        })?,
        Err(e) => {
            return Err(bifrost_core::BifrostError::Config(format!(
                "Failed to connect to server on port {port}: {e}\nIs bifrost running? Try 'bifrost status' to check."
            )));
        }
    };

    println!("Active Rules Summary");
    println!("====================");
    println!();

    if resp.total == 0 {
        println!("No active rules.");
        return Ok(());
    }

    println!("Total active: {} rule file(s)", resp.total);
    println!();

    let own_rules: Vec<_> = resp.rules.iter().filter(|r| r.group_id.is_none()).collect();
    let mut group_map: std::collections::BTreeMap<String, (String, Vec<&ActiveRuleItem>)> =
        std::collections::BTreeMap::new();
    for r in resp.rules.iter().filter(|r| r.group_id.is_some()) {
        let gid = r.group_id.as_deref().unwrap_or_default();
        let entry = group_map.entry(gid.to_string()).or_insert_with(|| {
            let display = r.group_name.as_deref().unwrap_or(gid);
            (display.to_string(), Vec::new())
        });
        entry.1.push(r);
    }

    if !own_rules.is_empty() {
        println!("My Rules ({}):", own_rules.len());
        for rule in &own_rules {
            println!("  ⚡ {} ({} rules)", rule.name, rule.rule_count);
        }
        println!();
    }

    for (gid, (group_name, rules)) in &group_map {
        let label = if group_name != gid {
            format!("{group_name} ({gid})")
        } else {
            gid.clone()
        };
        println!("Group: {} ({} file(s)):", label, rules.len());
        for rule in rules {
            println!("  ⚡ {} ({} rules)", rule.name, rule.rule_count);
        }
        println!();
    }

    if !resp.variable_conflicts.is_empty() {
        println!("⚠  Variable Conflicts ({}):", resp.variable_conflicts.len());
        for conflict in &resp.variable_conflicts {
            println!("  {{{}}}", conflict.variable_name);
            for def in &conflict.definitions {
                println!("    - {}: {}", def.rule_name, def.value_preview);
            }
        }
        println!();
    }

    println!("Merged Rules (in parsing order)");
    println!("-------------------------------");
    let content = resp.merged_content.trim();
    if content.is_empty() {
        println!("(empty)");
    } else {
        println!("{}", content);
    }
    println!();

    Ok(())
}
