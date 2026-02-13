use egui::{Color32, RichText, ScrollArea, TextEdit, Ui};
use egui_extras::{Column, TableBuilder};

use crate::state::{
    AppState, FilterProtocol, FilterStatus, TrafficDetailTab, TrafficEntry, TrafficStatus,
};

pub struct TrafficPanel;

impl TrafficPanel {
    pub fn show(ui: &mut Ui, state: &mut AppState) {
        Self::show_toolbar(ui, state);
        ui.add_space(8.0);

        let available_height = ui.available_height();

        if state.traffic_view.show_detail_panel {
            let table_height = available_height * (1.0 - state.traffic_view.detail_panel_ratio);
            let detail_height = available_height * state.traffic_view.detail_panel_ratio;

            ui.vertical(|ui| {
                ui.set_height(table_height - 4.0);
                Self::show_traffic_table(ui, state);
            });

            ui.separator();

            ui.vertical(|ui| {
                ui.set_height(detail_height - 4.0);
                Self::show_detail_panel(ui, state);
            });
        } else {
            Self::show_traffic_table(ui, state);
        }
    }

    fn show_toolbar(ui: &mut Ui, state: &mut AppState) {
        ui.horizontal(|ui| {
            ui.heading("Traffic");
            ui.add_space(16.0);

            let pause_text = if state.traffic_view.paused {
                "▶ Resume"
            } else {
                "⏸ Pause"
            };
            if ui.button(pause_text).clicked() {
                state.traffic_view.paused = !state.traffic_view.paused;
            }

            if ui.button("🗑 Clear").clicked() {
                state.clear_traffic();
            }

            ui.separator();

            let detail_text = if state.traffic_view.show_detail_panel {
                "◀ Hide Detail"
            } else {
                "▶ Show Detail"
            };
            if ui.button(detail_text).clicked() {
                state.traffic_view.show_detail_panel = !state.traffic_view.show_detail_panel;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let filtered_count = state.filtered_traffic().count();
                let total_count = state.traffic.len();
                if filtered_count == total_count {
                    ui.label(format!("{} requests", total_count));
                } else {
                    ui.label(format!("{} / {} requests", filtered_count, total_count));
                }
            });
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("🔍");
            let search_response = ui.add(
                TextEdit::singleline(&mut state.traffic_view.filter.search_text)
                    .hint_text("Search URL, Method, Host...")
                    .desired_width(200.0),
            );
            if search_response.changed() {
                state.traffic_view.selected_id = None;
            }

            ui.separator();

            egui::ComboBox::from_id_salt("protocol_filter")
                .selected_text(protocol_display(&state.traffic_view.filter.protocol))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut state.traffic_view.filter.protocol,
                        FilterProtocol::All,
                        "All Protocols",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.protocol,
                        FilterProtocol::Http,
                        "HTTP",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.protocol,
                        FilterProtocol::Https,
                        "HTTPS",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.protocol,
                        FilterProtocol::Ws,
                        "WebSocket",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.protocol,
                        FilterProtocol::Wss,
                        "WebSocket Secure",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.protocol,
                        FilterProtocol::Tunnel,
                        "Tunnel",
                    );
                });

            egui::ComboBox::from_id_salt("status_filter")
                .selected_text(status_filter_display(&state.traffic_view.filter.status))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut state.traffic_view.filter.status,
                        FilterStatus::All,
                        "All Status",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.status,
                        FilterStatus::Pending,
                        "Pending",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.status,
                        FilterStatus::Status2xx,
                        "2xx Success",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.status,
                        FilterStatus::Status3xx,
                        "3xx Redirect",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.status,
                        FilterStatus::Status4xx,
                        "4xx Client Error",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.status,
                        FilterStatus::Status5xx,
                        "5xx Server Error",
                    );
                    ui.selectable_value(
                        &mut state.traffic_view.filter.status,
                        FilterStatus::Error,
                        "Error",
                    );
                });

            if has_active_filter(&state.traffic_view.filter)
                && ui.button("✕ Clear Filters").clicked()
            {
                state.traffic_view.filter = Default::default();
            }
        });
    }

    fn show_traffic_table(ui: &mut Ui, state: &mut AppState) {
        let available_height = ui.available_height();

        #[derive(Clone)]
        struct DisplayEntry {
            id: u64,
            timestamp: String,
            protocol: crate::state::TrafficProtocol,
            method: String,
            url: String,
            status: crate::state::TrafficStatus,
            status_code: Option<u16>,
            duration_ms: Option<u64>,
            response_size: Option<u64>,
        }

        let entries: Vec<DisplayEntry> = state
            .filtered_traffic()
            .map(|e| DisplayEntry {
                id: e.id,
                timestamp: e.timestamp.format("%H:%M:%S").to_string(),
                protocol: e.protocol,
                method: e.method().to_string(),
                url: e.full_url(),
                status: e.status,
                status_code: e.status_code(),
                duration_ms: e.duration_ms,
                response_size: e.response_size(),
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let selected_id = state.traffic_view.selected_id;
        let mut clicked_id: Option<u64> = None;

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .sense(egui::Sense::click())
            .column(Column::auto().at_least(70.0))
            .column(Column::auto().at_least(50.0))
            .column(Column::auto().at_least(60.0))
            .column(Column::remainder().at_least(200.0))
            .column(Column::auto().at_least(60.0))
            .column(Column::auto().at_least(80.0))
            .column(Column::auto().at_least(80.0))
            .auto_shrink([false, false])
            .max_scroll_height(available_height)
            .header(24.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Time");
                });
                header.col(|ui| {
                    ui.strong("Protocol");
                });
                header.col(|ui| {
                    ui.strong("Method");
                });
                header.col(|ui| {
                    ui.strong("URL");
                });
                header.col(|ui| {
                    ui.strong("Status");
                });
                header.col(|ui| {
                    ui.strong("Duration");
                });
                header.col(|ui| {
                    ui.strong("Size");
                });
            })
            .body(|body| {
                body.rows(22.0, entries.len(), |mut row| {
                    let entry = &entries[row.index()];
                    let is_selected = selected_id == Some(entry.id);

                    if is_selected {
                        row.set_selected(true);
                    }

                    row.col(|ui| {
                        ui.label(&entry.timestamp);
                    });

                    row.col(|ui| {
                        let color = protocol_color(&entry.protocol);
                        ui.label(
                            RichText::new(entry.protocol.to_string())
                                .color(color)
                                .small(),
                        );
                    });

                    row.col(|ui| {
                        let color = method_color(&entry.method);
                        ui.label(RichText::new(&entry.method).color(color));
                    });

                    row.col(|ui| {
                        let truncated = if entry.url.len() > 80 {
                            format!("{}...", &entry.url[..77])
                        } else {
                            entry.url.clone()
                        };
                        ui.label(&truncated).on_hover_text(&entry.url);
                    });

                    row.col(|ui| match entry.status {
                        TrafficStatus::Pending => {
                            ui.label(RichText::new("⏳").color(Color32::YELLOW));
                        }
                        TrafficStatus::Error => {
                            ui.label(RichText::new("ERR").color(Color32::RED));
                        }
                        TrafficStatus::Aborted => {
                            ui.label(RichText::new("ABT").color(Color32::GRAY));
                        }
                        TrafficStatus::Complete => {
                            if let Some(status) = entry.status_code {
                                let color = status_color(status);
                                ui.label(RichText::new(status.to_string()).color(color));
                            } else {
                                ui.label("-");
                            }
                        }
                    });

                    row.col(|ui| {
                        if let Some(duration) = entry.duration_ms {
                            let color = duration_color(duration);
                            ui.label(RichText::new(format!("{}ms", duration)).color(color));
                        } else {
                            ui.label("-");
                        }
                    });

                    row.col(|ui| {
                        if let Some(size) = entry.response_size {
                            ui.label(format_size(size));
                        } else {
                            ui.label("-");
                        }
                    });

                    if row.response().clicked() {
                        clicked_id = Some(entry.id);
                    }
                });
            });

        if let Some(id) = clicked_id {
            state.traffic_view.selected_id = Some(id);
            state.traffic_view.show_detail_panel = true;
        }
    }

    fn show_detail_panel(ui: &mut Ui, state: &mut AppState) {
        if let Some(entry) = state.get_selected_traffic().cloned() {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut state.traffic_view.detail_tab,
                    TrafficDetailTab::Overview,
                    "Overview",
                );
                ui.selectable_value(
                    &mut state.traffic_view.detail_tab,
                    TrafficDetailTab::Headers,
                    "Headers",
                );
                ui.selectable_value(
                    &mut state.traffic_view.detail_tab,
                    TrafficDetailTab::Body,
                    "Body",
                );
                ui.selectable_value(
                    &mut state.traffic_view.detail_tab,
                    TrafficDetailTab::Raw,
                    "Raw",
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✕").clicked() {
                        state.traffic_view.selected_id = None;
                    }
                });
            });

            ui.separator();

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match state.traffic_view.detail_tab {
                    TrafficDetailTab::Overview => Self::show_overview_tab(ui, &entry),
                    TrafficDetailTab::Headers => Self::show_headers_tab(ui, &entry),
                    TrafficDetailTab::Body => Self::show_body_tab(ui, &entry),
                    TrafficDetailTab::Raw => Self::show_raw_tab(ui, &entry),
                });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a request to view details");
            });
        }
    }

    fn show_overview_tab(ui: &mut Ui, entry: &TrafficEntry) {
        ui.heading("Request");
        ui.add_space(8.0);

        egui::Grid::new("request_overview")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                ui.label("URL:");
                ui.label(entry.full_url());
                ui.end_row();

                ui.label("Method:");
                ui.label(entry.method());
                ui.end_row();

                ui.label("Protocol:");
                ui.label(entry.protocol.to_string());
                ui.end_row();

                ui.label("HTTP Version:");
                ui.label(&entry.request.http_version);
                ui.end_row();

                ui.label("Client IP:");
                ui.label(&entry.client_ip);
                ui.end_row();

                ui.label("Time:");
                ui.label(entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string());
                ui.end_row();
            });

        ui.add_space(16.0);
        ui.heading("Response");
        ui.add_space(8.0);

        if let Some(ref response) = entry.response {
            egui::Grid::new("response_overview")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Status:");
                    let status_text =
                        format!("{} {}", response.status_code, response.status_message);
                    let color = status_color(response.status_code);
                    ui.label(RichText::new(status_text).color(color));
                    ui.end_row();

                    ui.label("HTTP Version:");
                    ui.label(&response.http_version);
                    ui.end_row();

                    ui.label("Size:");
                    ui.label(format_size(response.body_size));
                    ui.end_row();

                    if let Some(duration) = entry.duration_ms {
                        ui.label("Duration:");
                        ui.label(format!("{}ms", duration));
                        ui.end_row();
                    }

                    if let Some(content_type) = entry.content_type() {
                        ui.label("Content-Type:");
                        ui.label(content_type);
                        ui.end_row();
                    }
                });
        } else {
            match entry.status {
                TrafficStatus::Pending => {
                    ui.label(RichText::new("⏳ Request pending...").color(Color32::YELLOW));
                }
                TrafficStatus::Error => {
                    ui.label(RichText::new("❌ Request failed").color(Color32::RED));
                }
                TrafficStatus::Aborted => {
                    ui.label(RichText::new("⚠ Request aborted").color(Color32::GRAY));
                }
                TrafficStatus::Complete => {
                    ui.label("No response data available");
                }
            }
        }

        if !entry.matched_rules.is_empty() {
            ui.add_space(16.0);
            ui.heading("Matched Rules");
            ui.add_space(8.0);
            for rule in &entry.matched_rules {
                ui.label(format!("• {}", rule));
            }
        }
    }

    fn show_headers_tab(ui: &mut Ui, entry: &TrafficEntry) {
        ui.collapsing("Request Headers", |ui| {
            Self::show_headers_table(ui, "req_headers", &entry.request.headers.entries);
        });

        ui.add_space(8.0);

        if let Some(ref response) = entry.response {
            ui.collapsing("Response Headers", |ui| {
                Self::show_headers_table(ui, "res_headers", &response.headers.entries);
            });
        }
    }

    fn show_headers_table(ui: &mut Ui, id: &str, headers: &[(String, String)]) {
        if headers.is_empty() {
            ui.label("No headers");
            return;
        }

        egui::Grid::new(id)
            .num_columns(2)
            .spacing([16.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for (key, value) in headers {
                    ui.label(RichText::new(key).strong());
                    ui.label(value);
                    ui.end_row();
                }
            });
    }

    fn show_body_tab(ui: &mut Ui, entry: &TrafficEntry) {
        ui.collapsing("Request Body", |ui| {
            if let Some(ref body) = entry.request.body {
                if body.is_empty() {
                    ui.label("(empty)");
                } else {
                    Self::show_body_content(ui, "req_body", body);
                }
            } else {
                ui.label("No request body");
            }
        });

        ui.add_space(8.0);

        if let Some(ref response) = entry.response {
            ui.collapsing("Response Body", |ui| {
                if let Some(ref body) = response.body {
                    if body.is_empty() {
                        ui.label("(empty)");
                    } else {
                        Self::show_body_content(ui, "res_body", body);
                    }
                } else {
                    ui.label("No response body");
                }
            });
        }
    }

    fn show_body_content(ui: &mut Ui, id: &str, body: &str) {
        let truncated = if body.len() > 10000 {
            format!(
                "{}...\n\n[Truncated, {} bytes total]",
                &body[..10000],
                body.len()
            )
        } else {
            body.to_string()
        };

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&truncated) {
            let pretty = serde_json::to_string_pretty(&json).unwrap_or(truncated);
            egui::ScrollArea::vertical()
                .id_salt(id)
                .max_height(300.0)
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut pretty.as_str())
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                });
        } else {
            egui::ScrollArea::vertical()
                .id_salt(id)
                .max_height(300.0)
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut truncated.as_str())
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                });
        }
    }

    fn show_raw_tab(ui: &mut Ui, entry: &TrafficEntry) {
        let mut raw = String::new();

        raw.push_str(&format!(
            "{} {} {}\r\n",
            entry.request.method, entry.request.url, entry.request.http_version
        ));
        for (key, value) in &entry.request.headers.entries {
            raw.push_str(&format!("{}: {}\r\n", key, value));
        }
        raw.push_str("\r\n");
        if let Some(ref body) = entry.request.body {
            raw.push_str(body);
        }

        if let Some(ref response) = entry.response {
            raw.push_str("\r\n\r\n--- Response ---\r\n\r\n");
            raw.push_str(&format!(
                "{} {} {}\r\n",
                response.http_version, response.status_code, response.status_message
            ));
            for (key, value) in &response.headers.entries {
                raw.push_str(&format!("{}: {}\r\n", key, value));
            }
            raw.push_str("\r\n");
            if let Some(ref body) = response.body {
                let truncated = if body.len() > 5000 {
                    format!("{}...\n[Truncated]", &body[..5000])
                } else {
                    body.clone()
                };
                raw.push_str(&truncated);
            }
        }

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut raw.as_str())
                        .code_editor()
                        .desired_width(f32::INFINITY),
                );
            });
    }
}

