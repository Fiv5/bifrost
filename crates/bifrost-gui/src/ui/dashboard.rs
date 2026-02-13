use egui::{Color32, RichText, ScrollArea};
use egui_plot::{Line, Plot, PlotPoints};

use crate::state::{AppState, ProxyStatus};

pub struct DashboardPanel;

impl DashboardPanel {
    pub fn show(ui: &mut egui::Ui, state: &AppState) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("Dashboard");
                ui.add_space(16.0);

                ui.horizontal_wrapped(|ui| {
                    Self::status_card(ui, state);
                    ui.add_space(16.0);
                    Self::metrics_card(ui, state);
                    ui.add_space(16.0);
                    Self::performance_card(ui, state);
                });

                ui.add_space(16.0);

                Self::charts_section(ui, state);

                ui.add_space(16.0);

                Self::protocol_stats_section(ui, state);

                ui.add_space(16.0);

                Self::config_card(ui, state);

                if let Some(ref error) = state.error_message {
                    ui.add_space(16.0);
                    Self::error_card(ui, error);
                }
            });
    }

    fn status_card(ui: &mut egui::Ui, state: &AppState) {
        egui::Frame::group(ui.style())
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_min_width(220.0);
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

                ui.add_space(8.0);

                let system_proxy_text = if state.system_proxy_enabled {
                    RichText::new("✓ System Proxy Enabled").color(Color32::GREEN)
                } else {
                    RichText::new("○ System Proxy Disabled").color(Color32::GRAY)
                };
                ui.label(system_proxy_text);
            });
    }

    fn metrics_card(ui: &mut egui::Ui, state: &AppState) {
        egui::Frame::group(ui.style())
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_min_width(220.0);
                ui.heading(RichText::new("Traffic").size(16.0));
                ui.add_space(12.0);

                egui::Grid::new("metrics_grid")
                    .num_columns(2)
                    .spacing([24.0, 8.0])
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

    fn performance_card(ui: &mut egui::Ui, state: &AppState) {
        egui::Frame::group(ui.style())
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_min_width(220.0);
                ui.heading(RichText::new("Performance").size(16.0));
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    ui.label("CPU:");
                    let cpu_color = if state.metrics.cpu_usage > 80.0 {
                        Color32::RED
                    } else if state.metrics.cpu_usage > 50.0 {
                        Color32::YELLOW
                    } else {
                        Color32::GREEN
                    };
                    ui.add(
                        egui::ProgressBar::new(state.metrics.cpu_usage as f32 / 100.0)
                            .fill(cpu_color)
                            .text(format!("{:.1}%", state.metrics.cpu_usage)),
                    );
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Memory:");
                    let mem_color = if state.metrics.memory_usage > 80.0 {
                        Color32::RED
                    } else if state.metrics.memory_usage > 50.0 {
                        Color32::YELLOW
                    } else {
                        Color32::GREEN
                    };
                    let mem_text = format!(
                        "{:.1}% ({})",
                        state.metrics.memory_usage,
                        format_bytes(state.metrics.memory_bytes)
                    );
                    ui.add(
                        egui::ProgressBar::new(state.metrics.memory_usage as f32 / 100.0)
                            .fill(mem_color)
                            .text(mem_text),
                    );
                });

                ui.add_space(12.0);

                egui::Grid::new("speed_grid")
                    .num_columns(2)
                    .spacing([24.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("↑ Upload:");
                        ui.label(
                            RichText::new(format!(
                                "{}/s",
                                format_bytes(state.metrics.upload_speed as u64)
                            ))
                            .color(Color32::from_rgb(97, 175, 239)),
                        );
                        ui.end_row();

                        ui.label("↓ Download:");
                        ui.label(
                            RichText::new(format!(
                                "{}/s",
                                format_bytes(state.metrics.download_speed as u64)
                            ))
                            .color(Color32::from_rgb(152, 195, 121)),
                        );
                        ui.end_row();
                    });
            });
    }

    fn charts_section(ui: &mut egui::Ui, state: &AppState) {
        ui.heading(RichText::new("Real-time Charts").size(16.0));
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            Self::show_chart(
                ui,
                "QPS",
                &state.metrics_history.qps,
                Color32::from_rgb(97, 175, 239),
                "req/s",
            );
            ui.add_space(16.0);
            Self::show_chart(
                ui,
                "CPU Usage",
                &state.metrics_history.cpu_usage,
                Color32::from_rgb(229, 192, 123),
                "%",
            );
            ui.add_space(16.0);
            Self::show_chart(
                ui,
                "Memory Usage",
                &state.metrics_history.memory_usage,
                Color32::from_rgb(198, 120, 221),
                "%",
            );
        });

        ui.add_space(16.0);

        ui.horizontal_wrapped(|ui| {
            Self::show_chart(
                ui,
                "Upload Speed",
                &state.metrics_history.upload_speed,
                Color32::from_rgb(97, 175, 239),
                "B/s",
            );
            ui.add_space(16.0);
            Self::show_chart(
                ui,
                "Download Speed",
                &state.metrics_history.download_speed,
                Color32::from_rgb(152, 195, 121),
                "B/s",
            );
        });
    }

    fn show_chart(
        ui: &mut egui::Ui,
        title: &str,
        data: &std::collections::VecDeque<f64>,
        color: Color32,
        unit: &str,
    ) {
        egui::Frame::group(ui.style())
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_min_width(280.0);
                ui.set_min_height(150.0);

                let current = data.back().copied().unwrap_or(0.0);
                let max_val = data.iter().copied().fold(0.0_f64, f64::max);

                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{:.1} {}", current, unit))
                                .color(color)
                                .strong(),
                        );
                    });
                });

                let points: PlotPoints = data
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| [i as f64, v])
                    .collect();

                let line = Line::new(points).color(color).fill(0.0);

                Plot::new(format!("{}_plot", title))
                    .height(100.0)
                    .show_axes([false, true])
                    .show_grid(true)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .allow_scroll(false)
                    .include_y(0.0)
                    .include_y(max_val * 1.1)
                    .show(ui, |plot_ui| {
                        plot_ui.line(line);
                    });
            });
    }

    fn protocol_stats_section(ui: &mut egui::Ui, state: &AppState) {
        ui.heading(RichText::new("Protocol Statistics").size(16.0));
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            Self::protocol_stat_card(
                ui,
                "HTTP",
                &state.metrics.http,
                Color32::from_rgb(97, 175, 239),
            );
            ui.add_space(8.0);
            Self::protocol_stat_card(
                ui,
                "HTTPS",
                &state.metrics.https,
                Color32::from_rgb(152, 195, 121),
            );
            ui.add_space(8.0);
            Self::protocol_stat_card(
                ui,
                "WebSocket",
                &state.metrics.ws,
                Color32::from_rgb(229, 192, 123),
            );
            ui.add_space(8.0);
            Self::protocol_stat_card(
                ui,
                "WSS",
                &state.metrics.wss,
                Color32::from_rgb(198, 120, 221),
            );
            ui.add_space(8.0);
            Self::protocol_stat_card(ui, "Tunnel", &state.metrics.tunnel, Color32::GRAY);
        });
    }

    fn protocol_stat_card(
        ui: &mut egui::Ui,
        name: &str,
        stats: &crate::state::ProtocolMetrics,
        color: Color32,
    ) {
        egui::Frame::group(ui.style())
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_min_width(140.0);

                ui.label(RichText::new(name).color(color).strong());
                ui.add_space(8.0);

                egui::Grid::new(format!("{}_stats", name))
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Requests:").small());
                        ui.label(
                            RichText::new(format!("{}", stats.requests))
                                .small()
                                .strong(),
                        );
                        ui.end_row();

                        ui.label(RichText::new("Connections:").small());
                        ui.label(
                            RichText::new(format!("{}", stats.connections))
                                .small()
                                .strong(),
                        );
                        ui.end_row();

                        ui.label(RichText::new("↑ Sent:").small());
                        ui.label(RichText::new(format_bytes(stats.bytes_sent)).small());
                        ui.end_row();

                        ui.label(RichText::new("↓ Recv:").small());
                        ui.label(RichText::new(format_bytes(stats.bytes_received)).small());
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
