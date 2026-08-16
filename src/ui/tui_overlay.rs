//! Ratatui ANSI in-terminal overlay panel (Ctrl+Shift+R).
//! Tabs: Settings, AI Chat, 3-Way History. Fully wrappable text UI.

use std::io::{self, stdout};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
    Terminal,
};

use crate::intent::api_key;
use crate::intent::local_model;
use crate::intent::render;
use crate::learning::db;
use crate::terminal::shell::ShellKind;

pub fn run_tui_overlay() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let res = main_tui_loop(&mut terminal);

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

#[derive(PartialEq)]
enum ActiveTab {
    Settings = 0,
    Chat = 1,
    History = 2,
}

fn main_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut active_tab = ActiveTab::Settings;
    let mut chat_input = String::new();
    let mut chat_messages: Vec<(String, String)> = Vec::new();

    let conn = db::open_default_db().ok();
    if let Some(c) = &conn {
        if let Ok(hist) = db::get_chat_history(c, 50) {
            for m in hist {
                chat_messages.push((m.role, m.content));
            }
        }
    }

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),  // Header / Tabs
                    Constraint::Min(12),    // Main Content
                    Constraint::Length(3),  // Footer Navigation
                ].as_ref())
                .split(f.area());

            // Tab bar header
            let titles = vec![
                Line::from(vec![Span::styled(" [1] ⚙ Settings ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
                Line::from(vec![Span::styled(" [2] 💬 AI Chat ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))]),
                Line::from(vec![Span::styled(" [3] 📜 3-Way History ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            ];

            let tabs = Tabs::new(titles)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(Span::styled(" 🦀 RUSTY TERMINAL AI OVERLAY ", Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD)))
                )
                .select(match active_tab {
                    ActiveTab::Settings => 0,
                    ActiveTab::Chat => 1,
                    ActiveTab::History => 2,
                })
                .style(Style::default().fg(Color::DarkGray))
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                );
            f.render_widget(tabs, chunks[0]);

            match active_tab {
                ActiveTab::Settings => {
                    let has_key = api_key::is_configured();
                    let local_present = local_model::is_local_model_present();

                    let key_status_span = if has_key {
                        Span::styled("Encrypted DPAPI Key Configured ✅", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled("No Cloud Key (Using Local Offline GGUF Engine)", Style::default().fg(Color::Yellow))
                    };

                    let model_status_span = if local_present {
                        Span::styled("Qwen2.5-Coder 0.5B GGUF Model Ready (~398 MB) ✅", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled("Model Not Pulled (Run 'rusty-cli download-model' to fetch)", Style::default().fg(Color::Red))
                    };

                    let settings_lines = vec![
                        Line::from(vec![
                            Span::styled("AI ENGINE MODE: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                            Span::raw("Offline-First Local GGUF Engine (Qwen2.5-Coder 0.5B)"),
                        ]),
                        Line::from(vec![
                            Span::styled("MODEL STATUS:   ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                            model_status_span,
                        ]),
                        Line::from(vec![
                            Span::styled("CLOUD API KEY:  ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                            key_status_span,
                        ]),
                        Line::from(""),
                        Line::from(Span::styled("COMMAND SAFETY TIERS:", Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD))),
                        Line::from(vec![
                            Span::styled("  • Tier 1 (Read-Only):  ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                            Span::raw("Auto-rendered & directly inserted into line (ls, git status, git log)"),
                        ]),
                        Line::from(vec![
                            Span::styled("  • Tier 2 (Idempotent): ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                            Span::raw("Requires interactive [Y/n] confirmation (git pull, docker up, mkdir)"),
                        ]),
                        Line::from(vec![
                            Span::styled("  • Tier 3 (Destructive):", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                            Span::raw("Blocked outright by safety gate (rm -rf, drop database, disk format)"),
                        ]),
                        Line::from(""),
                        Line::from(Span::styled("QUICK COMMANDS:", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))),
                        Line::from("  rusty-cli download-model   -> Downloads ~398MB local offline model"),
                        Line::from("  rusty-cli config set-key   -> Saves Anthropic / OpenRouter API Key"),
                        Line::from("  rusty-cli config delete-key-> Switches back to local offline model"),
                    ];

                    let p = Paragraph::new(settings_lines)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(Span::styled(" System & AI Engine Configuration ", Style::default().fg(Color::Cyan)))
                        )
                        .wrap(Wrap { trim: false });
                    f.render_widget(p, chunks[1]);
                },
                ActiveTab::Chat => {
                    let inner_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(6), Constraint::Length(3)].as_ref())
                        .split(chunks[1]);

                    let mut chat_lines = Vec::new();
                    for (role, msg) in &chat_messages {
                        if role == "user" {
                            chat_lines.push(Line::from(vec![
                                Span::styled(" You ❯ ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                                Span::styled(msg, Style::default().fg(Color::White)),
                            ]));
                        } else {
                            chat_lines.push(Line::from(vec![
                                Span::styled(" 🤖 Rusty ❯ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                                Span::styled(msg, Style::default().fg(Color::LightCyan)),
                            ]));
                        }
                        chat_lines.push(Line::from("")); // Blank line spacing
                    }

                    let p_chat = Paragraph::new(chat_lines)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(Span::styled(" AI Conversation History ", Style::default().fg(Color::Green)))
                        )
                        .wrap(Wrap { trim: false });
                    f.render_widget(p_chat, inner_chunks[0]);

                    let input_p = Paragraph::new(Line::from(vec![
                        Span::styled(" ❯ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(&chat_input),
                        Span::styled("█", Style::default().fg(Color::Cyan)),
                    ]))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(Span::styled(" Type your question and press Enter ", Style::default().fg(Color::Yellow)))
                    )
                    .wrap(Wrap { trim: false });
                    f.render_widget(input_p, inner_chunks[1]);
                },
                ActiveTab::History => {
                    let hist_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Percentage(34),
                            Constraint::Percentage(33),
                            Constraint::Percentage(33),
                        ].as_ref())
                        .split(chunks[1]);

                    let mut sug_lines = vec![];
                    let mut acc_lines = vec![];
                    let mut rej_lines = vec![];

                    if let Some(c) = &conn {
                        if let Ok(sug) = db::get_ai_suggestions(c, 30) {
                            for s in sug {
                                sug_lines.push(Line::from(vec![
                                    Span::styled(format!("[T{}] ", s.tier), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                                    Span::raw(format!("{} -> {}", s.phrase, s.rendered_cmd)),
                                ]));
                                sug_lines.push(Line::from(""));
                            }
                        }
                        if let Ok(acc) = db::get_ai_accepted(c, 30) {
                            for a in acc {
                                acc_lines.push(Line::from(vec![
                                    Span::styled("[Executed] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                                    Span::raw(a.rendered_cmd),
                                ]));
                                acc_lines.push(Line::from(""));
                            }
                        }
                        if let Ok(rej) = db::get_ai_rejected(c, 30) {
                            for r in rej {
                                rej_lines.push(Line::from(vec![
                                    Span::styled("[Rejected] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                                    Span::raw(r.rendered_cmd),
                                ]));
                                rej_lines.push(Line::from(""));
                            }
                        }
                    }

                    let p_sug = Paragraph::new(sug_lines)
                        .block(Block::default().borders(Borders::ALL).title(Span::styled(" 💡 Suggested ", Style::default().fg(Color::Cyan))))
                        .wrap(Wrap { trim: false });
                    let p_acc = Paragraph::new(acc_lines)
                        .block(Block::default().borders(Borders::ALL).title(Span::styled(" ✅ Accepted ", Style::default().fg(Color::Green))))
                        .wrap(Wrap { trim: false });
                    let p_rej = Paragraph::new(rej_lines)
                        .block(Block::default().borders(Borders::ALL).title(Span::styled(" ❌ Rejected/Ignored ", Style::default().fg(Color::Red))))
                        .wrap(Wrap { trim: false });

                    f.render_widget(p_sug, hist_chunks[0]);
                    f.render_widget(p_acc, hist_chunks[1]);
                    f.render_widget(p_rej, hist_chunks[2]);
                },
            }

            // Footer info bar
            let footer = Paragraph::new(Line::from(vec![
                Span::styled(" CONTROLS: ", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(" Press [1] Settings  [2] AI Chat  [3] History  |  Tab: Cycle  |  Esc/q: Return to Terminal ", Style::default().fg(Color::White).bg(Color::DarkGray)),
            ]));
            f.render_widget(footer, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Windows key event filter: only handle key press down events
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Esc => break,
                    KeyCode::Char('q') if active_tab != ActiveTab::Chat || chat_input.is_empty() => break,
                    KeyCode::Char('1') if active_tab != ActiveTab::Chat => active_tab = ActiveTab::Settings,
                    KeyCode::Char('2') if active_tab != ActiveTab::Chat => active_tab = ActiveTab::Chat,
                    KeyCode::Char('3') if active_tab != ActiveTab::Chat => active_tab = ActiveTab::History,
                    KeyCode::Tab => {
                        active_tab = match active_tab {
                            ActiveTab::Settings => ActiveTab::Chat,
                            ActiveTab::Chat => ActiveTab::History,
                            ActiveTab::History => ActiveTab::Settings,
                        };
                    },
                    KeyCode::Char(c) if active_tab == ActiveTab::Chat => {
                        chat_input.push(c);
                    },
                    KeyCode::Backspace if active_tab == ActiveTab::Chat => {
                        chat_input.pop();
                    },
                    KeyCode::Enter if active_tab == ActiveTab::Chat => {
                        if !chat_input.trim().is_empty() {
                            let msg = chat_input.trim().to_string();
                            chat_messages.push(("user".to_string(), msg.clone()));
                            if let Some(c) = &conn {
                                let _ = db::save_chat_message(c, "user", &msg);
                            }
                            chat_input.clear();

                            let reply = if crate::intent::code_gen::is_code_gen_request(&msg) {
                                // 2b. Code-generation Agent ("I want to achieve X" -> file output)
                                let sample_code = format!(
                                    "# Generated code for: {msg}\n# Target execution: Manual (Not executed automatically)\n\ndef main():\n    print('Executing goal: {}')\n\nif __name__ == '__main__':\n    main()\n",
                                    msg.replace('\'', "\\'")
                                );
                                match crate::intent::code_gen::process_code_gen(&msg, &sample_code, None, false) {
                                    Ok(res) => res.status_message,
                                    Err(e) => format!("❌ Code generation failed: {e}"),
                                }
                            } else {
                                // Command Intent / Tier Safety Logic
                                match local_model::run_local_inference(&msg) {
                                    Ok(intent) => {
                                        let cmd = render::render(&intent, &ShellKind::PowerShell);
                                        let tier = crate::safety::tier_classifier::classify(&cmd);

                                        // Record in 3-way history database with tier
                                        if let Some(c) = &conn {
                                            let _ = db::record_ai_suggestion(c, &msg, &cmd, tier as u8, "local");
                                        }

                                        match tier {
                                            crate::safety::tier_classifier::Tier::ReadOnly => {
                                                format!("Command: {cmd}\n[Tier 1 · Read-Only — Executable]")
                                            },
                                            crate::safety::tier_classifier::Tier::Idempotent => {
                                                format!("Command: {cmd}\n[Tier 2 · Low Blast Radius — Requires Confirmation]")
                                            },
                                            crate::safety::tier_classifier::Tier::Destructive => {
                                                format!("Command: {cmd}\n[DESTRUCTIVE — reference only]\n(Structurally blocked from auto-insertion. Retype or manual copy required.)")
                                            },
                                        }
                                    },
                                    Err(_) => {
                                        format!("To accomplish '{msg}', run the command or use Ctrl+K for inline AI suggestions.")
                                    },
                                }
                            };

                            chat_messages.push(("assistant".to_string(), reply.clone()));
                            if let Some(c) = &conn {
                                let _ = db::save_chat_message(c, "assistant", &reply);
                            }
                        }
                    },
                    _ => {},
                }
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }
            }
        }
    }

    Ok(())
}
