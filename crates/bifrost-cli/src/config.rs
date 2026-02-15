use std::path::PathBuf;

pub fn get_bifrost_dir() -> bifrost_core::Result<PathBuf> {
    Ok(bifrost_storage::data_dir())
}

#[deprecated(note = "Use ConfigManager::new() instead, which handles directory initialization")]
#[allow(dead_code)]
pub fn init_config_dir() -> bifrost_core::Result<()> {
    use bifrost_storage::BifrostConfig;
    let bifrost_dir = get_bifrost_dir()?;

    let config_path = bifrost_dir.join("config.toml");
    if !config_path.exists() {
        println!("Initializing configuration directory: {:?}", bifrost_dir);

        std::fs::create_dir_all(&bifrost_dir)?;

        let subdirs = ["rules", "values", "plugins", "certs"];
        for subdir in &subdirs {
            let path = bifrost_dir.join(subdir);
            std::fs::create_dir_all(&path)?;
        }

        let default_config = BifrostConfig::default();
        let config_content = toml::to_string_pretty(&default_config).map_err(|e| {
            bifrost_core::BifrostError::Config(format!("Failed to serialize config: {}", e))
        })?;
        std::fs::write(&config_path, &config_content)?;

        println!("  Created config file: {:?}", config_path);
        println!("  Created subdirectories: {:?}", subdirs);
        println!("Configuration initialized successfully.");
    }

    Ok(())
}

#[deprecated(note = "Use ConfigManager::config() instead")]
#[allow(dead_code)]
pub fn load_config() -> bifrost_storage::BifrostConfig {
    use bifrost_storage::BifrostConfig;
    let config_path = get_bifrost_dir()
        .map(|p| p.join("config.toml"))
        .unwrap_or_default();
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str(&content) {
                return config;
            }
        }
    }
    BifrostConfig::default()
}

#[deprecated(note = "Use ConfigManager::update_config() instead")]
#[allow(dead_code)]
pub fn save_config(config: &bifrost_storage::BifrostConfig) -> bifrost_core::Result<()> {
    let config_dir = get_bifrost_dir()?;
    std::fs::create_dir_all(&config_dir)?;
    let config_path = config_dir.join("config.toml");
    let content = toml::to_string_pretty(config).map_err(|e| {
        bifrost_core::BifrostError::Config(format!("Failed to serialize config: {}", e))
    })?;
    std::fs::write(&config_path, content)?;
    Ok(())
}
