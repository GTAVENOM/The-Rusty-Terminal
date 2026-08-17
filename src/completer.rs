use std::path::{Path, PathBuf};
use rustyline::completion::{Completer, Pair};
use rustyline::{Context,Helper};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use std::fs;


pub struct FolderCompleter;

impl FolderCompleter {
    pub fn new() -> Self {
        Self
    }
}

impl Completer for FolderCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let mut candidates = Vec::new();
        let line_up_to_pos = &line[..pos];
        let trimmed_line = line_up_to_pos.trim_start();

        // Special case: typing "cd" without a trailing space completes to "cd "
        if trimmed_line.to_lowercase() == "cd" {
            let start_idx = line_up_to_pos.len() - trimmed_line.len();
            candidates.push(Pair {
                display: "cd ".to_string(),
                replacement: "cd ".to_string(),
            });
            return Ok((start_idx, candidates));
        }

        // Find the start index of the current word at cursor
        let word_start_idx = match line_up_to_pos.rfind(|c: char| c.is_whitespace()) {
            Some(idx) => idx + 1,
            None => 0,
        };

        let word = &line_up_to_pos[word_start_idx..];

        // Check if completion should trigger:
        // 1) The word contains slash '/', backslash '\', or tilde '~'
        // 2) Or line starts with a command prefix (e.g. "cd ", "open ", "cat ", "ls ")
        let is_path_pattern = word.contains('/') || word.contains('\\') || word.contains('~') || word.starts_with('.');
        let is_command_prefix = [
            "cd ", "open ", "show ", "goto ", "launch ", "ls ", "cat ", "vim ", "nano ",
        ]
        .iter()
        .any(|prefix| trimmed_line.to_lowercase().starts_with(prefix));

        if !is_path_pattern && !is_command_prefix {
            return Ok((word_start_idx, candidates));
        }

        let is_cd_cmd = trimmed_line.to_lowercase().starts_with("cd ");

        let (search_dir, file_prefix) = if word.is_empty() {
            (PathBuf::from("."), "".to_string())
        } else if word == "~" {
            (dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")), "".to_string())
        } else if word.starts_with("~/") {
            let relative = &word[2..];
            if relative.ends_with('/') || relative.ends_with('\\') {
                let expanded = dirs::home_dir().map(|mut h| { h.push(relative); h }).unwrap_or_else(|| PathBuf::from(word));
                (expanded, "".to_string())
            } else {
                let path = Path::new(relative);
                let parent = path.parent().unwrap_or_else(|| Path::new(""));
                let parent_str = parent.to_str().unwrap_or("");
                let expanded = dirs::home_dir().map(|mut h| {
                    if !parent_str.is_empty() {
                        h.push(parent_str);
                    }
                    h
                }).unwrap_or_else(|| PathBuf::from(word));
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                (expanded, file_name)
            }
        } else if word.ends_with('/') || word.ends_with('\\') {
            (PathBuf::from(word), "".to_string())
        } else {
            let path = Path::new(word);
            let parent = path.parent().unwrap_or_else(|| Path::new(""));
            let parent_str = parent.to_str().unwrap_or("");
            let search_path = if parent_str.is_empty() {
                PathBuf::from(".")
            } else {
                PathBuf::from(parent_str)
            };
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            (search_path, file_name)
        };

        if let Ok(entries) = fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    let is_dir = metadata.is_dir();
                    // For "cd", only suggest directories
                    if is_cd_cmd && !is_dir {
                        continue;
                    }
                    if let Some(name_str) = entry.file_name().to_str() {
                        // Filter hidden files unless explicitly requested with '.'
                        if name_str.starts_with('.') && !file_prefix.starts_with('.') {
                            continue;
                        }
                        if name_str.to_lowercase().starts_with(&file_prefix.to_lowercase()) {
                            let typed_parent = if word == "~" {
                                "~/".to_string()
                            } else if word.ends_with('/') || word.ends_with('\\') {
                                word.to_string()
                            } else if let Some(last_slash) = word.rfind('/') {
                                word[..=last_slash].to_string()
                            } else if let Some(last_slash) = word.rfind('\\') {
                                word[..=last_slash].to_string()
                            } else {
                                "".to_string()
                            };

                            let suffix = if is_dir { "/" } else { "" };
                            let display_name = format!("{}{}", name_str, suffix);
                            let replacement = format!("{}{}{}", typed_parent, name_str, suffix);

                            candidates.push(Pair {
                                display: display_name,
                                replacement,
                            });
                        }
                    }
                }
            }
        }

        if candidates.is_empty() && !file_prefix.is_empty() {
            use fuzzy_matcher::skim::SkimMatcherV2;
            use fuzzy_matcher::FuzzyMatcher;
            let matcher = SkimMatcherV2::default();

            if let Ok(entries) = fs::read_dir(&search_dir) {
                let mut scored_entries = Vec::new();
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        let is_dir = metadata.is_dir();
                        if is_cd_cmd && !is_dir {
                            continue;
                        }
                        if let Some(name_str) = entry.file_name().to_str() {
                            if name_str.starts_with('.') && !file_prefix.starts_with('.') {
                                continue;
                            }
                            if let Some(score) = matcher.fuzzy_match(name_str, &file_prefix) {
                                if score > 0 {
                                    scored_entries.push((score, name_str.to_string(), is_dir));
                                }
                            }
                        }
                    }
                }

                scored_entries.sort_by(|a, b| b.0.cmp(&a.0));

                for (_score, name_str, is_dir) in scored_entries {
                    let typed_parent = if word == "~" {
                        "~/".to_string()
                    } else if word.ends_with('/') || word.ends_with('\\') {
                        word.to_string()
                    } else if let Some(last_slash) = word.rfind('/') {
                        word[..=last_slash].to_string()
                    } else if let Some(last_slash) = word.rfind('\\') {
                        word[..=last_slash].to_string()
                    } else {
                        "".to_string()
                    };

                    let suffix = if is_dir { "/" } else { "" };
                    let display_name = format!("{}{}", name_str, suffix);
                    let replacement = format!("{}{}{}", typed_parent, name_str, suffix);

                    candidates.push(Pair {
                        display: display_name,
                        replacement,
                    });
                }
            }
        }

        Ok((word_start_idx, candidates))
    }
}

