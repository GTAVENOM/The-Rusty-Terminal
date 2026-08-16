use std::io::{self, Write};
use std::process;

use rusty_terminal::context::scanner::ProjectContext;
use rusty_terminal::intent::api_key;
use rusty_terminal::intent::client::{self, IntentRequest};
use rusty_terminal::intent::local_model;
use rusty_terminal::intent::render;
use rusty_terminal::intent::schema::ToolsetScope;
use rusty_terminal::learning::db::{self, default_db_path};
use rusty_terminal::safety::tier_classifier::{classify, Tier};
use rusty_terminal::terminal::shell::ShellKind;
use rusty_terminal::ui::tui_overlay;

fn print_usage() {
    eprintln!("🦀 Rusty CLI — AI Terminal Assistant & Wrapper");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  rusty-cli \"<natural language prompt>\"");
    eprintln!("  rusty-cli suggest \"<natural language prompt>\"");
    eprintln!("  rusty-cli inline \"<buffer or query>\"");
    eprintln!("  rusty-cli overlay                       (Opens ANSI TUI Panel: Ctrl+Shift+R)");
    eprintln!("  rusty-cli download-model                (Pulls Qwen2.5-Coder-1.5B local GGUF model: <1GB)");
    eprintln!("  rusty-cli config set-key <API_KEY>      (Stores Anthropic/OpenRouter cloud API key)");
    eprintln!("  rusty-cli setup-ps                      (Registers PowerShell PSReadLine predictor)");
    eprintln!("  rusty-cli history [query]              (Displays 3-way history logs)");
    eprintln!("  rusty-cli clean-scratch                 (Cleans old chat-generated scratch files)");
    eprintln!("  rusty-cli error-help \"<cmd>\" <code> \"<err>\" (Matches error against known pattern list)");
    eprintln!("  rusty-cli uninstall                     (Removes binary, PATH, and shell hooks)");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("  rusty-cli \"show last 50 docker logs for api\"");
    eprintln!("  rusty-cli \"I want a Python script that renames files\"");
    eprintln!("  rusty-cli overlay");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "-h" | "--help" | "help" => {
            print_usage();
        },
        "overlay" => {
            if let Err(e) = tui_overlay::run_tui_overlay() {
                eprintln!("Error running TUI overlay: {e}");
                process::exit(1);
            }
        },
        "clean-scratch" => {
            match rusty_terminal::intent::code_gen::clean_scratch_directory(None) {
                Ok(count) => println!("🧹 Cleaned {count} old scratch file(s) from .rusty_scratch/"),
                Err(e) => eprintln!("❌ Failed to clean scratch directory: {e}"),
            }
        },
        "error-help" => {
            if args.len() < 5 {
                eprintln!("Usage: rusty-cli error-help \"<command>\" <exit_code> \"<stderr>\"");
                process::exit(1);
            }
            let cmd = &args[2];
            let code: i32 = args[3].parse().unwrap_or(1);
            let stderr = &args[4];
            if let Some(fix) = rusty_terminal::error_help::match_error(cmd, code, stderr) {
                println!("💡 [{}] {}", fix.category, fix.explanation);
                println!("   Suggested Fix: {}", fix.fix_command);
            } else {
                eprintln!("No known error pattern matched.");
            }
        },
        "download-model" => {
            if let Err(e) = local_model::download_model_if_missing() {
                eprintln!("❌ Model download failed: {e}");
                process::exit(1);
            } else {
                println!("✅ Local GGUF model is ready!");
            }
        },
        "uninstall" => {
            uninstall_rusty();
        },
        "website" => {
            let html_path = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("website")
                .join("index.html");
            if html_path.exists() {
                println!("🌐 Opening Rusty Terminal website: {}", html_path.display());
                let _ = std::process::Command::new("cmd")
                    .args(["/C", "start", "", &html_path.display().to_string()])
                    .spawn();
            } else {
                eprintln!("Website files not found at {}", html_path.display());
            }
        },

        "config" => {
            if args.len() >= 4 && args[2] == "set-key" {
                let key = &args[3];
                if let Err(e) = api_key::store_encrypted_key(key) {
                    eprintln!("❌ Failed to save encrypted API key: {e}");
                    process::exit(1);
                } else {
                    println!("✅ Anthropic API key stored securely (Windows DPAPI).");
                }
            } else if args.len() >= 3 && args[2] == "delete-key" {
                let _ = api_key::delete_stored_key();
                println!("✅ Stored API key removed. Switched back to local offline model.");
            } else {
                eprintln!("Usage: rusty-cli config set-key <KEY> | rusty-cli config delete-key");
                process::exit(1);
            }
        },

        "setup-ps" => {
            setup_powershell_profile();
        },
        "history" => {
            let query = if args.len() >= 3 {
                args[2..].join(" ")
            } else {
                String::new()
            };
            show_history(&query);
        },
        "inline" => {
            let prompt = if args.len() >= 3 {
                args[2..].join(" ")
            } else {
                String::new()
            };
            handle_suggest_flow(&prompt, true);
        },
        "suggest" => {
            if args.len() < 3 {
                eprintln!("Usage: rusty-cli suggest \"<natural language prompt>\"");
                process::exit(1);
            }
            let prompt = args[2..].join(" ");
            handle_suggest_flow(&prompt, false);
        },
        _prompt => {
            let full_prompt = args[1..].join(" ");
            handle_suggest_flow(&full_prompt, false);
        },
    }
}

