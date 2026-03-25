use std::sync::Arc;

use bifrost_storage::{ConfigManager, RuleFile, RulesStorage};
use bifrost_sync::{SyncAction, SyncManager};

use crate::cli::RuleCommands;

pub fn handle_rule_command(action: RuleCommands) -> bifrost_core::Result<()> {
    match action {
        RuleCommands::Sync => handle_rule_sync(),
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
        RuleCommands::Sync => unreachable!(),
    }

    Ok(())
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
