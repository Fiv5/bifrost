use bifrost_core::{BifrostError, Result};
use bifrost_storage::ValuesStorage;
use dialoguer::{theme::ColorfulTheme, Password};

use crate::cli::{AdminCommands, AdminRemoteCommands};

fn open_values_storage() -> Result<ValuesStorage> {
    let values_dir = bifrost_storage::data_dir().join("values");
    ValuesStorage::with_dir(values_dir)
}

fn ensure_username(storage: &mut ValuesStorage, username: &str) -> Result<()> {
    let u = username.trim();
    if u.is_empty() {
        return Err(BifrostError::Config("Username cannot be empty".to_string()));
    }
    storage.set_value(bifrost_admin::ADMIN_AUTH_USERNAME_KEY, u)?;
    Ok(())
}

fn prompt_new_password() -> std::result::Result<String, dialoguer::Error> {
    Password::with_theme(&ColorfulTheme::default())
        .with_prompt("New admin password")
        .with_confirmation("Confirm password", "Passwords do not match")
        .allow_empty_password(false)
        .interact()
}

fn set_password(storage: &mut ValuesStorage, password: &str) -> Result<()> {
    let hashed = bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| BifrostError::Storage(format!("Failed to hash password: {e}")))?;
    storage.set_value(bifrost_admin::ADMIN_AUTH_PASSWORD_HASH_KEY, &hashed)?;
    Ok(())
}

fn set_remote_enabled(storage: &mut ValuesStorage, enabled: bool) -> Result<()> {
    storage.set_value(
        bifrost_admin::ADMIN_REMOTE_ACCESS_ENABLED_KEY,
        if enabled { "true" } else { "false" },
    )?;
    Ok(())
}

fn get_remote_enabled(storage: &ValuesStorage) -> bool {
    storage
        .get_value(bifrost_admin::ADMIN_REMOTE_ACCESS_ENABLED_KEY)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn has_password(storage: &ValuesStorage) -> bool {
    storage
        .get_value(bifrost_admin::ADMIN_AUTH_PASSWORD_HASH_KEY)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

pub fn handle_admin_command(action: AdminCommands) -> Result<()> {
    match action {
        AdminCommands::Remote { action } => {
            let mut storage = open_values_storage()?;
            match action {
                AdminRemoteCommands::Enable => {
                    if !has_password(&storage) {
                        println!(
                            "Admin password not set yet. Please set a password to enable remote access."
                        );
                        let pwd = prompt_new_password().map_err(|e| {
                            BifrostError::Config(format!("Failed to read password: {e}"))
                        })?;
                        ensure_username(&mut storage, "admin")?;
                        set_password(&mut storage, &pwd)?;
                    }
                    set_remote_enabled(&mut storage, true)?;
                    println!("Remote admin access: enabled");
                }
                AdminRemoteCommands::Disable => {
                    set_remote_enabled(&mut storage, false)?;
                    println!("Remote admin access: disabled");
                }
                AdminRemoteCommands::Status => {
                    let enabled = get_remote_enabled(&storage);
                    let username = storage
                        .get_value(bifrost_admin::ADMIN_AUTH_USERNAME_KEY)
                        .unwrap_or_else(|| "admin".to_string());
                    println!(
                        "Remote admin access: {}",
                        if enabled { "enabled" } else { "disabled" }
                    );
                    println!("Admin username: {}", username.trim());
                    println!(
                        "Admin password: {}",
                        if has_password(&storage) {
                            "set"
                        } else {
                            "not set"
                        }
                    );
                    println!(
                        "Audit DB: {}",
                        bifrost_admin::admin_audit::audit_db_path()?.display()
                    );
                }
            }
        }
        AdminCommands::Passwd { username } => {
            let mut storage = open_values_storage()?;
            ensure_username(&mut storage, &username)?;
            let pwd = prompt_new_password()
                .map_err(|e| BifrostError::Config(format!("Failed to read password: {e}")))?;
            set_password(&mut storage, &pwd)?;
            println!("Admin password updated.");
        }
        AdminCommands::RevokeAll => {
            let mut storage = open_values_storage()?;
            let ts = chrono::Utc::now().timestamp();
            storage.set_value(bifrost_admin::ADMIN_AUTH_REVOKE_BEFORE_KEY, &ts.to_string())?;
            println!("All admin sessions revoked (revoke_before={}).", ts);
        }
        AdminCommands::Audit {
            limit,
            offset,
            json,
        } => {
            let limit = limit.clamp(1, 500);
            let total = bifrost_admin::admin_audit::count_logins()?;
            let items = bifrost_admin::admin_audit::list_logins(limit, offset)?;
            if json {
                let out = serde_json::json!({
                    "total": total,
                    "limit": limit,
                    "offset": offset,
                    "items": items,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string())
                );
                return Ok(());
            }

            if items.is_empty() {
                println!("No audit records.");
                println!(
                    "Audit DB: {}",
                    bifrost_admin::admin_audit::audit_db_path()?.display()
                );
                return Ok(());
            }

            println!(
                "Admin login audit (total: {}, showing: {}):",
                total,
                items.len()
            );
            println!("====================================================");
            for item in items {
                let ts = chrono::DateTime::<chrono::Utc>::from_timestamp(item.ts, 0)
                    .map(|t| t.to_rfc3339())
                    .unwrap_or_else(|| item.ts.to_string());
                let ua_preview = if item.ua.len() > 120 {
                    format!("{}...", &item.ua[..120])
                } else {
                    item.ua.clone()
                };
                println!(
                    "- id={} ts={} user={} ip={} ua={}",
                    item.id, ts, item.username, item.ip, ua_preview
                );
            }
            println!(
                "Audit DB: {}",
                bifrost_admin::admin_audit::audit_db_path()?.display()
            );
        }
    }

    Ok(())
}
