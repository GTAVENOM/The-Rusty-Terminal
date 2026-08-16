use std::sync::mpsc::{self, Receiver, Sender};

use egui_term::PtyEvent;

use crate::config::theme::AppTheme;
use crate::context::scanner::ProjectContext;
use crate::intent::client::{self, IntentRequest, IntentResponse};
use crate::intent::render;
use crate::intent::schema::ToolsetScope;
use crate::learning::db::{self, DbCommand, DbEvent, DbHandle};
use crate::learning::shortcuts::{parse_osc_body, ShellMark};
use crate::safety::tier_classifier::{classify, Tier};
use crate::terminal::pane::{PaneId, SplitDir};
use crate::terminal::shell::{self, ShellKind};
use crate::ui::confirm_dialog::{ConfirmDialog, ConfirmOutcome};
use crate::ui::history_palette::{HistoryPalette, PaletteAction};
use crate::ui::nl_overlay::{NlOverlay, OverlayAction};
use crate::ui::search_bar::{SearchAction, SearchBar};
use crate::ui::shortcut_ui::{ShortcutAction, ShortcutUi};
use crate::ui::tabs::TabManager;
use crate::ui::panes;

pub struct RustyApp {
    pty_event_sender: Sender<(u64, PtyEvent)>,
    pty_event_receiver: Receiver<(u64, PtyEvent)>,
    tabs: TabManager,
    available_shells: Vec<ShellKind>,
    theme: AppTheme,
    search: SearchBar,
    /// Open the "pick a shell" popup for the new-tab button.
    shell_menu_open: bool,
    /// Learning engine (may be None if the DB failed to open — the
    /// terminal still works, learning features are disabled).
    db: Option<DbHandle>,
    db_event_receiver: Receiver<DbEvent>,
    history: HistoryPalette,
    shortcuts: ShortcutUi,
    integration: crate::ui::integration_ui::IntegrationUi,
    /// Ctrl+K natural-language intent overlay.
    nl: NlOverlay,
    confirm: ConfirmDialog,
    intent_reply_sender: Sender<IntentResponse>,
    intent_reply_receiver: Receiver<IntentResponse>,
    /// Transient error banner text.
    error_banner: Option<String>,
    /// Session already persisted for this exit (persist-once guard).
    session_saved: bool,
}

