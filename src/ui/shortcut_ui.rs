use std::sync::mpsc::{self, Receiver};

use crate::learning::db::{DbCommand, DbHandle, Shortcut};

/// Toast + naming dialog for "save these commands as a shortcut?", plus a
/// palette to invoke saved shortcuts. Shortcuts are only ever created with
/// explicit user approval and are never auto-run: invoking one inserts its
/// commands into the input line one at a time, each requiring the user's
/// own Enter.
#[derive(Default)]
pub struct ShortcutUi {
    /// Pending "save as shortcut?" suggestion from the learning engine.
    pub suggestion: Option<SuggestionState>,
    /// Shortcut palette (list + invoke).
    pub palette_open: bool,
    shortcuts: Vec<Shortcut>,
    reply: Option<Receiver<Vec<Shortcut>>>,
    /// Commands queued for step-by-step insertion (never auto-run).
    pub pending_steps: Vec<String>,
}

pub struct SuggestionState {
    pub commands: Vec<String>,
    pub count: u32,
    pub naming: bool,
    pub name: String,
}

pub enum ShortcutAction {
    None,
    /// Insert this text into the focused pane's input line (no newline).
    InsertStep(String),
}

impl ShortcutUi {
    pub fn on_suggestion(&mut self, commands: Vec<String>, count: u32) {
        // One suggestion at a time; newer wins.
        self.suggestion = Some(SuggestionState {
            commands,
            count,
            naming: false,
            name: String::new(),
        });
    }

    pub fn toggle_palette(&mut self, db: &DbHandle) {
        self.palette_open = !self.palette_open;
        if self.palette_open {
            let (tx, rx) = mpsc::channel();
            db.send(DbCommand::ListShortcuts { reply: tx });
            self.reply = Some(rx);
        }
    }

    fn poll_reply(&mut self) {
        if let Some(rx) = &self.reply {
            if let Ok(shortcuts) = rx.try_recv() {
                self.shortcuts = shortcuts;
                self.reply = None;
            }
        }
    }

    /// The suggestion toast + naming dialog. Bottom-right corner.
    pub fn show_suggestion(
        &mut self,
        ctx: &egui::Context,
        db: &DbHandle,
        accent: egui::Color32,
    ) {
        let Some(state) = &mut self.suggestion else {
            return;
        };
        let mut dismiss = false;
        let screen = ctx.content_rect();
        egui::Window::new("shortcut_suggestion")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(egui::pos2(
                screen.max.x - 380.0,
                screen.max.y - if state.naming { 190.0 } else { 150.0 },
            ))
            .fixed_size(egui::vec2(360.0, 0.0))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "You've run these together {}× — save as a shortcut?",
                        state.count
                    ))
                    .color(accent),
                );
                for cmd in &state.commands {
                    ui.monospace(format!("  {cmd}"));
                }
                if state.naming {
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        let edit = ui.text_edit_singleline(&mut state.name);
                        let submit = (edit.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            || ui.button("Save").clicked();
                        if submit && !state.name.trim().is_empty() {
                            db.send(DbCommand::SaveShortcut {
                                name: state.name.trim().to_string(),
                                commands: state.commands.clone(),
                            });
                            dismiss = true;
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        if ui.button("Save as shortcut…").clicked() {
                            state.naming = true;
                        }
                        if ui.button("Dismiss").clicked() {
                            dismiss = true;
                        }
                    });
                }
            });
        if dismiss {
            self.suggestion = None;
        }
    }

    /// The shortcut palette. Selecting a shortcut queues its commands for
    /// step-by-step insertion.
    pub fn show_palette(
        &mut self,
        ctx: &egui::Context,
        db: &DbHandle,
    ) -> ShortcutAction {
        if !self.palette_open {
            return ShortcutAction::None;
        }
        self.poll_reply();

        let mut action = ShortcutAction::None;
        let mut delete: Option<String> = None;
        let screen = ctx.content_rect();
        let mut open = self.palette_open;
        egui::Window::new("Shortcuts")
            .open(&mut open)
            .resizable(false)
            .default_pos(egui::pos2(
                screen.center().x - 220.0,
                screen.min.y + 80.0,
            ))
            .fixed_size(egui::vec2(440.0, 280.0))
            .show(ctx, |ui| {
                if self.shortcuts.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "No shortcuts yet. Run a command sequence a few \
                             times and Rusty will offer to save it.",
                        )
                        .color(egui::Color32::GRAY),
                    );
                    return;
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for shortcut in &self.shortcuts {
                        ui.horizontal(|ui| {
                            if ui
                                .button(
                                    egui::RichText::new(&shortcut.name)
                                        .strong(),
                                )
                                .clicked()
                            {
                                // Queue all steps; first is inserted now,
                                // the rest one at a time as each command
                                // completes (each needs the user's Enter).
                                let mut steps = shortcut.commands.clone();
                                if !steps.is_empty() {
                                    let first = steps.remove(0);
                                    self.pending_steps = steps;
                                    action =
                                        ShortcutAction::InsertStep(first);
                                }
                            }
                            ui.label(
                                egui::RichText::new(
                                    shortcut.commands.join("  →  "),
                                )
                                .monospace()
                                .color(egui::Color32::GRAY),
                            );
                            if ui.small_button("🗑").clicked() {
                                delete = Some(shortcut.name.clone());
                            }
                        });
                    }
                });
            });
        self.palette_open = open;
        if matches!(action, ShortcutAction::InsertStep(_)) {
            self.palette_open = false;
        }
        if let Some(name) = delete {
            db.send(DbCommand::DeleteShortcut { name: name.clone() });
            self.shortcuts.retain(|s| s.name != name);
        }
        action
    }

    /// Called when a command finished in the focused pane (OSC 133;D):
    /// offer the next queued shortcut step, if any.
    pub fn next_step(&mut self) -> Option<String> {
        if self.pending_steps.is_empty() {
            None
        } else {
            Some(self.pending_steps.remove(0))
        }
    }
}
