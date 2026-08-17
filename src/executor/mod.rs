use std::error::Error;
use std::io::Write;
use crate::actions::CommandAction;

pub struct SystemExecutor;
impl SystemExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self, action: &CommandAction) -> Result<(), Box<dyn Error>> {
        match action {
            CommandAction::OpenApp { name } => self.open_app(name),
            CommandAction::OpenFolder { path } => self.open_folder(path),
            CommandAction::ExecuteSystemCommand { command, args } => {
                if command == "cd" {
                    let target = args.first().map(|s| s.as_str()).unwrap_or("~");
                    self.change_dir(target)
                } else if command == "clear" || command == "cls" {
                    self.clear_screen()
                } else {
                    self.run_command(command, args)
                }
            }
            CommandAction::ChangeDirectory { path } => {
                let path_str = path.to_string_lossy();
                self.change_dir(&path_str)
            }
            CommandAction::ClearScreen => self.clear_screen(),
            CommandAction::Exit => Ok(()),
            CommandAction::Open { .. } => Ok(()),
            CommandAction::Unknown => {
                println!("Unknown command.");
                Ok(())
            }
        }
    }

    fn change_dir(&self, target_str: &str) -> Result<(), Box<dyn Error>> {
        let clean_target = target_str.trim().replace(['"', '\''], "");
        let candidates = crate::fuzzy::resolve_fuzzy_candidates(&clean_target);

        if candidates.is_empty() {
            eprintln!("cd: {}: No such file or directory", target_str);
            return Ok(());
        }

        let is_home = crate::fuzzy::is_home_alias(target_str);

        // Ambiguity check:
        // If target is "home" and CWD contains a matching folder besides ~
        // Or top 2 candidates have scores close to each other
        let is_ambiguous = if candidates.len() > 1 {
            if is_home {
                candidates.iter().any(|c| c.display.starts_with("./"))
            } else {
                let top_score = candidates[0].score;
                let second_score = candidates[1].score;
                top_score >= 5000 && (top_score - second_score) <= 2500
            }
        } else {
            false
        };

        if is_ambiguous {
            println!("\nMultiple directory matches found for '{}':", target_str);
            for (idx, cand) in candidates.iter().enumerate() {
                println!("  [{}] {}", idx + 1, cand.display);
            }
            print!("Select directory (1-{}, or press Enter to cancel): ", candidates.len());
            std::io::stdout().flush()?;

            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_ok() {
                let trimmed = input.trim();
                if let Ok(choice) = trimmed.parse::<usize>() {
                    if choice >= 1 && choice <= candidates.len() {
                        let selected_path = &candidates[choice - 1].path;
                        if let Err(e) = std::env::set_current_dir(selected_path) {
                            eprintln!("cd: {}: {}", selected_path.display(), e);
                        }
                        return Ok(());
                    }
                }
            }
            println!("Navigation cancelled.");
            return Ok(());
        }

        let target_path = &candidates[0].path;
        if let Err(e) = std::env::set_current_dir(target_path) {
            eprintln!("cd: {}: {}", target_path.display(), e);
        }

        Ok(())
    }

    fn clear_screen(&self) -> Result<(), Box<dyn Error>> {
        print!("\x1B[2J\x1B[1H");
        std::io::stdout().flush()?;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos;