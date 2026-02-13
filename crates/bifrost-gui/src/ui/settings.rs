use egui::{Color32, RichText, ScrollArea};

use crate::proxy_controller::ProxyController;
use crate::state::AppState;

pub struct SettingsPanel;

impl SettingsPanel {
    pub fn show(ui: &mut egui::Ui, state: &mut AppState, controller: &mut ProxyController) {
        ui.heading("Settings");
        ui.add_space(16.0);

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                Self::system_proxy_section(ui, state, controller);
                ui.add_space(16.0);
                Self::proxy_settings(ui, state);
                ui.add_space(16.0);
                Self::tls_settings(ui, state);
                ui.add_space(16.0);
                Self::certificate_section(ui, state);
                ui.add_space(16.0);
                Self::usage_guide(ui);
            });
    }

    fn system_proxy_section(
        ui: &mut egui::Ui,
        state: &mut AppState,
        controller: &mut ProxyController,
    ) {
        egui::CollapsingHeader::new(RichText::new("🖥️ System Proxy").strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.add_space(8.0);

                let is_supported = controller.is_system_proxy_supported();

                if !is_supported {
                    ui.label(
                        RichText::new("⚠ System proxy is not supported on this platform")
                            .color(Color32::YELLOW),
                    );
                    return;
                }

                egui::Frame::group(ui.style())
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        let is_enabled = state.system_proxy_enabled;

                        ui.horizontal(|ui| {
                            let toggle_text = if is_enabled {
                                "✓ System Proxy Enabled"
                            } else {
                                "○ System Proxy Disabled"
                            };

                            let button_color = if is_enabled {
                                Color32::GREEN
                            } else {
                                Color32::GRAY
                            };

                            if ui
                                .button(RichText::new(toggle_text).color(button_color))
                                .clicked()
                            {
                                let result = if is_enabled {
                                    controller.disable_system_proxy()
                                } else {
                                    controller.enable_system_proxy()
                                };
                                if let Err(e) = result {
                                    state.error_message =
                                        Some(format!("Failed to toggle system proxy: {}", e));
                                }
                            }
                        });

                        ui.add_space(8.0);

                        if is_enabled {
                            ui.label(
                                RichText::new(format!(
                                    "HTTP/HTTPS proxy: {}:{}",
                                    state.settings.host, state.settings.port
                                ))
                                .small(),
                            );

                            if let Some(socks5_port) = state.settings.socks5_port {
                                ui.label(
                                    RichText::new(format!(
                                        "SOCKS5 proxy: {}:{}",
                                        state.settings.host, socks5_port
                                    ))
                                    .small(),
                                );
                            }
                        }

                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(
                                "Note: Enabling system proxy will redirect all system HTTP/HTTPS \
                                 traffic through Bifrost. Remember to disable it when done.",
                            )
                            .small()
                            .weak(),
                        );
                    });
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

    fn usage_guide(ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(RichText::new("📖 Usage Guide").strong())
            .default_open(false)
            .show(ui, |ui| {
                ui.add_space(8.0);

                ui.label(RichText::new("Rule Syntax").strong());
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Rules use pattern matching to intercept and modify requests:\n\
                         • pattern operator target\n\
                         • Example: *.example.com mock://response.json",
                    )
                    .small()
                    .monospace(),
                );

                ui.add_space(12.0);

                ui.label(RichText::new("Common Operators").strong());
                ui.add_space(4.0);

                egui::Grid::new("operators_grid")
                    .num_columns(2)
                    .spacing([20.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("mock://").monospace().small());
                        ui.label(RichText::new("Return mock response").small());
                        ui.end_row();

                        ui.label(RichText::new("redirect://").monospace().small());
                        ui.label(RichText::new("Redirect to another URL").small());
                        ui.end_row();

                        ui.label(RichText::new("delay://").monospace().small());
                        ui.label(RichText::new("Add delay before response").small());
                        ui.end_row();

                        ui.label(RichText::new("status://").monospace().small());
                        ui.label(RichText::new("Return specific status code").small());
                        ui.end_row();

                        ui.label(RichText::new("header://").monospace().small());
                        ui.label(RichText::new("Modify request/response headers").small());
                        ui.end_row();

                        ui.label(RichText::new("replace://").monospace().small());
                        ui.label(RichText::new("Replace response body content").small());
                        ui.end_row();
                    });

                ui.add_space(12.0);

                ui.label(RichText::new("Supported Protocols").strong());
                ui.add_space(4.0);

                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("HTTP")
                            .small()
                            .background_color(Color32::from_rgb(40, 60, 80)),
                    );
                    ui.label(
                        RichText::new("HTTPS")
                            .small()
                            .background_color(Color32::from_rgb(40, 70, 50)),
                    );
                    ui.label(
                        RichText::new("WebSocket")
                            .small()
                            .background_color(Color32::from_rgb(70, 60, 40)),
                    );
                    ui.label(
                        RichText::new("WSS")
                            .small()
                            .background_color(Color32::from_rgb(60, 40, 70)),
                    );
                    ui.label(
                        RichText::new("SOCKS5")
                            .small()
                            .background_color(Color32::from_rgb(50, 50, 50)),
                    );
                    ui.label(
                        RichText::new("Tunnel")
                            .small()
                            .background_color(Color32::from_rgb(50, 50, 50)),
                    );
                });

                ui.add_space(12.0);

                ui.label(RichText::new("Quick Start").strong());
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "1. Start the proxy (default port: 9900)\n\
                         2. Configure your browser/system to use the proxy\n\
                         3. Install the CA certificate for HTTPS interception\n\
                         4. Add rules to intercept and modify traffic\n\
                         5. Monitor traffic in the Traffic panel",
                    )
                    .small(),
                );
            });
    }
}
