use std::sync::mpsc::{self, Receiver};

use crate::learning::db::{DbCommand, DbHandle, HistoryEntry};

/// Ctrl+R fuzzy history search over the SQLite command history.
/// Selecting an entry inserts it into the focused pane's input line —
/// never executes it (no `\r` is ever sent).
#[derive(Default)]
pub struct HistoryPalette {
    pub open: bool,
    query: String,
    last_sent_query: Option<String>,
    entries: Vec<HistoryEntry>,
    selected: usize,
    reply: Option<Receiver<Vec<HistoryEntry>>>,
    request_focus: bool,
}

pub enum PaletteAction {
    None,
    /// Insert this text into the focused pane's input line (no newline).
    Insert(String),
    Close,
}

impl HistoryPalette {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.query.clear();
            self.last_sent_query = None;
            self.entries.clear();
            self.selected = 0;
            self.request_focus = true;
        }
    }

    fn poll_reply(&mut self) {
        if let Some(rx) = &self.reply {
            if let Ok(entries) = rx.try_recv() {
                self.entries = entries;
                self.selected = 0;
                self.reply = None;
            }
        }
    }

    fn request_search(&mut self, db: &DbHandle) {
        if self.last_sent_query.as_deref() == Some(self.query.as_str()) {
            return;
        }
        let (tx, rx) = mpsc::channel();
        db.send(DbCommand::SearchHistory {
            query: self.query.clone(),
            limit: 50,
            reply: tx,
        });
        self.reply = Some(rx);
        self.last_sent_query = Some(self.query.clone());
    }

    /// Fuzzy subsequence score: all query chars must appear in order;
    /// tighter matches score higher. Applied UI-side over the SQL result.
    fn fuzzy_rank(entries: &mut Vec<HistoryEntry>, query: &str) {
        if query.is_empty() {
            return;
        }
        let q: Vec<char> =
            query.chars().flat_map(|c| c.to_lowercase()).collect();
        entries.retain(|e| {
            let mut qi = 0;
            for c in e.command.to_lowercase().chars() {
                if qi < q.len() && c == q[qi] {
                    qi += 1;
                }
            }
            qi == q.len()
        });
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        db: &DbHandle,
    ) -> PaletteAction {
        if !self.open {
            return PaletteAction::None;
        }
        self.poll_reply();
        self.request_search(db);

        let mut action = PaletteAction::None;
        let screen = ctx.content_rect();
        egui::Window::new("history_palette")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(egui::pos2(
                screen.center().x - 260.0,
                screen.min.y + 60.0,
            ))
            .fixed_size(egui::vec2(520.0, 320.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("⌕");
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .desired_width(f32::INFINITY)
                            .hint_text("search command history…"),
                    );
                    if self.request_focus {
                        edit.request_focus();
                        self.request_focus = false;
                    }
                });
                ui.separator();

                let mut display = self.entries.clone();
                Self::fuzzy_rank(&mut display, &self.query);
                if self.selected >= display.len() && !display.is_empty() {
                    self.selected = display.len() - 1;
                }

                let (up, down, enter, escape) = ctx.input_mut(|i| {
                    (
                        i.consume_key(
                            egui::Modifiers::NONE,
                            egui::Key::ArrowUp,
                        ),
                        i.consume_key(
                            egui::Modifiers::NONE,
                            egui::Key::ArrowDown,
                        ),
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                        i.consume_key(
                            egui::Modifiers::NONE,
                            egui::Key::Escape,
                        ),
                    )
                });
                if up && self.selected > 0 {
                    self.selected -= 1;
                }
                if down && self.selected + 1 < display.len() {
                    self.selected += 1;
                }
                if escape {
                    action = PaletteAction::Close;
                }
                if enter {
                    if let Some(entry) = display.get(self.selected) {
                        action = PaletteAction::Insert(entry.command.clone());
                    }
                }

                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for (idx, entry) in display.iter().enumerate() {
                            let selected = idx == self.selected;
                            let label = format!(
                                "{}    ({}×)",
                                entry.command, entry.count
                            );
                            let response =
                                ui.selectable_label(selected, label);
                            if response.clicked() {
                                action = PaletteAction::Insert(
                                    entry.command.clone(),
                                );
                            }
                            if selected {
                                response.scroll_to_me(None);
                            }
                        }
                        if display.is_empty() {
                            ui.label(
                                egui::RichText::new("no matches")
                                    .color(egui::Color32::GRAY),
                            );
                        }
                    });
            });
        action
    }
}
