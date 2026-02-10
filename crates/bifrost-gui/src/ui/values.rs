use egui::{Color32, RichText, TextEdit};

use crate::proxy_controller::ProxyController;
use crate::state::{AppState, ValueEntry};

pub struct ValuesPanel {
    selected_value: Option<String>,
    editing_value: String,
    new_value_name: String,
    show_new_value_dialog: bool,
    search_filter: String,
}

impl Default for ValuesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ValuesPanel {
    pub fn new() -> Self {
        Self {
            selected_value: None,
            editing_value: String::new(),
            new_value_name: String::new(),
            show_new_value_dialog: false,
            search_filter: String::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, state: &mut AppState, controller: &ProxyController) {
        ui.horizontal(|ui| {
            ui.heading("Values");
            ui.add_space(16.0);

            if ui.button("➕ New Value").clicked() {
                self.show_new_value_dialog = true;
                self.new_value_name.clear();
            }

            if ui.button("🔄 Reload").clicked() {
                state.values = controller.load_values();
            }
        });

        ui.add_space(16.0);

        if self.show_new_value_dialog {
            self.new_value_dialog(ui, state, controller);
        }

        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.add(TextEdit::singleline(&mut self.search_filter).desired_width(200.0));
            if ui.button("✕").clicked() {
                self.search_filter.clear();
            }
        });

        ui.add_space(8.0);

        let available_size = ui.available_size();
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(250.0, available_size.y),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.values_list(ui, state, controller);
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                egui::vec2(available_size.x - 260.0, available_size.y),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.value_editor(ui, state, controller);
                },
            );
        });
    }

    fn new_value_dialog(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut AppState,
        controller: &ProxyController,
    ) {
        egui::Window::new("New Value")
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.new_value_name);
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() && !self.new_value_name.is_empty() {
                        let entry = ValueEntry {
                            name: self.new_value_name.clone(),
                            value: String::new(),
                        };
                        if controller.save_value(&entry).is_ok() {
                            state.values = controller.load_values();
                            self.selected_value = Some(self.new_value_name.clone());
                            self.editing_value.clear();
                        }
                        self.show_new_value_dialog = false;
                    }

                    if ui.button("Cancel").clicked() {
                        self.show_new_value_dialog = false;
                    }
                });
            });
    }

    fn values_list(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut AppState,
        controller: &ProxyController,
    ) {
        let available_height = ui.available_height();
        egui::ScrollArea::vertical()
            .id_salt("values_list")
            .max_height(available_height)
            .show(ui, |ui| {
                ui.set_min_width(230.0);

                if state.values.is_empty() {
                    ui.label(RichText::new("No values defined").weak());
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Values allow you to define reusable\nvariables in rules.")
                            .weak()
                            .small(),
                    );
                    return;
                }

                let filter = self.search_filter.to_lowercase();
                let values_snapshot: Vec<_> = state
                    .values
                    .iter()
                    .filter(|v| {
                        filter.is_empty()
                            || v.name.to_lowercase().contains(&filter)
                            || v.value.to_lowercase().contains(&filter)
                    })
                    .cloned()
                    .collect();

                ui.label(
                    RichText::new(format!("{} values", values_snapshot.len()))
                        .weak()
                        .small(),
                );
                ui.add_space(4.0);

                for value in &values_snapshot {
                    let is_selected = self.selected_value.as_ref() == Some(&value.name);

                    ui.horizontal(|ui| {
                        let response = ui
                            .selectable_label(is_selected, RichText::new(&value.name).monospace());

                        if response.clicked() {
                            self.selected_value = Some(value.name.clone());
                            self.editing_value = value.value.clone();
                        }

                        if response.secondary_clicked()
                            && controller.delete_value(&value.name).is_ok()
                        {
                            state.values = controller.load_values();
                            if self.selected_value.as_ref() == Some(&value.name) {
                                self.selected_value = None;
                                self.editing_value.clear();
                            }
                        }
                    });
                }
            });
    }

    fn value_editor(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut AppState,
        controller: &ProxyController,
    ) {
        if let Some(ref name) = self.selected_value.clone() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(name).strong().monospace().size(16.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(RichText::new("🗑 Delete").color(Color32::from_rgb(224, 108, 117)))
                        .clicked()
                        && controller.delete_value(name).is_ok()
                    {
                        state.values = controller.load_values();
                        self.selected_value = None;
                        self.editing_value.clear();
                    }

                    if ui.button("📋 Copy").clicked() {
                        ui.ctx().copy_text(self.editing_value.clone());
                    }

                    if ui.button("💾 Save").clicked() {
                        let updated = ValueEntry {
                            name: name.clone(),
                            value: self.editing_value.clone(),
                        };
                        if controller.save_value(&updated).is_ok() {
                            state.values = controller.load_values();
                        }
                    }
                });
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "Use {{{}}} in rules to reference this value.",
                    name
                ))
                .weak()
                .small(),
            );

            ui.add_space(8.0);

            let available_size = ui.available_size();
            egui::ScrollArea::vertical()
                .id_salt("value_editor")
                .max_height(available_size.y - 30.0)
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut self.editing_value)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(available_size.x - 16.0)
                            .desired_rows(20)
                            .hint_text("Enter value content..."),
                    );
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} characters", self.editing_value.len()))
                        .weak()
                        .small(),
                );
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Select a value to edit").weak());
            });
        }
    }
}
