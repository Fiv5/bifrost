use egui::{Align, Color32, Layout, RichText, Sense, Vec2};

use super::Panel;
use crate::state::ProxyStatus;

pub struct Sidebar;

impl Sidebar {
    pub fn show(
        ctx: &egui::Context,
        current_panel: &mut Panel,
        proxy_status: ProxyStatus,
        on_start: impl FnOnce(),
        on_stop: impl FnOnce(),
    ) {
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(200.0)
            .show(ctx, |ui| {
                ui.add_space(16.0);

                ui.vertical_centered(|ui| {
                    ui.heading(RichText::new("Bifrost").size(24.0).strong());
                    ui.add_space(4.0);
                    ui.label(RichText::new("HTTP Proxy").size(12.0).weak());
                });

                ui.add_space(24.0);
                ui.separator();
                ui.add_space(16.0);

                Self::status_section(ui, proxy_status, on_start, on_stop);

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(16.0);

                Self::navigation_section(ui, current_panel);

                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    ui.add_space(16.0);
                    ui.label(RichText::new("v0.1.0").size(10.0).weak());
                    ui.add_space(8.0);
                });
            });
    }

    fn status_section(
        ui: &mut egui::Ui,
        status: ProxyStatus,
        on_start: impl FnOnce(),
        on_stop: impl FnOnce(),
    ) {
        ui.horizontal(|ui| {
            let (color, label) = match status {
                ProxyStatus::Stopped => (Color32::GRAY, "Stopped"),
                ProxyStatus::Starting => (Color32::YELLOW, "Starting..."),
                ProxyStatus::Running => (Color32::GREEN, "Running"),
                ProxyStatus::Stopping => (Color32::YELLOW, "Stopping..."),
                ProxyStatus::Error => (Color32::RED, "Error"),
            };

            let circle_size = 8.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(circle_size), Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), circle_size / 2.0, color);

            ui.label(RichText::new(label).strong());
        });

        ui.add_space(12.0);

        ui.horizontal(|ui| {
            let is_running = matches!(status, ProxyStatus::Running);
            let is_transitioning = matches!(status, ProxyStatus::Starting | ProxyStatus::Stopping);

            ui.add_enabled_ui(!is_transitioning, |ui| {
                if is_running {
                    if ui.button(RichText::new("⏹ Stop").size(14.0)).clicked() {
                        on_stop();
                    }
                } else if ui.button(RichText::new("▶ Start").size(14.0)).clicked() {
                    on_start();
                }
            });
        });
    }

    fn navigation_section(ui: &mut egui::Ui, current_panel: &mut Panel) {
        let panels = [
            Panel::Dashboard,
            Panel::Traffic,
            Panel::Rules,
            Panel::Whitelist,
            Panel::Settings,
        ];

        for panel in panels {
            let is_selected = *current_panel == panel;
            let text = format!("{} {}", panel.icon(), panel.label());

            let response = ui.selectable_label(is_selected, RichText::new(text).size(14.0));

            if response.clicked() {
                *current_panel = panel;
            }

            ui.add_space(4.0);
        }
    }
}
