//! Multi-option disambiguation resolver.
//!
//! When plain-English user input has multiple matching candidate targets
//! (e.g. "go to kt" matching `kt/`, `kt_backend/`, and `kt_frontend/`),
//! `resolve_candidates` returns a list of candidate actions to present as a
//! numbered selection prompt.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisambiguationCandidate {
    pub number: usize,
    pub command: String,
    pub description: String,
}

/// Resolve potential disambiguation candidates for a plain-English prompt.
pub fn resolve_candidates(prompt: &str, cwd: Option<&Path>) -> Vec<DisambiguationCandidate> {
    let lower = prompt.trim().to_lowercase();

    // 1. Directory Navigation: "go to <target>", "cd <target>", "navigate to <target>"
    if let Some(target) = extract_nav_target(&lower) {
        if !target.is_empty() {
            let matches = find_matching_directories(target, cwd);
            if matches.len() > 1 {
                return matches
                    .into_iter()
                    .enumerate()
                    .map(|(idx, dir_name)| DisambiguationCandidate {
                        number: idx + 1,
                        command: format!("cd {dir_name}/"),
                        description: format!("Navigate to {dir_name}/ directory"),
                    })
                    .collect();
            } else if matches.len() == 1 {
                let dir_name = &matches[0];
                return vec![DisambiguationCandidate {
                    number: 1,
                    command: format!("cd {dir_name}/"),
                    description: format!("Navigate to {dir_name}/ directory"),
                }];
            }
        }
    }

    // Default: single inferred intent candidate
    Vec::new()
}

fn extract_nav_target(prompt: &str) -> Option<&str> {
    if let Some(rest) = prompt.strip_prefix("go to ") {
        return Some(rest.trim());
    }
    if let Some(rest) = prompt.strip_prefix("navigate to ") {
        return Some(rest.trim());
    }
    if let Some(rest) = prompt.strip_prefix("cd ") {
        return Some(rest.trim());
    }
    if let Some(rest) = prompt.strip_prefix("open folder ") {
        return Some(rest.trim());
    }
    None
}

fn find_matching_directories(target: &str, cwd: Option<&Path>) -> Vec<String> {
    let base_dir = cwd.unwrap_or_else(|| Path::new("."));
    let mut matches = Vec::new();

    if let Ok(entries) = fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let lower_name = name.to_lowercase();
                    if lower_name.contains(target) {
                        matches.push(name);
                    }
                }
            }
        }
    }

    matches.sort();
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nav_target() {
        assert_eq!(extract_nav_target("go to kt"), Some("kt"));
        assert_eq!(extract_nav_target("navigate to src"), Some("src"));
        assert_eq!(extract_nav_target("cd target"), Some("target"));
    }

    #[test]
    fn matches_directories() {
        let temp_dir = std::env::temp_dir().join("rusty_test_disambiguation");
        let _ = fs::create_dir_all(temp_dir.join("kt"));
        let _ = fs::create_dir_all(temp_dir.join("kt_backend"));
        let _ = fs::create_dir_all(temp_dir.join("kt_frontend"));

        let candidates = resolve_candidates("go to kt", Some(&temp_dir));
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].command, "cd kt/");
        assert_eq!(candidates[1].command, "cd kt_backend/");
        assert_eq!(candidates[2].command, "cd kt_frontend/");

        let _ = fs::remove_dir_all(temp_dir);
    }
}
