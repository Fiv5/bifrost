use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::error::{Result, BifrostError};

pub fn init_logging(level: &str) -> Result<()> {
    let filter = EnvFilter::try_new(level)
        .map_err(|e| BifrostError::Config(format!("Invalid log level '{}': {}", level, e)))?;

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .try_init()
        .map_err(|e| BifrostError::Config(format!("Failed to initialize logging: {}", e)))?;

    Ok(())
}