fn protocol_display(protocol: &FilterProtocol) -> &'static str {
    match protocol {
        FilterProtocol::All => "All Protocols",
        FilterProtocol::Http => "HTTP",
        FilterProtocol::Https => "HTTPS",
        FilterProtocol::Ws => "WebSocket",
        FilterProtocol::Wss => "WSS",
        FilterProtocol::Tunnel => "Tunnel",
    }
}

fn status_filter_display(status: &FilterStatus) -> &'static str {
    match status {
        FilterStatus::All => "All Status",
        FilterStatus::Status1xx => "1xx Info",
        FilterStatus::Status2xx => "2xx Success",
        FilterStatus::Status3xx => "3xx Redirect",
        FilterStatus::Status4xx => "4xx Client Error",
        FilterStatus::Status5xx => "5xx Server Error",
        FilterStatus::Pending => "Pending",
        FilterStatus::Error => "Error",
    }
}

fn has_active_filter(filter: &crate::state::TrafficFilter) -> bool {
    !matches!(filter.protocol, FilterProtocol::All)
        || !matches!(filter.status, FilterStatus::All)
        || !filter.search_text.is_empty()
        || filter.method_filter.is_some()
}

fn protocol_color(protocol: &crate::state::TrafficProtocol) -> Color32 {
    match protocol {
        crate::state::TrafficProtocol::Http => Color32::from_rgb(97, 175, 239),
        crate::state::TrafficProtocol::Https => Color32::from_rgb(152, 195, 121),
        crate::state::TrafficProtocol::Ws => Color32::from_rgb(229, 192, 123),
        crate::state::TrafficProtocol::Wss => Color32::from_rgb(198, 120, 221),
        crate::state::TrafficProtocol::Tunnel => Color32::GRAY,
    }
}