impl RustyApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (pty_event_sender, pty_event_receiver) = mpsc::channel();
        // Custom theme.json if present, else the built-in dark theme.
        let theme = crate::config::theme::load_custom();
        theme.apply_chrome(&cc.egui_ctx);

        let (db_event_sender, db_event_receiver) = mpsc::channel();
        let (intent_reply_sender, intent_reply_receiver) = mpsc::channel();
        let db = db::spawn(
            db::default_db_path(),
            db_event_sender,
            cc.egui_ctx.clone(),
        )
        .map_err(|err| log::error!("db thread failed to start: {err}"))
        .ok();

        let mut app = Self {
            pty_event_sender,
            pty_event_receiver,
            tabs: TabManager::new(),
            available_shells: shell::detect_available_shells(),
            theme,
            search: SearchBar::default(),
            shell_menu_open: false,
            db,
            db_event_receiver,
            history: HistoryPalette::default(),
            shortcuts: ShortcutUi::default(),
            integration: Default::default(),
            nl: NlOverlay::default(),
            confirm: ConfirmDialog::default(),
            intent_reply_sender,
            intent_reply_receiver,
            error_banner: None,
            session_saved: false,
        };

        // Restore the previous session if one exists; otherwise start
        // with a single PowerShell tab.
        let session_path = crate::session::restore::session_path();
        let restored = crate::session::restore::load(&session_path)
            .ok()
            .filter(|s| !s.tabs.is_empty())
            .map(|session| {
                app.tabs.restore_from_session(
                    &session,
                    cc.egui_ctx.clone(),
                    app.pty_event_sender.clone(),
                )
            })
            .unwrap_or(0);
        if restored == 0 {
            app.tabs.open_tab(
                cc.egui_ctx.clone(),
                app.pty_event_sender.clone(),
                ShellKind::PowerShell,
            );
        } else {
            log::info!("restored {restored} tab(s) from session");
        }
        app
    }

    /// Persist the session once, when the window is closing.
    fn maybe_save_session(&mut self) {
        if self.session_saved {
            return;
        }
        self.session_saved = true;
        if let Err(err) = crate::session::restore::persist(
            &self.tabs,
            &crate::session::restore::session_path(),
        ) {
            log::error!("failed to save session: {err}");
        }
    }

    fn drain_pty_events(&mut self) {
        while let Ok((pane_id, event)) = self.pty_event_receiver.try_recv() {
            match event {
                PtyEvent::ShellIntegrationOsc(ref body) => {
                    self.on_shell_mark(pane_id, body);
                },
                other => self.tabs.on_pty_event(pane_id, other),
            }
        }
        // Poll each pane's input gate for newly submitted command lines.
        self.collect_submitted_commands();
    }

    /// Handle an OSC 133 / cwd mark from a pane's output stream.
    fn on_shell_mark(&mut self, pane_id: PaneId, body: &str) {
        let Some(mark) = parse_osc_body(body) else {
            return;
        };
        match mark {
            ShellMark::Finished { exit_code } => {
                if let Some(code) = exit_code {
                    if let Some(db) = &self.db {
                        db.send(DbCommand::RecordExitCode {
                            pane_id,
                            exit_code: code,
                        });
                    }
                }
                // A command finished: if a shortcut is stepping through its
                // commands in the focused pane, offer the next step.
                let focused = self
                    .tabs
                    .active_tab_mut()
                    .map(|t| t.focused_pane);
                if focused == Some(pane_id) {
                    if let Some(step) = self.shortcuts.next_step() {
                        self.insert_into_focused(&step);
                    }
                }
            },
            ShellMark::Cwd(cwd) => {
                for tab in self.tabs.tabs.values_mut() {
                    if let Some(pane) = tab.root.find_pane_mut(pane_id) {
                        pane.working_directory =
                            Some(std::path::PathBuf::from(cwd));
                        break;
                    }
                }
            },
            // PromptStart / CommandStart / PreExec: boundaries only; the
            // input gate handles text capture at Enter time.
            _ => {},
        }
    }

    /// Pull Enter-submitted lines out of every pane's input gate and record
    /// them in the history DB.
    fn collect_submitted_commands(&mut self) {
        let Some(db) = &self.db else {
            return;
        };
        for tab in self.tabs.tabs.values_mut() {
            for pane_id in tab.root.pane_ids() {
                let Some(pane) = tab.root.find_pane_mut(pane_id) else {
                    continue;
                };
                let submitted = pane
                    .input_gate
                    .lock()
                    .ok()
                    .and_then(|mut g| g.take_submitted());
                if let Some(line) = submitted {
                    db.send(DbCommand::RecordCommand {
                        pane_id,
                        command: line.text.clone(),
                        raw: line.text,
                        shell: pane.shell.db_key(),
                        cwd: pane
                            .working_directory
                            .as_ref()
                            .map(|p| p.display().to_string()),
                        exit_code: None,
                    });
                }
            }
        }
    }

    fn drain_db_events(&mut self) {
        while let Ok(event) = self.db_event_receiver.try_recv() {
            match event {
                DbEvent::ShortcutSuggestion { commands, count } => {
                    self.shortcuts.on_suggestion(commands, count);
                },
                DbEvent::Error(msg) => {
                    log::error!("db error: {msg}");
                    self.error_banner = Some(msg);
                },
            }
        }
    }

    /// THE injection gate for AI-suggested text: re-classifies at insertion
    /// time and refuses Tier-3 matches outright (defense in depth — the
    /// schema cannot express Tier 3, but if a rendered command ever matched
    /// a destructive pattern, it stops here). Never appends a newline.
    fn inject_suggestion(&mut self, command: &str) {
        if classify(command) == Tier::Destructive {
            log::error!(
                "refusing to inject Tier-3 command (suggestion blocked)"
            );
            self.error_banner = Some(
                "Suggestion blocked: it matched a destructive-command \
                 pattern"
                    .to_string(),
            );
            return;
        }
        self.insert_into_focused(command);
    }

    fn insert_into_focused(&mut self, text: &str) {
        if let Some(tab) = self.tabs.active_tab_mut() {
            let focused = tab.focused_pane;
            if let Some(pane) = tab.root.find_pane_mut(focused) {
                pane.insert_text(text);
            }
        }
    }

    /// Launch an intent request for the NL overlay phrase.
    fn submit_intent(
        &mut self,
        ctx: &egui::Context,
        request_id: u64,
        phrase: String,
    ) {
        let (shell, cwd) = self
            .tabs
            .active_tab_mut()
            .and_then(|tab| {
                let focused = tab.focused_pane;
                tab.root.find_pane_mut(focused).map(|p| {
                    (p.shell.clone(), p.working_directory.clone())
                })
            })
            .unwrap_or((ShellKind::PowerShell, None));

        let scan_dir = cwd
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let context = ProjectContext::scan(&scan_dir);

        client::spawn_request(
            IntentRequest {
                request_id,
                phrase,
                shell,
                cwd: cwd.map(|p| p.display().to_string()),
                context,
                // Stage (d): Tier 1 + Tier 2 intents. Tier-2 responses
                // route through the confirm dialog.
                scope: ToolsetScope::Tier1And2,
                model: client::DEFAULT_MODEL.to_string(),
            },
            self.intent_reply_sender.clone(),
            ctx.clone(),
        );
    }

    fn drain_intent_replies(&mut self) {
        // Responses render against the focused pane's shell.
        let shell = self
            .tabs
            .active_tab_mut()
            .and_then(|tab| {
                let focused = tab.focused_pane;
                tab.root.find_pane_mut(focused).map(|p| p.shell.clone())
            })
            .unwrap_or(ShellKind::PowerShell);
        while let Ok(response) = self.intent_reply_receiver.try_recv() {
            self.nl
                .on_response(response, |intent| render::render(intent, &shell));
        }
    }

    fn handle_hotkeys(&mut self, ctx: &egui::Context) {
        let (
            new_tab,
            close_tab,
            split_h,
            split_v,
            toggle_search,
            next_tab,
            toggle_history,
            toggle_shortcuts,
            toggle_nl,
        ) = ctx.input_mut(|i| {
            (
                i.consume_key(
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    egui::Key::T,
                ),
                i.consume_key(
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    egui::Key::W,
                ),
                i.consume_key(
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    egui::Key::D,
                ),
                i.consume_key(
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    egui::Key::E,
                ),
                i.consume_key(
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    egui::Key::F,
                ),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::Tab),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::R),
                i.consume_key(
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    egui::Key::S,
                ),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::K),
            )
        });

        if new_tab {
            self.tabs.open_tab(
                ctx.clone(),
                self.pty_event_sender.clone(),
                ShellKind::PowerShell,
            );
        }
        if close_tab {
            if let Some(id) = self.tabs.active_tab {
                self.tabs.close_tab(id);
            }
        }
        if split_h {
            self.tabs.split_focused(
                ctx.clone(),
                self.pty_event_sender.clone(),
                SplitDir::Horizontal,
            );
        }
        if split_v {
            self.tabs.split_focused(
                ctx.clone(),
                self.pty_event_sender.clone(),
                SplitDir::Vertical,
            );
        }
        if toggle_search {
            self.search.toggle();
        }
        if toggle_history {
            self.history.toggle();
        }
        if toggle_nl {
            self.nl.toggle();
        }
        if toggle_shortcuts {
            if let Some(db) = &self.db {
                let db = DbHandle {
                    sender: db.sender.clone(),
                };
                self.shortcuts.toggle_palette(&db);
            }
        }
        if next_tab {
            let keys: Vec<_> = self.tabs.tabs.keys().copied().collect();
            if let (Some(active), false) =
                (self.tabs.active_tab, keys.is_empty())
            {
                let pos = keys.iter().position(|k| *k == active).unwrap_or(0);
                self.tabs.active_tab = Some(keys[(pos + 1) % keys.len()]);
            }
        }
    }

    fn show_tab_bar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mut close_requested = None;
            let mut activate = None;
            for (id, tab) in self.tabs.tabs.iter() {
                let selected = self.tabs.active_tab == Some(*id);
                let label = egui::RichText::new(format!("  {}  ", tab.title));
                let label = if selected {
                    label.color(self.theme.accent)
                } else {
                    label
                };
                let response = ui.selectable_label(selected, label);
                if response.clicked() {
                    activate = Some(*id);
                }
                if response.middle_clicked() {
                    close_requested = Some(*id);
                }
            }
            if let Some(id) = activate {
                self.tabs.active_tab = Some(id);
            }
            if let Some(id) = close_requested {
                self.tabs.close_tab(id);
            }

            // New-tab button with shell picker.
            let plus = ui.button("＋");
            if plus.clicked() {
                self.shell_menu_open = !self.shell_menu_open;
            }
            let popup_id = ui.make_persistent_id("shell_picker");
            if self.shell_menu_open {
                egui::Area::new(popup_id)
                    .order(egui::Order::Foreground)
                    .fixed_pos(plus.rect.left_bottom())
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            for shell in self.available_shells.clone() {
                                if ui.button(shell.label()).clicked() {
                                    self.tabs.open_tab(
                                        ctx.clone(),
                                        self.pty_event_sender.clone(),
                                        shell,
                                    );
                                    self.shell_menu_open = false;
                                }
                            }
                        });
                    });
                // Click elsewhere closes the picker.
                if ctx.input(|i| i.pointer.any_click())
                    && !plus.clicked()
                    && !ctx.rect_contains_pointer(
                        egui::LayerId::new(
                            egui::Order::Foreground,
                            popup_id,
                        ),
                        egui::Rect::EVERYTHING,
                    )
                {
                    // Keep simple: close on Escape instead of geometry games.
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.shell_menu_open = false;
                }
            }

            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.label(
                        egui::RichText::new("🦀 Rusty")
                            .color(self.theme.accent)
                            .strong(),
                    );
                    if ui
                        .button("⚙")
                        .on_hover_text("Shell integration setup")
                        .clicked()
                    {
                        self.integration.open = !self.integration.open;
                    }
                },
            );
        });
    }

    fn run_search(&mut self, forward: bool) {
        let query = self.search.query.clone();
        if let Some(tab) = self.tabs.active_tab_mut() {
            let focused = tab.focused_pane;
            if let Some(pane) = tab.root.find_pane_mut(focused) {
                let count = pane.backend.search_scroll(&query, forward);
                self.search.match_count = Some(count);
            }
        }
    }
}

