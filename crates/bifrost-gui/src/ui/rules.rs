use egui::{Color32, RichText, TextEdit};

use crate::proxy_controller::ProxyController;
use crate::state::{AppState, RuleEntry};

pub struct RulesPanel {
    selected_rule: Option<String>,
    editing_content: String,
    new_rule_name: String,
    show_new_rule_dialog: bool,
}

impl Default for RulesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl RulesPanel {
    pub fn new() -> Self {
        Self {
            selected_rule: None,
            editing_content: String::new(),
            new_rule_name: String::new(),
            show_new_rule_dialog: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, state: &mut AppState, controller: &ProxyController) {
        ui.horizontal(|ui| {
            ui.heading("Rules");
            ui.add_space(16.0);

            if ui.button("➕ New Rule").clicked() {
                self.show_new_rule_dialog = true;
                self.new_rule_name.clear();
            }

            if ui.button("🔄 Reload").clicked() {
                state.rules = controller.load_rules();
            }
        });

        ui.add_space(16.0);

        if self.show_new_rule_dialog {
            self.new_rule_dialog(ui, state, controller);
        }

        let available_size = ui.available_size();
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(250.0, available_size.y),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.rules_list(ui, state, controller);
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                egui::vec2(available_size.x - 260.0, available_size.y),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.rule_editor(ui, state, controller);
                },
            );
        });
    }

    fn new_rule_dialog(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut AppState,
        controller: &ProxyController,
    ) {
        egui::Window::new("New Rule")
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.new_rule_name);
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() && !self.new_rule_name.is_empty() {
                        let rule = RuleEntry {
                            name: self.new_rule_name.clone(),
                            enabled: true,
                            content: String::new(),
                        };
                        if controller.save_rule(&rule).is_ok() {
                            state.rules = controller.load_rules();
                            self.selected_rule = Some(self.new_rule_name.clone());
                            self.editing_content.clear();
                        }
                        self.show_new_rule_dialog = false;
                    }

                    if ui.button("Cancel").clicked() {
                        self.show_new_rule_dialog = false;
                    }
                });
            });
    }

    fn rules_list(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut AppState,
        controller: &ProxyController,
    ) {
        let available_height = ui.available_height();
        egui::ScrollArea::vertical()
            .id_salt("rules_list")
            .max_height(available_height)
            .show(ui, |ui| {
                ui.set_min_width(230.0);

                if state.rules.is_empty() {
                    ui.label(RichText::new("No rules configured").weak());
                    return;
                }

                let rules_snapshot: Vec<_> = state.rules.to_vec();

                for rule in &rules_snapshot {
                    let is_selected = self.selected_rule.as_ref() == Some(&rule.name);

                    ui.horizontal(|ui| {
                        let mut enabled = rule.enabled;
                        if ui.checkbox(&mut enabled, "").changed()
                            && controller.toggle_rule(&rule.name, enabled).is_ok()
                        {
                            if let Some(r) = state.rules.iter_mut().find(|r| r.name == rule.name) {
                                r.enabled = enabled;
                            }
                        }

                        let color = if rule.enabled {
                            Color32::WHITE
                        } else {
                            Color32::GRAY
                        };

                        let response = ui
                            .selectable_label(is_selected, RichText::new(&rule.name).color(color));

                        if response.clicked() {
                            self.selected_rule = Some(rule.name.clone());
                            self.editing_content = rule.content.clone();
                        }
                    });
                }
            });
    }

    fn rule_editor(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut AppState,
        controller: &ProxyController,
    ) {
        if let Some(ref name) = self.selected_rule.clone() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(name).strong().size(16.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(RichText::new("🗑 Delete").color(Color32::from_rgb(224, 108, 117)))
                        .clicked()
                        && controller.delete_rule(name).is_ok()
                    {
                        state.rules = controller.load_rules();
                        self.selected_rule = None;
                        self.editing_content.clear();
                    }

                    if ui.button("💾 Save").clicked() {
                        if let Some(rule) = state.rules.iter().find(|r| &r.name == name) {
                            let updated = RuleEntry {
                                name: rule.name.clone(),
                                enabled: rule.enabled,
                                content: self.editing_content.clone(),
                            };
                            if controller.save_rule(&updated).is_ok() {
                                state.rules = controller.load_rules();
                            }
                        }
                    }
                });
            });

            ui.add_space(8.0);

            let available_size = ui.available_size();
            egui::ScrollArea::vertical()
                .id_salt("rule_editor")
                .max_height(available_size.y)
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut self.editing_content)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(available_size.x - 16.0)
                            .desired_rows(30)
                            .code_editor(),
                    );
                });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Select a rule to edit").weak());
            });
        }
    }
}
