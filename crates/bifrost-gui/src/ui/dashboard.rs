use egui::{Color32, RichText};

use crate::state::{AppState, ProxyStatus};

pub struct DashboardPanel;

impl DashboardPanel {
    pub fn show(ui: &mut egui::Ui, state: &AppState) {
        ui.heading("Dashboard");
        ui.add_space(16.0);

        ui.horizontal(|ui| {
            Self::status_card(ui, state);
            ui.add_space(16.0);
            Self::metrics_card(ui, state);
        });

        ui.add_space(16.0);

        Self::config_card(ui, state);

        if let Some(ref error) = state.error_message {
            ui.add_space(16.0);
            Self::error_card(ui, error);
        }
    }

    fn status_card(ui: &mut egui::Ui, state: &AppState) {
        egui::Frame::group(ui.style())
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_min_width(250.0);
                ui.heading(RichText::new("Proxy Status").size(16.0));
                ui.add_space(12.0);

                let (status_color, status_text) = match state.proxy_status {
                    ProxyStatus::Running => (Color32::GREEN, "● Running"),
                    ProxyStatus::Stopped => (Color32::GRAY, "○ Stopped"),
                    ProxyStatus::Starting => (Color32::YELLOW, "◐ Starting..."),
                    ProxyStatus::Stopping => (Color32::YELLOW, "◐ Stopping..."),
                    ProxyStatus::Error => (Color32::RED, "✗ Error"),
                };

                ui.label(RichText::new(status_text).color(status_color).size(18.0));
                ui.add_space(8.0);

                if let Some(uptime) = state.uptime() {
                    ui.label(format!("Uptime: {}", uptime));
                }

                ui.label(format!(
                    "Listening: {}:{}",
                    state.settings.host, state.settings.port
                ));

                if let Some(socks5_port) = state.settings.socks5_port {
                    ui.label(format!("SOCKS5: {}", socks5_port));
                }
            });
    }

    fn metrics_card(ui: &mut egui::Ui, state: &AppState) {
        egui::Frame::group(ui.style())
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_min_width(250.0);
                ui.heading(RichText::new("Metrics").size(16.0));
                ui.add_space(12.0);

                egui::Grid::new("metrics_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Total Requests:");
                        ui.label(
                            RichText::new(format!("{}", state.metrics.total_requests)).strong(),
                        );
                        ui.end_row();

                        ui.label("Active Connections:");
                        ui.label(
                            RichText::new(format!("{}", state.metrics.active_connections)).strong(),
                        );
                        ui.end_row();

                        ui.label("Requests/sec:");
                        ui.label(
                            RichText::new(format!("{:.1}", state.metrics.requests_per_second))
                                .strong(),
                        );
                        ui.end_row();

                        ui.label("Bytes Sent:");
                        ui.label(RichText::new(format_bytes(state.metrics.bytes_sent)).strong());
                        ui.end_row();

                        ui.label("Bytes Received:");
                        ui.label(
                            RichText::new(format_bytes(state.metrics.bytes_received)).strong(),
                        );
                        ui.end_row();
                    });
            });
    }

    fn config_card(ui: &mut egui::Ui, state: &AppState) {
        egui::Frame::group(ui.style())
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.heading(RichText::new("Configuration").size(16.0));
                ui.add_space(12.0);

                egui::Grid::new("config_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("TLS Interception:");
                        ui.label(if state.settings.enable_tls_interception {
                            RichText::new("✓ Enabled").color(Color32::GREEN)
                        } else {
                            RichText::new("✗ Disabled").color(Color32::GRAY)
                        });
                        ui.end_row();

                        ui.label("Allow LAN:");
                        ui.label(if state.settings.allow_lan {
                            RichText::new("✓ Enabled").color(Color32::GREEN)
                        } else {
                            RichText::new("✗ Disabled").color(Color32::GRAY)
                        });
                        ui.end_row();

                        ui.label("Unsafe SSL:");
                        ui.label(if state.settings.unsafe_ssl {
                            RichText::new("⚠ Enabled").color(Color32::YELLOW)
                        } else {
                            RichText::new("✗ Disabled").color(Color32::GREEN)
                        });
                        ui.end_row();

                        ui.label("CA Certificate:");
                        match state.ca_installed {
                            Some(true) => {
                                ui.label(RichText::new("✓ Installed").color(Color32::GREEN));
                            }
                            Some(false) => {
                                ui.label(RichText::new("⚠ Not installed").color(Color32::YELLOW));
                            }
                            None => {
                                ui.label(RichText::new("Unknown").color(Color32::GRAY));
                            }
                        }
                        ui.end_row();

                        ui.label("Loaded Rules:");
                        ui.label(RichText::new(format!("{}", state.rules.len())).strong());
                        ui.end_row();
                    });
            });
    }

    fn error_card(ui: &mut egui::Ui, error: &str) {
        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(60, 20, 20))
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚠").color(Color32::RED).size(20.0));
                    ui.label(RichText::new("Error").color(Color32::RED).strong());
                });
                ui.add_space(8.0);
                ui.label(RichText::new(error).color(Color32::WHITE));
            });
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
