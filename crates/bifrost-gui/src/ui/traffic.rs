use egui::{Color32, RichText};
use egui_extras::{Column, TableBuilder};

use crate::state::AppState;

pub struct TrafficPanel;

impl TrafficPanel {
    pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
        ui.horizontal(|ui| {
            ui.heading("Traffic");
            ui.add_space(16.0);

            if ui.button("🗑 Clear").clicked() {
                state.clear_traffic();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{} requests", state.traffic.len()));
            });
        });

        ui.add_space(16.0);

        let available_height = ui.available_height();

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto().at_least(60.0))
            .column(Column::auto().at_least(60.0))
            .column(Column::remainder().at_least(200.0))
            .column(Column::auto().at_least(60.0))
            .column(Column::auto().at_least(80.0))
            .column(Column::auto().at_least(80.0))
            .auto_shrink([false, false])
            .max_scroll_height(available_height)
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Time");
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
                let entries: Vec<_> = state.traffic.iter().rev().collect();
                body.rows(20.0, entries.len(), |mut row| {
                    let entry = entries[row.index()];

                    row.col(|ui| {
                        ui.label(entry.timestamp.format("%H:%M:%S").to_string());
                    });

                    row.col(|ui| {
                        let color = method_color(&entry.method);
                        ui.label(RichText::new(&entry.method).color(color));
                    });

                    row.col(|ui| {
                        ui.label(&entry.url);
                    });

                    row.col(|ui| {
                        if let Some(status) = entry.status {
                            let color = status_color(status);
                            ui.label(RichText::new(status.to_string()).color(color));
                        } else {
                            ui.label("-");
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
                        if let Some(size) = entry.size {
                            ui.label(format_size(size));
                        } else {
                            ui.label("-");
                        }
                    });
                });
            });
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
        _ => Color32::WHITE,
    }
}

fn status_color(status: u16) -> Color32 {
    match status {
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
        _ => Color32::from_rgb(224, 108, 117),
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