fn handle_suggest_flow(prompt: &str, inline_mode: bool) {
    if prompt.trim().is_empty() {
        if inline_mode {
            eprint!("🦀 Rusty AI Prompt: ");
            io::stderr().flush().unwrap();
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
                process::exit(0);
            }
            return handle_suggest_flow(input.trim(), true);
        } else {
            eprintln!("Error: Please provide a natural language prompt.");
            process::exit(1);
        }
    }

    // Check 2b: Code generation agent goal
    if rusty_terminal::intent::code_gen::is_code_gen_request(prompt) {
        let sample_code = format!(
            "# Generated code for goal: {prompt}\n# Manual execution target\n\ndef main():\n    print('Executing: {}')\n\nif __name__ == '__main__':\n    main()\n",
            prompt.replace('\'', "\\'")
        );
        match rusty_terminal::intent::code_gen::process_code_gen(prompt, &sample_code, None, false) {
            Ok(res) => {
                if inline_mode {
                    eprintln!("{}", res.status_message);
                } else {
                    println!("{}", res.status_message);
                }
                return;
            },
            Err(e) => {
                eprintln!("❌ Code generation failed: {e}");
                process::exit(1);
            }
        }
    }

    let cwd = std::env::current_dir().ok();
    let scan_dir = cwd.clone().unwrap_or_else(|| std::path::PathBuf::from("."));

    // Check Multi-Option Disambiguation Candidate Resolution
    let candidates = rusty_terminal::intent::disambiguation::resolve_candidates(prompt, cwd.as_deref());
    if !candidates.is_empty() {
        let selected_cmd = if candidates.len() > 1 {
            eprintln!("\x1b[1;33m🔍 Multiple matching options found for \"{prompt}\":\x1b[0m");
            for c in &candidates {
                eprintln!("  \x1b[1;36m[{}]\x1b[0m \x1b[1;32m{}\x1b[0m \x1b[90m({})\x1b[0m", c.number, c.command, c.description);
            }
            eprint!("\x1b[1;33mSelect an option [1-{}]: \x1b[0m", candidates.len());
            io::stderr().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok() {
                let choice: usize = input.trim().parse().unwrap_or(1);
                if choice >= 1 && choice <= candidates.len() {
                    candidates[choice - 1].command.clone()
                } else {
                    candidates[0].command.clone()
                }
            } else {
                candidates[0].command.clone()
            }
        } else {
            candidates[0].command.clone()
        };

        let tier = classify(&selected_cmd);
        eprintln!("\x1b[1;32m✨ Executing: \x1b[1;36m{}\x1b[0m", selected_cmd);
        if let Ok(conn) = db::open_default_db() {
            if let Ok(s_id) = db::record_ai_suggestion(&conn, prompt, &selected_cmd, tier as u8, "disambiguation") {
                let _ = db::record_ai_accepted(&conn, s_id, Some(0));
            }
        }
        println!("{selected_cmd}");
        return;
    }

    let context = ProjectContext::scan(&scan_dir);
    let shell = detect_shell();

    let request = IntentRequest {
        request_id: 1,
        phrase: prompt.to_string(),
        shell: shell.clone(),
        cwd: cwd.map(|p| p.display().to_string()),
        context,
        scope: ToolsetScope::Tier1And2,
        model: client::DEFAULT_MODEL.to_string(),
    };

    // Determine provider & intent
    let (provider_name, intent_res) = if api_key::is_configured() {
        match client::run_request(&request) {
            Ok(intent) => ("cloud", Ok(intent)),
            Err(e) => {
                if !inline_mode {
                    eprintln!("⚠️ Cloud API error ({e}). Falling back to local offline model...");
                }
                ("local_fallback", local_model::run_local_inference(prompt))
            }
        }
    } else {
        ("local", local_model::run_local_inference(prompt))
    };

    match intent_res {
        Ok(intent) => {
            let rendered = render::render(&intent, &shell);
            let tier = classify(&rendered);

            // Record suggestion in 3-way history DB (including Tier 3)
            let suggestion_id = if let Ok(conn) = db::open_default_db() {
                db::record_ai_suggestion(&conn, prompt, &rendered, tier as u8, provider_name).ok()
            } else {
                None
            };

            match tier {
                Tier::ReadOnly => {
                    eprintln!("\x1b[1;32m✨ Executing Intent [{provider_name}]: \x1b[1;36m{}\x1b[0m", rendered);
                    if let (Some(s_id), Ok(conn)) = (suggestion_id, db::open_default_db()) {
                        let _ = db::record_ai_accepted(&conn, s_id, Some(0));
                    }
                    println!("{rendered}");
                },
                Tier::Idempotent => {
                    if inline_mode {
                        eprintln!("\x1b[1;32m✨ Executing Intent [{provider_name}]: \x1b[1;36m{}\x1b[0m", rendered);
                        println!("{rendered}");
                        return;
                    }
                    eprintln!("\x1b[1;33m⚠️  Tier 2 (Idempotent) Intent [{provider_name}]: [{}]\x1b[0m", intent.name());
                    eprintln!("   Command: \x1b[1;36m{}\x1b[0m", rendered);
                    eprint!("\x1b[1;33m   Execute/Insert command? [Y/n]: \x1b[0m");
                    io::stderr().flush().unwrap();

                    let mut choice = String::new();
                    if io::stdin().read_line(&mut choice).is_ok() {
                        let c = choice.trim().to_lowercase();
                        if c.is_empty() || c == "y" || c == "yes" {
                            if let (Some(s_id), Ok(conn)) = (suggestion_id, db::open_default_db()) {
                                let _ = db::record_ai_accepted(&conn, s_id, Some(0));
                            }
                            eprintln!("\x1b[1;32m✨ Executing: \x1b[1;36m{}\x1b[0m", rendered);
                            println!("{rendered}");
                        } else {
                            if let (Some(s_id), Ok(conn)) = (suggestion_id, db::open_default_db()) {
                                let _ = db::record_ai_rejected(&conn, s_id);
                            }
                            eprintln!("Cancelled.");
                            process::exit(1);
                        }
                    }
                },
                Tier::Destructive => {
                    // Logged to history table with tier 3, but structurally forbidden from being printed to stdout for live input insertion.
                    eprintln!("\x1b[1;31m🛑 [DESTRUCTIVE — reference only]\x1b[0m");
                    eprintln!("   Reference command: \x1b[1;31m{}\x1b[0m", rendered);
                    eprintln!("   (Structurally incapable of auto-insertion into live shell line. Retyping or manual copy required.)");
                    process::exit(1);
                },
            }
        },
        Err(err) => {
            eprintln!("❌ Intent processing failed: {err}");
            process::exit(1);
        },
    }
}

