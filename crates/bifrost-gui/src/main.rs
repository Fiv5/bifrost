#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod proxy_controller;
mod state;
mod ui;

use app::BifrostApp;
use bifrost_core::init_logging;
use bifrost_tls::init_crypto_provider;

fn main() -> eframe::Result<()> {
    init_crypto_provider();

    if let Err(e) = init_logging("info") {
        eprintln!("Failed to initialize logging: {}", e);
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Bifrost Proxy"),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Bifrost Proxy",
        native_options,
        Box::new(|cc| Ok(Box::new(BifrostApp::new(cc)))),
    )
}
