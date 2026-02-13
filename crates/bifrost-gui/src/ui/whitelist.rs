use egui::{Color32, RichText};

use crate::state::{AccessMode, AppState, WhitelistEntry};

pub struct WhitelistPanel {
    new_entry: String,
    temp_entry: String,
}

impl Default for WhitelistPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl WhitelistPanel {
    pub fn new() -> Self {
        Self {
            new_entry: String::new(),
            temp_entry: String::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, state: &mut AppState) {
        ui.heading("Access Control");
        ui.add_space(8.0);

        self.show_access_mode(ui, state);
        ui.add_space(16.0);

        self.show_settings(ui, state);
        ui.add_space(16.0);

        if !matches!(state.whitelist.access_mode, AccessMode::AllowAll) {
            self.show_pending_authorization(ui, state);
            ui.add_space(16.0);
        }

        ui.collapsing("Permanent Whitelist", |ui| {
            self.show_permanent_whitelist(ui, state);
        });

        ui.add_space(8.0);

        ui.collapsing("Temporary Whitelist", |ui| {
            self.show_temporary_whitelist(ui, state);
        });
    }

    fn show_access_mode(&mut self, ui: &mut egui::Ui, state: &mut AppState) {
        ui.label(RichText::new("Access Mode").strong());
        ui.add_space(4.0);

        egui::Frame::group(ui.style())
            .inner_margin(12.0)
            .show(ui, |ui| {
                egui::ComboBox::from_id_salt("access_mode")
                    .selected_text(state.whitelist.access_mode.to_string())
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.whitelist.access_mode,
                            AccessMode::AllowAll,
                            "Allow All",
                        );
                        ui.selectable_value(
                            &mut state.whitelist.access_mode,
                            AccessMode::LocalOnly,
                            "Local Only",
                        );
                        ui.selectable_value(
                            &mut state.whitelist.access_mode,
                            AccessMode::Whitelist,
                            "Whitelist Only",
                        );
                        ui.selectable_value(
                            &mut state.whitelist.access_mode,
                            AccessMode::Interactive,
                            "Interactive",
                        );
                    });

                ui.add_space(8.0);
                let description = match state.whitelist.access_mode {
                    AccessMode::AllowAll => "All connections are allowed without restrictions.",
                    AccessMode::LocalOnly => {
                        "Only connections from localhost (127.0.0.1) are allowed."
                    }
                    AccessMode::Whitelist => {
                        "Only connections from whitelisted IP addresses are allowed."
                    }
                    AccessMode::Interactive => "Each new IP address requires manual authorization.",
                };
                ui.label(RichText::new(description).weak().small());
            });
    }

    fn show_settings(&mut self, ui: &mut egui::Ui, state: &mut AppState) {
        ui.label(RichText::new("Settings").strong());
        ui.add_space(4.0);

        egui::Frame::group(ui.style())
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.whitelist.allow_lan, "Allow LAN Access");
                });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "When enabled, allows connections from local network (192.168.x.x, 10.x.x.x, etc.)",
                    )
                    .weak()
                    .small(),
                );
            });
    }

    fn show_pending_authorization(&mut self, ui: &mut egui::Ui, state: &mut AppState) {
        if state.whitelist.pending_authorization.is_empty() {
            return;
        }

        ui.label(
            RichText::new("⚠ Pending Authorization")
                .strong()
                .color(Color32::YELLOW),
        );
        ui.add_space(4.0);

        egui::Frame::group(ui.style())
            .inner_margin(12.0)
            .show(ui, |ui| {
                let mut to_approve = None;
                let mut to_reject = None;

                for (idx, ip) in state.whitelist.pending_authorization.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(ip).monospace());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("❌ Reject").clicked() {
                                to_reject = Some(idx);
                            }
                            if ui.button("✅ Approve").clicked() {
                                to_approve = Some(idx);
                            }
                        });
                    });
                    ui.separator();
                }

                if let Some(idx) = to_approve {
                    if let Some(ip) = state.whitelist.pending_authorization.get(idx).cloned() {
                        state
                            .whitelist
                            .temporary
                            .push(WhitelistEntry { ip_or_cidr: ip });
                        state.whitelist.pending_authorization.remove(idx);
                    }
                }

                if let Some(idx) = to_reject {
                    state.whitelist.pending_authorization.remove(idx);
                }

                if !state.whitelist.pending_authorization.is_empty() {
                    ui.horizontal(|ui| {
                        if ui.button("Approve All").clicked() {
                            for ip in state.whitelist.pending_authorization.drain(..) {
                                state
                                    .whitelist
                                    .temporary
                                    .push(WhitelistEntry { ip_or_cidr: ip });
                            }
                        }
                        if ui.button("Reject All").clicked() {
                            state.whitelist.pending_authorization.clear();
                        }
                    });
                }
            });
    }

    fn show_permanent_whitelist(&mut self, ui: &mut egui::Ui, state: &mut AppState) {
        ui.add_space(8.0);
        ui.label(
            RichText::new("Permanent entries persist across proxy restarts.")
                .weak()
                .small(),
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Add IP/CIDR:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.new_entry)
                    .hint_text("e.g., 192.168.1.100 or 10.0.0.0/8")
                    .desired_width(200.0),
            );

            let can_add = !self.new_entry.trim().is_empty();
            let enter_pressed =
                response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            if ui
                .add_enabled(can_add, egui::Button::new("➕ Add"))
                .clicked()
                || (can_add && enter_pressed)
            {
                let entry = self.new_entry.trim().to_string();
                if !entry.is_empty()
                    && !state
                        .whitelist
                        .permanent
                        .iter()
                        .any(|e| e.ip_or_cidr == entry)
                {
                    state
                        .whitelist
                        .permanent
                        .push(WhitelistEntry { ip_or_cidr: entry });
                    self.new_entry.clear();
                }
            }
        });

        ui.add_space(8.0);

        if state.whitelist.permanent.is_empty() {
            ui.label(RichText::new("No permanent entries").weak());
        } else {
            let mut to_remove = None;

            egui::ScrollArea::vertical()
                .id_salt("permanent_whitelist_scroll")
                .max_height(150.0)
                .show(ui, |ui| {
                    for (idx, entry) in state.whitelist.permanent.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&entry.ip_or_cidr).monospace());

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("🗑").clicked() {
                                        to_remove = Some(idx);
                                    }
                                },
                            );
                        });
                        ui.separator();
                    }
                });

            if let Some(idx) = to_remove {
                state.whitelist.permanent.remove(idx);
            }
        }
    }

    fn show_temporary_whitelist(&mut self, ui: &mut egui::Ui, state: &mut AppState) {
        ui.add_space(8.0);
        ui.label(
            RichText::new("Temporary entries are cleared when the proxy stops.")
                .weak()
                .small(),
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Add IP:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.temp_entry)
                    .hint_text("e.g., 192.168.1.100")
                    .desired_width(200.0),
            );

            let can_add = !self.temp_entry.trim().is_empty();
            let enter_pressed =
                response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            if ui
                .add_enabled(can_add, egui::Button::new("➕ Add"))
                .clicked()
                || (can_add && enter_pressed)
            {
                let entry = self.temp_entry.trim().to_string();
                if !entry.is_empty()
                    && !state
                        .whitelist
                        .temporary
                        .iter()
                        .any(|e| e.ip_or_cidr == entry)
                {
                    state
                        .whitelist
                        .temporary
                        .push(WhitelistEntry { ip_or_cidr: entry });
                    self.temp_entry.clear();
                }
            }
        });

        ui.add_space(8.0);

        if state.whitelist.temporary.is_empty() {
            ui.label(RichText::new("No temporary entries").weak());
        } else {
            ui.horizontal(|ui| {
                ui.label(format!("{} entries", state.whitelist.temporary.len()));
                if ui.small_button("Clear All").clicked() {
                    state.whitelist.temporary.clear();
                }
            });

            let mut to_remove = None;

            egui::ScrollArea::vertical()
                .id_salt("temporary_whitelist_scroll")
                .max_height(150.0)
                .show(ui, |ui| {
                    for (idx, entry) in state.whitelist.temporary.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&entry.ip_or_cidr).monospace());

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("🗑").clicked() {
                                        to_remove = Some(idx);
                                    }
                                    if ui.small_button("📌 Make Permanent").clicked() {
                                        let ip = entry.ip_or_cidr.clone();
                                        if !state
                                            .whitelist
                                            .permanent
                                            .iter()
                                            .any(|e| e.ip_or_cidr == ip)
                                        {
                                            state
                                                .whitelist
                                                .permanent
                                                .push(WhitelistEntry { ip_or_cidr: ip });
                                        }
                                        to_remove = Some(idx);
                                    }
                                },
                            );
                        });
                        ui.separator();
                    }
                });

            if let Some(idx) = to_remove {
                state.whitelist.temporary.remove(idx);
            }
        }
    }
}