fn detect_shell() -> ShellKind {
    if let Ok(shell_var) = std::env::var("SHELL") {
        if shell_var.contains("bash") || shell_var.contains("zsh") {
            return ShellKind::Wsl("default".to_string());
        }
    }
    if std::env::var("PSModulePath").is_ok() {
        return ShellKind::PowerShell;
    }
    if std::env::var("ComSpec").is_ok() {
        return ShellKind::Cmd;
    }
    ShellKind::PowerShell
}

fn show_history(_query: &str) {
    let db_path = default_db_path();
    if !db_path.exists() {
        eprintln!("No history database found at {}", db_path.display());
        return;
    }
    if let Ok(conn) = db::open_default_db() {

        println!("📜 3-WAY AI HISTORY LOGS");
        println!("--------------------------------------------------");
        println!("1. AI SUGGESTIONS:");
        if let Ok(sug) = db::get_ai_suggestions(&conn, 10) {
            for s in sug {
                println!("   • [T{}] ({}) {} -> {}", s.tier, s.provider, s.phrase, s.rendered_cmd);
            }
        }
        println!("\n2. ACCEPTED COMMANDS:");
        if let Ok(acc) = db::get_ai_accepted(&conn, 10) {
            for a in acc {
                println!("   • [Exit {:?}] {}", a.exit_code, a.rendered_cmd);
            }
        }
        println!("\n3. REJECTED / IGNORED COMMANDS:");
        if let Ok(rej) = db::get_ai_rejected(&conn, 10) {
            for r in rej {
                println!("   • [Inferred] {}", r.rendered_cmd);
            }
        }
    }
}

