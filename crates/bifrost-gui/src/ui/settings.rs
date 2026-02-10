use egui::{Color32, RichText};

use crate::state::AppState;

pub struct SettingsPanel;

impl SettingsPanel {
    pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
        ui.heading("Settings");
        ui.add_space(16.0);

        let available_height = ui.available_height();
        egui::ScrollArea::vertical()
            .max_height(available_height)
            .show(ui, |ui| {
                Self::proxy_settings(ui, state);
                ui.add_space(16.0);
                Self::tls_settings(ui, state);
                ui.add_space(16.0);
                Self::certificate_section(ui, state);
            });
    }

    fn proxy_settings(ui: &mut egui::Ui, state: &mut AppState) {
        egui::CollapsingHeader::new(RichText::new("🌐 Proxy Settings").strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.add_space(8.0);

                egui::Grid::new("proxy_settings_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Host:");
                        ui.add(
                            egui::TextEdit::singleline(&mut state.settings.host)
                                .desired_width(200.0),
                        );
                        ui.end_row();

                        ui.label("Port:");
                        ui.add(egui::DragValue::new(&mut state.settings.port).range(1..=65535));
                        ui.end_row();

                        ui.label("SOCKS5 Port:");
                        let mut has_socks5 = state.settings.socks5_port.is_some();
                        let mut socks5_port = state.settings.socks5_port.unwrap_or(1080);

                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut has_socks5, "").changed() {
                                state.settings.socks5_port =
                                    if has_socks5 { Some(socks5_port) } else { None };
                            }

                            ui.add_enabled(
                                has_socks5,
                                egui::DragValue::new(&mut socks5_port).range(1..=65535),
                            );

                            if has_socks5 {
                                state.settings.socks5_port = Some(socks5_port);
                            }
                        });
                        ui.end_row();

                        ui.label("Allow LAN:");
                        ui.checkbox(&mut state.settings.allow_lan, "Allow connections from LAN");
                        ui.end_row();
                    });
            });
    }

    fn tls_settings(ui: &mut egui::Ui, state: &mut AppState) {
        egui::CollapsingHeader::new(RichText::new("🔒 TLS Settings").strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.add_space(8.0);

                egui::Grid::new("tls_settings_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("TLS Interception:");
                        ui.checkbox(
                            &mut state.settings.enable_tls_interception,
                            "Enable HTTPS interception",
                        );
                        ui.end_row();

                        ui.label("Unsafe SSL:");
                        ui.horizontal(|ui| {
                            ui.checkbox(
                                &mut state.settings.unsafe_ssl,
                                "Skip certificate verification",
                            );
                            if state.settings.unsafe_ssl {
                                ui.label(
                                    RichText::new("⚠ Not recommended")
                                        .color(Color32::YELLOW)
                                        .small(),
                                );
                            }
                        });
                        ui.end_row();
                    });

                ui.add_space(8.0);

                ui.label("Intercept Exclude Patterns:");
                ui.add_space(4.0);

                let mut exclude_text = state.settings.intercept_exclude.join("\n");
                let response = ui.add(
                    egui::TextEdit::multiline(&mut exclude_text)
                        .desired_width(400.0)
                        .desired_rows(4)
                        .hint_text("One pattern per line, e.g.:\n*.apple.com\n*.microsoft.com"),
                );

                if response.changed() {
                    state.settings.intercept_exclude = exclude_text
                        .lines()
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| l.trim().to_string())
                        .collect();
                }
            });
    }

    fn certificate_section(ui: &mut egui::Ui, state: &AppState) {
        egui::CollapsingHeader::new(RichText::new("📜 Certificate").strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("CA Status:");
                    match state.ca_installed {
                        Some(true) => {
                            ui.label(
                                RichText::new("✓ Installed and trusted").color(Color32::GREEN),
                            );
                        }
                        Some(false) => {
                            ui.label(RichText::new("⚠ Not installed").color(Color32::YELLOW));
                        }
                        None => {
                            ui.label(RichText::new("Unknown").color(Color32::GRAY));
                        }
                    }
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("📂 Open Cert Directory").clicked() {
                        if let Ok(data_dir) = std::env::var("HOME")
                            .map(|h| std::path::PathBuf::from(h).join(".bifrost").join("certs"))
                        {
                            let _ = open::that(&data_dir);
                        }
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    if ui.button("📥 Export CA Certificate").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("bifrost-ca.crt")
                            .add_filter("Certificate", &["crt", "pem"])
                            .save_file()
                        {
                            let cert_dir = bifrost_storage::data_dir().join("certs");
                            let ca_cert = cert_dir.join("ca.crt");
                            if ca_cert.exists() {
                                let _ = std::fs::copy(&ca_cert, &path);
                            }
                        }
                    }
                });

                ui.add_space(8.0);

                ui.label(
                    RichText::new(
                        "Note: To intercept HTTPS traffic, the CA certificate must be installed \
                         and trusted in your system's certificate store.",
                    )
                    .small()
                    .weak(),
                );
            });
    }
}
