use bifrost_storage::{RuleFile, RulesStorage};

use crate::cli::RuleCommands;

pub fn handle_rule_command(action: RuleCommands) -> bifrost_core::Result<()> {
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
            let rule_content = if let Some(c) = content {
                c
            } else if let Some(path) = file {
                std::fs::read_to_string(&path)?
            } else {
                return Err(bifrost_core::BifrostError::Config(
                    "Either --content or --file must be provided".to_string(),
                ));
            };

            let rule = RuleFile::new(&name, rule_content);
            storage.save(&rule)?;
            println!("Rule '{}' added successfully.", name);
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
    }

    Ok(())
}
