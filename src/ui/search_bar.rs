/// Scrollback search bar state (Ctrl+Shift+F). Searches the focused pane's
/// terminal grid via the vendored backend's `search_scroll`.
#[derive(Default)]
pub struct SearchBar {
    pub open: bool,
    pub query: String,
    pub match_count: Option<usize>,
    pub request_focus: bool,
}

pub enum SearchAction {
    None,
    Search { forward: bool },
    Close,
}

impl SearchBar {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.request_focus = true;
        } else {
            self.match_count = None;
        }
    }

    /// Render the bar; returns what the app should do.
    pub fn show(&mut self, ui: &mut egui::Ui) -> SearchAction {
        let mut action = SearchAction::None;
        ui.horizontal(|ui| {
            ui.label("Find:");
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .desired_width(240.0)
                    .hint_text("search scrollback (regex ok)"),
            );
            if self.request_focus {
                edit.request_focus();
                self.request_focus = false;
            }
            let enter = edit.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let shift = ui.input(|i| i.modifiers.shift);
            if enter {
                action = SearchAction::Search { forward: !shift };
                self.request_focus = true;
            }
            if ui.small_button("▼").clicked() {
                action = SearchAction::Search { forward: true };
            }
            if ui.small_button("▲").clicked() {
                action = SearchAction::Search { forward: false };
            }
            if let Some(count) = self.match_count {
                if count == 0 {
                    ui.colored_label(egui::Color32::LIGHT_RED, "no matches");
                } else {
                    ui.label(format!("{count} matches"));
                }
            }
            if ui.small_button("✕").clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                action = SearchAction::Close;
            }
        });
        action
    }
}
