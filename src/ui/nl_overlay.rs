//! Ctrl+V natural-language input overlay.
//!
//! A separate egui TextEdit — NL text never touches the PTY. On success
//! the overlay shows the LITERAL command + tier badge + intent name; the
//! user chooses to insert (Tier 1) or gets the confirm flow (Tier 2,
//! stage d). Nothing is ever executed from here: insertion carries no
//! newline.

use crate::intent::client::IntentResponse;
use crate::intent::schema::Intent;
use crate::safety::tier_classifier::Tier;

#[derive(Default)]
pub struct NlOverlay {
    pub open: bool,
    phrase: String,
    request_focus: bool,
    /// In-flight request id (spinner shown while set).
    pub pending_request: Option<u64>,
    /// Last completed suggestion, awaiting user action.
    pub suggestion: Option<SuggestionView>,
    error: Option<String>,
    next_request_id: u64,
}

pub struct SuggestionView {
    pub intent: Intent,
    pub command: String,
    pub tier: Tier,
}

pub enum OverlayAction {
    None,
    /// Send this phrase to the intent engine.
    Submit { request_id: u64, phrase: String },
    /// Insert the suggested Tier-1 command into the input line (no `\r`).
    InsertTier1(String),
    /// Open the Tier-2 confirm dialog for this suggestion (stage d).
    ConfirmTier2 { command: String, intent_name: String },
    Close,
}

impl NlOverlay {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.phrase.clear();
            self.suggestion = None;
            self.error = None;
            self.pending_request = None;
            self.request_focus = true;
        }
    }

    /// Deliver an intent-engine response. Stale responses (id mismatch)
    /// are dropped.
    pub fn on_response(
        &mut self,
        response: IntentResponse,
        render: impl Fn(&Intent) -> String,
    ) {
        if self.pending_request != Some(response.request_id) {
            return;
        }
        self.pending_request = None;
        match response.result {
            Ok(intent) => {
                let command = render(&intent);
                let tier = intent.tier();
                self.suggestion = Some(SuggestionView {
                    intent,
                    command,
                    tier,
                });
                self.error = None;
            },
            Err(err) => {
                self.error = Some(err.to_string());
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, accent: egui::Color32) -> OverlayAction {
        if !self.open {
            return OverlayAction::None;
        }
        let mut action = OverlayAction::None;
        let screen = ctx.content_rect();

        egui::Window::new("nl_overlay")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(egui::pos2(
                screen.center().x - 280.0,
                screen.min.y + 48.0,
            ))
            .fixed_size(egui::vec2(560.0, 0.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("✨").color(accent));
                    let edit = ui.add_enabled(
                        self.pending_request.is_none(),
                        egui::TextEdit::singleline(&mut self.phrase)
                            .desired_width(f32::INFINITY)
                            .hint_text(
                                "Describe what you want… e.g. \"show me \
                                 docker logs for the api container\"",
                            ),
                    );
                    if self.request_focus {
                        edit.request_focus();
                        self.request_focus = false;
                    }
                    let submitted = edit.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && !self.phrase.trim().is_empty();
                    if submitted && self.pending_request.is_none() {
                        self.next_request_id += 1;
                        self.pending_request = Some(self.next_request_id);
                        self.suggestion = None;
                        self.error = None;
                        action = OverlayAction::Submit {
                            request_id: self.next_request_id,
                            phrase: self.phrase.trim().to_string(),
                        };
                    }
                });

                if self.pending_request.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("asking Claude…");
                    });
                }

                if let Some(err) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }

                let mut dismiss_suggestion = false;
                if let Some(view) = &self.suggestion {
                    ui.separator();
                    ui.horizontal(|ui| {
                        let (badge, badge_color) = match view.tier {
                            Tier::ReadOnly => (
                                "Tier 1 · read-only",
                                egui::Color32::from_rgb(0x64, 0xb5, 0x64),
                            ),
                            Tier::Idempotent => (
                                "Tier 2 · needs confirmation",
                                egui::Color32::from_rgb(0xe0, 0xa0, 0x40),
                            ),
                            // Unreachable by construction (schema has no
                            // Tier-3 intents); shown defensively.
                            Tier::Destructive => (
                                "[DESTRUCTIVE — reference only]",
                                egui::Color32::LIGHT_RED,
                            ),
                        };
                        ui.label(
                            egui::RichText::new(badge)
                                .color(badge_color)
                                .small(),
                        );
                        ui.label(
                            egui::RichText::new(view.intent.name())
                                .color(egui::Color32::GRAY)
                                .small(),
                        );
                    });
                    ui.add_space(2.0);
                    // The literal command, front and center.
                    ui.monospace(
                        egui::RichText::new(&view.command).size(15.0),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        match view.tier {
                            Tier::ReadOnly => {
                                if ui.button("Insert (Enter)").clicked()
                                    || ui.input(|i| {
                                        i.key_pressed(egui::Key::Enter)
                                    })
                                {
                                    action = OverlayAction::InsertTier1(
                                        view.command.clone(),
                                    );
                                }
                            },
                            Tier::Idempotent => {
                                if ui.button("Continue… (Enter)").clicked()
                                    || ui.input(|i| {
                                        i.key_pressed(egui::Key::Enter)
                                    })
                                {
                                    action = OverlayAction::ConfirmTier2 {
                                        command: view.command.clone(),
                                        intent_name: view
                                            .intent
                                            .name()
                                            .to_string(),
                                    };
                                }
                            },
                            Tier::Destructive => {
                                ui.colored_label(
                                    egui::Color32::LIGHT_RED,
                                    "Reference only — retyping or manual copy required to execute (cannot auto-insert)",
                                );
                            },
                        }
                        if ui.button("Dismiss (Esc)").clicked() {
                            dismiss_suggestion = true;
                        }
                    });
                }
                if dismiss_suggestion {
                    self.suggestion = None;
                }

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    action = OverlayAction::Close;
                }
            });
        action
    }
}