fn method_color(method: &str) -> Color32 {
    match method {
        "GET" => Color32::from_rgb(97, 175, 239),
        "POST" => Color32::from_rgb(152, 195, 121),
        "PUT" => Color32::from_rgb(229, 192, 123),
        "DELETE" => Color32::from_rgb(224, 108, 117),
        "PATCH" => Color32::from_rgb(198, 120, 221),
        "OPTIONS" => Color32::GRAY,
        "HEAD" => Color32::GRAY,
        "CONNECT" => Color32::from_rgb(86, 182, 194),
        _ => Color32::WHITE,
    }
}

fn status_color(status: u16) -> Color32 {
    match status {
        100..=199 => Color32::from_rgb(86, 182, 194),
        200..=299 => Color32::from_rgb(152, 195, 121),
        300..=399 => Color32::from_rgb(97, 175, 239),
        400..=499 => Color32::from_rgb(229, 192, 123),
        500..=599 => Color32::from_rgb(224, 108, 117),
        _ => Color32::WHITE,
    }
}

fn duration_color(duration: u64) -> Color32 {
    match duration {
        0..=100 => Color32::from_rgb(152, 195, 121),
        101..=500 => Color32::from_rgb(229, 192, 123),
        501..=2000 => Color32::from_rgb(224, 108, 117),
        _ => Color32::from_rgb(190, 80, 70),
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