fn setup_powershell_profile() {
    let ps_snippet = r#"
# --- Rusty Terminal Integration (Ctrl+V) ---
if (-not (Get-Command rusty-cli -ErrorAction SilentlyContinue)) {
    $rustyBin = "$env:USERPROFILE\.rusty\bin"
    if (Test-Path "$rustyBin\rusty-cli.exe") {
        $env:PATH += ";$rustyBin"
    } elseif (Test-Path "v:\RustyTerminal\target\debug\rusty-cli.exe") {
        $env:PATH += ";v:\RustyTerminal\target\debug"
    } elseif (Test-Path "v:\RustyTerminal\target\release\rusty-cli.exe") {
        $env:PATH += ";v:\RustyTerminal\target\release"
    }
}

Set-PSReadLineKeyHandler -Chord 'Ctrl+v' -ScriptBlock {
    $line = $null
    $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    
    $rustyExe = Get-Command rusty-cli -ErrorAction SilentlyContinue
    $cmd = $null
    if ($rustyExe) {
        $cmd = & $rustyExe inline $line
    } else {
        $fallback = "$env:USERPROFILE\.rusty\bin\rusty-cli.exe"
        if (-not (Test-Path $fallback)) {
            $fallback = "v:\RustyTerminal\target\debug\rusty-cli.exe"
        }
        if (Test-Path $fallback) {
            $cmd = & $fallback inline $line
        }
    }

    if ($cmd) {
        [Microsoft.PowerShell.PSConsoleReadLine]::Replace(0, $line.Length, $cmd.Trim())
    }
}
"#;

    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let profiles = vec![
        home.join("Documents").join("PowerShell").join("profile.ps1"),
        home.join("Documents").join("WindowsPowerShell").join("profile.ps1"),
        home.join("Documents").join("PowerShell").join("Microsoft.PowerShell_profile.ps1"),
        home.join("Documents").join("WindowsPowerShell").join("Microsoft.PowerShell_profile.ps1"),
        home.join("Documents").join("PowerShell").join("Microsoft.VSCode_profile.ps1"),
        home.join("Documents").join("WindowsPowerShell").join("Microsoft.VSCode_profile.ps1"),
    ];

    let mut configured = false;
    for profile_path in profiles {
        if let Some(parent) = profile_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let existing = std::fs::read_to_string(&profile_path).unwrap_or_default();
        if existing.contains("rusty-cli inline") {
            println!("✅ Profile already configured: {}", profile_path.display());
            configured = true;
            continue;
        }
        let mut file = match std::fs::OpenOptions::new().create(true).append(true).open(&profile_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Could not open profile {}: {e}", profile_path.display());
                continue;
            },
        };
        if let Err(e) = writeln!(file, "{ps_snippet}") {
            eprintln!("Failed to write to profile {}: {e}", profile_path.display());
        } else {
            println!("🎉 Added Ctrl+V PSReadLine keybinding to profile: {}", profile_path.display());
            configured = true;
        }
    }

    if configured {
        println!();
        println!("Reload your PowerShell profile to activate Ctrl+V:");
        println!("  . $PROFILE");
    }
}

fn uninstall_rusty() {
    println!("🗑️  Uninstalling Rusty Terminal & CLI Assistant...");
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let profiles = vec![
        home.join("Documents").join("PowerShell").join("profile.ps1"),
        home.join("Documents").join("WindowsPowerShell").join("profile.ps1"),
        home.join("Documents").join("PowerShell").join("Microsoft.PowerShell_profile.ps1"),
        home.join("Documents").join("WindowsPowerShell").join("Microsoft.PowerShell_profile.ps1"),
        home.join("Documents").join("PowerShell").join("Microsoft.VSCode_profile.ps1"),
        home.join("Documents").join("WindowsPowerShell").join("Microsoft.VSCode_profile.ps1"),
    ];

    for p in profiles {
        if p.exists() {
            if let Ok(content) = std::fs::read_to_string(&p) {
                let cleaned: Vec<&str> = content
                    .lines()
                    .filter(|l| !l.contains("rusty-cli") && !l.contains("Rusty Terminal"))
                    .collect();
                let _ = std::fs::write(&p, cleaned.join("\n"));
                println!("🧹 Removed integration snippet from {}", p.display());
            }
        }
    }

    let bin_dir = home.join(".rusty");
    if bin_dir.exists() {
        let _ = std::fs::remove_dir_all(bin_dir);
        println!("🧹 Removed binary directory %USERPROFILE%\\.rusty");
    }

    println!("✅ Uninstall complete! Restart your shell session.");
}