impl eframe::App for RustyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Window close requested → persist the session before exit.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.maybe_save_session();
        }

        self.drain_pty_events();
        self.drain_db_events();
        self.drain_intent_replies();
        // Search bar and NL overlay hotkeys are handled before the terminal
        // widget can swallow keystrokes.
        self.handle_hotkeys(&ctx);

        // Ctrl+K natural-language overlay. Suggested commands are only
        // ever inserted (never executed) and pass the injection gate.
        match self.nl.show(&ctx, self.theme.accent) {
            OverlayAction::Submit { request_id, phrase } => {
                self.submit_intent(&ctx, request_id, phrase);
            },
            OverlayAction::InsertTier1(command) => {
                self.nl.open = false;
                self.nl.suggestion = None;
                self.inject_suggestion(&command);
            },
            OverlayAction::ConfirmTier2 {
                command,
                intent_name,
            } => {
                self.nl.open = false;
                self.nl.suggestion = None;
                // Injection-time backstop applies here too — the confirm
                // dialog only ever sees non-destructive commands.
                if classify(&command) == Tier::Destructive {
                    log::error!(
                        "refusing to confirm a Tier-3 command (blocked)"
                    );
                    self.error_banner = Some(
                        "Suggestion blocked: matched a destructive pattern"
                            .to_string(),
                    );
                } else {
                    self.confirm.request(command, intent_name, Tier::Idempotent);
                }
            },
            OverlayAction::Close => {
                self.nl.open = false;
            },
            OverlayAction::None => {},
        }

        // Tier-2 confirmation dialog.
        match self.confirm.show(&ctx, self.theme.accent) {
            ConfirmOutcome::Run { command, token } => {
                if classify(&command) == Tier::Destructive {
                    log::error!(
                        "final backstop caught a Tier-3 command at Run"
                    );
                    self.error_banner = Some(
                        "Blocked at final safety check".to_string(),
                    );
                    // Token is dropped without executing.
                    let _ = token;
                } else if let Some(tab) = self.tabs.active_tab_mut() {
                    let focused = tab.focused_pane;
                    if let Some(pane) = tab.root.find_pane_mut(focused) {
                        pane.run_confirmed(&command, token);
                    }
                }
            },
            ConfirmOutcome::InsertOnly(command) => {
                self.inject_suggestion(&command);
            },
            ConfirmOutcome::Cancelled | ConfirmOutcome::None => {},
        }

        // Learning-engine overlays (history palette, shortcut suggestion,
        // shortcut palette). Insertion never carries a newline.
        if let Some(db) = &self.db {
            let db = DbHandle {
                sender: db.sender.clone(),
            };
            match self.history.show(&ctx, &db) {
                PaletteAction::Insert(text) => {
                    self.history.open = false;
                    self.insert_into_focused(&text);
                },
                PaletteAction::Close => self.history.open = false,
                PaletteAction::None => {},
            }
            self.shortcuts.show_suggestion(&ctx, &db, self.theme.accent);
            match self.shortcuts.show_palette(&ctx, &db) {
                ShortcutAction::InsertStep(text) => {
                    self.insert_into_focused(&text);
                },
                ShortcutAction::None => {},
            }
        }
        // Theme picker lives at the top of the settings dialog.
        let mut theme_changed = false;
        self.integration.show(
            &ctx,
            &mut self.theme,
            &mut theme_changed,
        );
        if theme_changed {
            self.theme.apply_chrome(&ctx);
        }

        if let Some(banner) = self.error_banner.clone() {
            egui::Panel::bottom("error_banner").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::LIGHT_RED, &banner);
                    if ui.small_button("✕").clicked() {
                        self.error_banner = None;
                    }
                });
            });
        }

        egui::Panel::top("tab_bar")
            .frame(
                egui::Frame::default()
                    .fill(self.theme.tab_bar_bg)
                    .inner_margin(egui::Margin::symmetric(6, 4)),
            )
            .show(ui, |ui| {
                let ctx = ui.ctx().clone();
                self.show_tab_bar(&ctx, ui);
            });

        if self.search.open {
            let mut action = SearchAction::None;
            egui::Panel::top("search_bar").show(ui, |ui| {
                action = self.search.show(ui);
            });
            match action {
                SearchAction::Search { forward } => self.run_search(forward),
                SearchAction::Close => {
                    self.search.open = false;
                    self.search.match_count = None;
                },
                SearchAction::None => {},
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(egui::Color32::from_rgb(
                0x18, 0x18, 0x18,
            )))
            .show(ui, |ui| {
                let rect = ui.available_rect_before_wrap();
                if let Some(tab) = self.tabs.active_tab_mut() {
                    let focused = tab.focused_pane;
                    if let Some(clicked) = panes::show_pane_tree(
                        ui,
                        &mut tab.root,
                        focused,
                        &self.theme,
                        rect,
                    ) {
                        tab.focused_pane = clicked;
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new(
                                "No open tabs — Ctrl+Shift+T for a new one",
                            )
                            .color(egui::Color32::GRAY),
                        );
                    });
                }
            });
    }
}
