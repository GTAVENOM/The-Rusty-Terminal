use std::path::PathBuf;

use crate::config::theme::AppTheme;
use crate::learning::shortcuts::{BASH_SNIPPET, POWERSHELL_SNIPPET};

const MARKER: &str = "Rusty Terminal shell integration (begin)";

/// Consent-based shell-integration installer. Rusty NEVER modifies shell
/// profiles silently: this dialog shows the exact snippet and appends it
/// only when the user clicks Install. Declining leaves everything as-is
/// (command capture degrades gracefully).
#[derive(Default)]
pub struct IntegrationUi {
    pub open: bool,
    status: Option<String>,
    /// API-key entry buffer (never persisted in plaintext, never logged).
    api_key_input: String,
}

fn powershell_profile_path() -> Option<PathBuf> {
    // $PROFILE for Windows PowerShell 5.x (the default powershell.exe).
    dirs::document_dir().map(|d| {
        d.join("WindowsPowerShell")
            .join("Microsoft.PowerShell_profile.ps1")
    })
}

fn is_installed(path: &PathBuf) -> bool {
    std::fs::read_to_string(path)
        .map(|c| c.contains(MARKER))
        .unwrap_or(false)
}

fn append_snippet(path: &PathBuf, snippet: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut content = std::fs::read_to_string(path).unwrap_or_default();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push('\n');
    content.push_str(snippet);
    content.push('\n');
    std::fs::write(path, content)
}

impl IntegrationUi {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        theme: &mut AppTheme,
        theme_changed: &mut bool,
    ) {
        if !self.open {
            return;
        }
        let accent = theme.accent;
        let mut open = self.open;
        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(false)
            .default_size(egui::vec2(560.0, 560.0))
            .vscroll(true)
            .show(ctx, |ui| {
                // ---- Theme ----
                ui.label(
                    egui::RichText::new("Theme")
                        .color(accent)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "Current: {}",
                        theme.name,
                    ))
                    .color(egui::Color32::GRAY),
                );
                ui.horizontal(|ui| {
                    let dark = ui.selectable_label(
                        !theme.light_mode,
                        "Dark",
                    );
                    let light = ui.selectable_label(
                        theme.light_mode,
                        "Light",
                    );
                    if dark.clicked() && theme.light_mode {
                        *theme = AppTheme::dark();
                        *theme_changed = true;
                    }
                    if light.clicked() && !theme.light_mode {
                        *theme = AppTheme::light();
                        *theme_changed = true;
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "Custom themes: write theme.json in \
                         %APPDATA%\\RustyTerminal\\ — see README.",
                    )
                    .color(egui::Color32::GRAY)
                    .small(),
                );

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                // ---- Anthropic API key ----
                ui.label(
                    egui::RichText::new("Anthropic API key (Ctrl+K intents)")
                        .color(accent)
                        .strong(),
                );
                if std::env::var("ANTHROPIC_API_KEY")
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
                {
                    ui.colored_label(
                        egui::Color32::LIGHT_GREEN,
                        "✔ using ANTHROPIC_API_KEY from the environment",
                    );
                } else {
                    if crate::intent::api_key::is_configured() {
                        ui.colored_label(
                            egui::Color32::LIGHT_GREEN,
                            "✔ an encrypted key is stored for this Windows \
                             user",
                        );
                    } else {
                        ui.colored_label(
                            egui::Color32::GRAY,
                            "no key configured — Ctrl+K will report this",
                        );
                    }
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(
                                &mut self.api_key_input,
                            )
                            .password(true)
                            .desired_width(320.0)
                            .hint_text("sk-ant-…"),
                        );
                        if ui.button("Save encrypted").clicked()
                            && !self.api_key_input.trim().is_empty()
                        {
                            self.status = Some(
                                match crate::intent::api_key::store_encrypted_key(
                                    &self.api_key_input,
                                ) {
                                    Ok(()) => "API key stored (DPAPI, \
                                               current user)"
                                        .to_string(),
                                    Err(e) => format!("could not store: {e}"),
                                },
                            );
                            self.api_key_input.clear();
                        }
                        if ui.button("Forget").clicked() {
                            let _ =
                                crate::intent::api_key::delete_stored_key();
                            self.status =
                                Some("stored key deleted".to_string());
                        }
                    });
                    ui.label(
                        egui::RichText::new(
                            "Stored with Windows DPAPI under your user \
                             account. It is never written to the database \
                             and never logged.",
                        )
                        .color(egui::Color32::GRAY)
                        .small(),
                    );
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("Shell integration")
                        .color(accent)
                        .strong(),
                );
                ui.label(
                    "Rusty tracks command boundaries and exit codes via \
                     shell-integration marks (OSC 133). Installing adds the \
                     snippet below to your shell profile — nothing is \
                     modified without clicking Install, and you can remove \
                     the marked block at any time.",
                );
                ui.add_space(6.0);

                ui.label(
                    egui::RichText::new("PowerShell ($PROFILE)")
                        .color(accent)
                        .strong(),
                );
                match powershell_profile_path() {
                    Some(path) => {
                        ui.monospace(path.display().to_string());
                        egui::ScrollArea::vertical()
                            .id_salt("ps_snippet")
                            .max_height(140.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(
                                        &mut POWERSHELL_SNIPPET.to_string(),
                                    )
                                    .code_editor()
                                    .desired_width(f32::INFINITY)
                                    .interactive(false),
                                );
                            });
                        if is_installed(&path) {
                            ui.colored_label(
                                egui::Color32::LIGHT_GREEN,
                                "✔ installed",
                            );
                        } else if ui
                            .button("Install into PowerShell profile")
                            .clicked()
                        {
                            self.status = Some(
                                match append_snippet(
                                    &path,
                                    POWERSHELL_SNIPPET,
                                ) {
                                    Ok(()) => "PowerShell snippet installed — \
                                               takes effect in new tabs"
                                        .to_string(),
                                    Err(e) => format!("install failed: {e}"),
                                },
                            );
                        }
                    },
                    None => {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            "couldn't resolve the Documents folder",
                        );
                    },
                }

                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("WSL (bash/zsh — ~/.bashrc)")
                        .color(accent)
                        .strong(),
                );
                ui.label(
                    "Copy this into your ~/.bashrc (or ~/.zshrc) inside \
                     WSL — Rusty doesn't write into the WSL filesystem:",
                );
                egui::ScrollArea::vertical()
                    .id_salt("bash_snippet")
                    .max_height(120.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(
                                &mut BASH_SNIPPET.to_string(),
                            )
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                        );
                    });
                if ui.button("Copy WSL snippet to clipboard").clicked() {
                    ctx.copy_text(BASH_SNIPPET.to_string());
                    self.status =
                        Some("WSL snippet copied to clipboard".to_string());
                }

                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "cmd.exe: boundary marks are injected automatically \
                         via the PROMPT environment variable (no profile \
                         changes; no exit codes — a cmd limitation).",
                    )
                    .color(egui::Color32::GRAY),
                );

                if let Some(status) = &self.status {
                    ui.add_space(4.0);
                    ui.colored_label(accent, status);
                }
            });
        self.open = open;
    }
}
