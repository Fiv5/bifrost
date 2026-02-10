use egui::RichText;

use crate::state::{AppState, WhitelistEntry};

pub struct WhitelistPanel {
    new_entry: String,
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
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, state: &mut AppState) {
        ui.heading("IP Whitelist");
        ui.add_space(8.0);
        ui.label(
            RichText::new("Configure which IP addresses or CIDR ranges can access the proxy.")
                .weak(),
        );
        ui.add_space(16.0);

        self.add_entry_section(ui, state);
        ui.add_space(16.0);
        self.entries_list(ui, state);
    }

    fn add_entry_section(&mut self, ui: &mut egui::Ui, state: &mut AppState) {
        egui::Frame::group(ui.style())
            .inner_margin(12.0)
            .show(ui, |ui| {
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
                            && !state.whitelist.iter().any(|e| e.ip_or_cidr == entry)
                        {
                            state.whitelist.push(WhitelistEntry { ip_or_cidr: entry });
                            self.new_entry.clear();
                        }
                    }
                });
            });
    }

    fn entries_list(&self, ui: &mut egui::Ui, state: &mut AppState) {
        if state.whitelist.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(RichText::new("No whitelist entries").weak().size(14.0));
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "When empty, all IP addresses are allowed (based on Allow LAN setting)",
                    )
                    .weak()
                    .small(),
                );
            });
            return;
        }

        ui.label(format!("{} entries", state.whitelist.len()));
        ui.add_space(8.0);

        let mut to_remove = None;

        egui::ScrollArea::vertical()
            .id_salt("whitelist_scroll")
            .max_height(400.0)
            .show(ui, |ui| {
                for (idx, entry) in state.whitelist.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&entry.ip_or_cidr).monospace());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("🗑").clicked() {
                                to_remove = Some(idx);
                            }
                        });
                    });
                    ui.separator();
                }
            });

        if let Some(idx) = to_remove {
            state.whitelist.remove(idx);
        }
    }
}
