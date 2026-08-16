//! Tier-2 confirmation dialog.
//!
//! Safety invariant: a Tier-2 suggested command is NEVER written to the
//! PTY until the user confirms. The dialog is the only place a
//! `ConfirmationToken` can be minted, and `Pane::run_confirmed` is the only
//! code path allowed to append `\r` to programmatically-injected text.

use crate::safety::tier_classifier::Tier;
use crate::terminal::input_gate::ConfirmationToken;

pub struct ConfirmDialog {
    /// The pending suggestion, if any. Exactly one at a time — there is no
    /// queue, which is what makes "no autonomous multi-step execution"
    /// structural rather than a policy.
    pending: Option<PendingConfirm>,
}

impl Default for ConfirmDialog {
    fn default() -> Self {
        Self { pending: None }
    }
}

pub struct PendingConfirm {
    pub command: String,
    pub intent_name: String,
    pub tier: Tier,
}

pub enum ConfirmOutcome {
    None,
    /// User confirmed execution: the token proves this came from the
    /// dialog's Run handler.
    Run {
        command: String,
        token: ConfirmationToken,
    },
    /// Put the text on the input line without executing — from that point
    /// it is the user's own input and they own the Enter keystroke.
    InsertOnly(String),
    Cancelled,
}

impl ConfirmDialog {
    /// Queue a suggestion for confirmation, replacing any previous pending
    /// one (max one per approval step).
    pub fn request(
        &mut self,
        command: String,
        intent_name: String,
        tier: Tier,
    ) {
        self.pending = Some(PendingConfirm {
            command,
            intent_name,
            tier,
        });
    }

    #[allow(dead_code)]
    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        accent: egui::Color32,
    ) -> ConfirmOutcome {
        let Some(pending) = &self.pending else {
            return ConfirmOutcome::None;
        };
        let mut outcome = ConfirmOutcome::None;
        let screen = ctx.content_rect();

        egui::Window::new("Confirm command")
            .collapsible(false)
            .resizable(false)
            .fixed_pos(egui::pos2(
                screen.center().x - 260.0,
                screen.center().y - 90.0,
            ))
            .fixed_size(egui::vec2(520.0, 0.0))
            .show(ctx, |ui| {
                let (tier_label, tier_color) = match pending.tier {
                    Tier::ReadOnly => (
                        "Tier 1 · read-only",
                        egui::Color32::from_rgb(0x64, 0xb5, 0x64),
                    ),
                    Tier::Idempotent => (
                        "Tier 2 · changes state but is safe to re-run",
                        egui::Color32::from_rgb(0xe0, 0xa0, 0x40),
                    ),
                    Tier::Destructive => (
                        "Tier 3 · BLOCKED",
                        egui::Color32::LIGHT_RED,
                    ),
                };
                ui.label(
                    egui::RichText::new(format!(
                        "{tier_label} · {}",
                        pending.intent_name,
                    ))
                    .color(tier_color)
                    .small(),
                );
                ui.add_space(6.0);
                // The literal, fully-expanded command — never a summary.
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(0x0e, 0x0e, 0x12))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.monospace(
                            egui::RichText::new(&pending.command)
                                .size(15.0)
                                .color(accent),
                        );
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(egui::RichText::new("Run").strong())
                        .clicked()
                    {
                        // The ONLY place a ConfirmationToken is minted.
                        outcome = ConfirmOutcome::Run {
                            command: pending.command.clone(),
                            token: ConfirmationToken::issue(),
                        };
                    }
                    if ui.button("Insert only").clicked() {
                        outcome =
                            ConfirmOutcome::InsertOnly(pending.command.clone());
                    }
                    if ui.button("Cancel").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Escape))
                    {
                        outcome = ConfirmOutcome::Cancelled;
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "Run executes exactly this one command. Insert only \
                         puts it on the input line for you to edit and run \
                         yourself.",
                    )
                    .color(egui::Color32::GRAY)
                    .small(),
                );
            });

        if !matches!(outcome, ConfirmOutcome::None) {
            self.pending = None;
        }
        outcome
    }
}
