use std::path::PathBuf;

use super::config::client::ConfigApiClient;
use crate::cli::{ExportCommands, ImportArgs};

pub fn handle_import_command(args: ImportArgs, host: &str, port: u16) -> bifrost_core::Result<()> {
    let client = ConfigApiClient::new(host, port);

    let content = std::fs::read_to_string(&args.file).map_err(|e| {
        bifrost_core::BifrostError::Config(format!(
            "Failed to read file '{}': {}",
            args.file.display(),
            e
        ))
    })?;

    if args.detect_only {
        let result = client
            .bifrost_file_detect(&content)
            .map_err(bifrost_core::BifrostError::Config)?;

        println!("File Type Detection");
        println!("===================");
        if let Some(file_type) = result.get("file_type").and_then(|v| v.as_str()) {
            println!("Type: {}", file_type);
        }
        if let Some(meta) = result.get("meta") {
            println!(
                "Meta: {}",
                serde_json::to_string_pretty(meta).unwrap_or_default()
            );
        }
        return Ok(());
    }

    let result = client
        .bifrost_file_import(&content)
        .map_err(bifrost_core::BifrostError::Config)?;

    println!("Import Result");
    println!("=============");
    if let Some(success) = result.get("success").and_then(|v| v.as_bool()) {
        println!("Success: {}", success);
    }
    if let Some(file_type) = result.get("file_type").and_then(|v| v.as_str()) {
        println!("Type: {}", file_type);
    }
    if let Some(data) = result.get("data").and_then(|v| v.as_object()) {
        if let Some(count) = data.get("rule_count").and_then(|v| v.as_u64()) {
            println!("Rules imported: {}", count);
        }
        if let Some(names) = data.get("rule_names").and_then(|v| v.as_array()) {
            for name in names {
                if let Some(n) = name.as_str() {
                    println!("  - {}", n);
                }
            }
        }
        if let Some(count) = data.get("script_count").and_then(|v| v.as_u64()) {
            println!("Scripts imported: {}", count);
        }
        if let Some(count) = data.get("value_count").and_then(|v| v.as_u64()) {
            println!("Values imported: {}", count);
        }
        if let Some(count) = data.get("record_count").and_then(|v| v.as_u64()) {
            println!("Records imported: {}", count);
        }
    }
    if let Some(warnings) = result.get("warnings").and_then(|v| v.as_array()) {
        if !warnings.is_empty() {
            println!();
            println!("Warnings:");
            for w in warnings {
                if let Some(s) = w.as_str() {
                    println!("  ⚠ {}", s);
                }
            }
        }
    }

    Ok(())
}

pub fn handle_export_command(
    action: ExportCommands,
    host: &str,
    port: u16,
) -> bifrost_core::Result<()> {
    let client = ConfigApiClient::new(host, port);

    match action {
        ExportCommands::Rules {
            names,
            description,
            output,
        } => export_rules(&client, &names, description.as_deref(), output),
        ExportCommands::Values {
            names,
            description,
            output,
        } => {
            let names_ref = if names.is_empty() {
                None
            } else {
                Some(names.as_slice())
            };
            export_values(&client, names_ref, description.as_deref(), output)
        }
        ExportCommands::Scripts {
            names,
            description,
            output,
        } => export_scripts(&client, &names, description.as_deref(), output),
    }
}

fn export_rules(
    client: &ConfigApiClient,
    names: &[String],
    description: Option<&str>,
    output: Option<PathBuf>,
) -> bifrost_core::Result<()> {
    let content = client
        .bifrost_file_export_rules(names, description)
        .map_err(bifrost_core::BifrostError::Config)?;

    write_export(content, output, "rules")
}

fn export_values(
    client: &ConfigApiClient,
    names: Option<&[String]>,
    description: Option<&str>,
    output: Option<PathBuf>,
) -> bifrost_core::Result<()> {
    let content = client
        .bifrost_file_export_values(names, description)
        .map_err(bifrost_core::BifrostError::Config)?;

    write_export(content, output, "values")
}

fn export_scripts(
    client: &ConfigApiClient,
    names: &[String],
    description: Option<&str>,
    output: Option<PathBuf>,
) -> bifrost_core::Result<()> {
    let content = client
        .bifrost_file_export_scripts(names, description)
        .map_err(bifrost_core::BifrostError::Config)?;

    write_export(content, output, "scripts")
}

fn write_export(
    content: String,
    output: Option<PathBuf>,
    type_name: &str,
) -> bifrost_core::Result<()> {
    match output {
        Some(path) => {
            std::fs::write(&path, &content).map_err(|e| {
                bifrost_core::BifrostError::Config(format!(
                    "Failed to write to '{}': {}",
                    path.display(),
                    e
                ))
            })?;
            println!("Exported {} to: {}", type_name, path.display());
        }
        None => {
            print!("{}", content);
        }
    }
    Ok(())
}
