use std::sync::Arc;

use parking_lot::Mutex;

use crate::proxy_controller::ProxyController;
use crate::state::AppState;
use crate::ui::{
    DashboardPanel, Panel, RulesPanel, SettingsPanel, Sidebar, TrafficPanel, WhitelistPanel,
};

pub struct BifrostApp {
    state: Arc<Mutex<AppState>>,
    controller: ProxyController,
    current_panel: Panel,
    rules_panel: RulesPanel,
    whitelist_panel: WhitelistPanel,
}

impl BifrostApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let state = Arc::new(Mutex::new(AppState::new()));
        let controller = ProxyController::new(Arc::clone(&state));

        {
            let mut s = state.lock();
            s.rules = controller.load_rules();
            s.ca_installed = controller.check_ca_status();
        }

        Self {
            state,
            controller,
            current_panel: Panel::Dashboard,
            rules_panel: RulesPanel::new(),
            whitelist_panel: WhitelistPanel::new(),
        }
    }
}

impl eframe::App for BifrostApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let status = self.state.lock().proxy_status;

        let mut should_start = false;
        let mut should_stop = false;

        Sidebar::show(
            ctx,
            &mut self.current_panel,
            status,
            || should_start = true,
            || should_stop = true,
        );

        if should_start {
            self.controller.start();
        }
        if should_stop {
            self.controller.stop();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    let mut state = self.state.lock();
                    match self.current_panel {
                        Panel::Dashboard => {
                            DashboardPanel::show(ui, &state);
                        }
                        Panel::Traffic => {
                            TrafficPanel::show(ui, &mut state);
                        }
                        Panel::Rules => {
                            self.rules_panel.show(ui, &mut state, &self.controller);
                        }
                        Panel::Whitelist => {
                            self.whitelist_panel.show(ui, &mut state);
                        }
                        Panel::Settings => {
                            SettingsPanel::show(ui, &mut state);
                        }
                    }
                });
            });
        });

        if self.controller.is_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.controller.stop();
    }
}
