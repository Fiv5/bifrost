#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod proxy_controller;
mod state;
mod ui;

use app::BifrostApp;
use bifrost_core::{init_logging_with_config, install_panic_hook, LogConfig, LogOutput};
use bifrost_storage::data_dir;
use bifrost_tls::init_crypto_provider;

fn load_icon() -> Option<egui::IconData> {
    let icon_bytes = include_bytes!("../../../assets/bifrost.png");
    let image = image::load_from_memory(icon_bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result<()> {
    install_panic_hook();
    init_crypto_provider();

    let log_level = std::env::var("BIFROST_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let log_output =
        std::env::var("BIFROST_LOG_OUTPUT").unwrap_or_else(|_| "console,file".to_string());
    let log_retention_days: u32 = std::env::var("BIFROST_LOG_RETENTION_DAYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);

    let log_dir = std::env::var("BIFROST_LOG_DIR")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| data_dir().join("logs"));

    let log_outputs = LogOutput::parse(&log_output);
    let log_outputs = if log_outputs.is_empty() {
        vec![LogOutput::Console, LogOutput::File]
    } else {
        log_outputs
    };

    let log_config = LogConfig::new(log_level, log_dir)
        .with_outputs(log_outputs)
        .with_retention_days(log_retention_days);

    let _log_guard = match init_logging_with_config(&log_config) {
        Ok(guard) => Some(guard),
        Err(e) => {
            eprintln!("Failed to initialize logging: {}", e);
            None
        }
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1200.0, 800.0])
        .with_min_inner_size([800.0, 600.0])
        .with_title("Bifrost Proxy");

    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Bifrost Proxy",
        native_options,
        Box::new(|cc| Ok(Box::new(BifrostApp::new(cc)))),
    )
}
