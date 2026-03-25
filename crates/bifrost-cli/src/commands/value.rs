use bifrost_storage::ValuesStorage;

use crate::cli::ValueCommands;

pub fn handle_value_command(action: ValueCommands) -> bifrost_core::Result<()> {
    let values_dir = bifrost_storage::data_dir().join("values");
    let mut storage = ValuesStorage::with_dir(values_dir.clone())?;

    match action {
        ValueCommands::List => {
            let entries = storage.list_entries()?;
            if entries.is_empty() {
                println!("No values defined.");
                println!();
                println!("Values directory: {}", values_dir.display());
            } else {
                println!("Values ({}):", entries.len());
                println!("====================");
                for entry in entries {
                    let preview = entry.value.replace('\n', "\\n");
                    println!("  {} = {}", entry.name, preview);
                }
                println!();
                println!("Values directory: {}", values_dir.display());
            }
        }
        ValueCommands::Show { name } => {
            if let Some(value) = storage.get_value(&name) {
                println!("{}", value);
            } else {
                return Err(bifrost_core::BifrostError::NotFound(format!(
                    "Value '{}' not found",
                    name
                )));
            }
        }
        ValueCommands::Add { name, value } => {
            storage.set_value(&name, &value)?;
            println!("Value '{}' added successfully.", name);
        }
        ValueCommands::Update { name, value } => {
            storage.update(&name, &value)?;
            println!("Value '{}' updated successfully.", name);
        }
        ValueCommands::Delete { name } => {
            if storage.exists(&name) {
                storage.remove_value(&name)?;
                println!("Value '{}' deleted successfully.", name);
            } else {
                return Err(bifrost_core::BifrostError::NotFound(format!(
                    "Value '{}' not found",
                    name
                )));
            }
        }
        ValueCommands::Import { file } => {
            if !file.exists() {
                return Err(bifrost_core::BifrostError::NotFound(format!(
                    "File not found: {}",
                    file.display()
                )));
            }
            let count = storage.load_from_file(&file)?;
            println!("Imported {} value(s) from '{}'.", count, file.display());
        }
    }

    Ok(())
}