impl Hinter for FolderCompleter {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        None
    }
}

impl Helper for FolderCompleter {}

impl Highlighter for FolderCompleter {}

impl Validator for FolderCompleter {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cd_completer() {
        let completer = FolderCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);

        // "cd" completes to "cd "
        let (pos, candidates) = completer.complete("cd", 2, &ctx).unwrap();
        assert_eq!(pos, 0);
        assert!(!candidates.is_empty());
        assert_eq!(candidates[0].replacement, "cd ");

        // "cd sr" completes to "src/"
        let (pos, candidates) = completer.complete("cd sr", 5, &ctx).unwrap();
        assert_eq!(pos, 3);
        let has_src = candidates.iter().any(|c| c.replacement == "src/");
        assert!(has_src, "Expected candidate 'src/'");
    }

    #[test]
    fn test_natural_language_path_completer() {
        let completer = FolderCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);

        // "list files in src/ma" completes "src/ma" to "src/main.rs"
        let (pos, candidates) = completer.complete("list files in src/ma", 20, &ctx).unwrap();
        assert_eq!(pos, 14);
        let has_main = candidates.iter().any(|c| c.replacement == "src/main.rs");
        assert!(has_main, "Expected candidate 'src/main.rs'");
    }

    #[test]
    fn test_fuzzy_path_completer() {
        let completer = FolderCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);

        // "cd sc" (fuzzy matching "src")
        let (pos, candidates) = completer.complete("cd sc", 5, &ctx).unwrap();
        assert_eq!(pos, 3);
        let has_src = candidates.iter().any(|c| c.replacement == "src/");
        assert!(has_src, "Expected candidate 'src/' via fuzzy match");
    }
}