//! Extensible pattern matcher for bounded error help.
//!
//! Maps non-zero exit codes or stderr output matching known error classes to
//! one-line inline fix suggestions (Tier 1 or Tier 2 commands, shown as literal
//! text, never auto-run).
//!
//! If stderr does not match a known pattern, `match_error` returns `None`
//! (Rusty says nothing; no LLM guessing).

use crate::safety::tier_classifier::{classify, Tier};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorHelpFix {
    /// The short description of the matched error class.
    pub category: &'static str,
    /// One-line explanation of the issue.
    pub explanation: String,
    /// Literal command fix suggested (Tier 1 or Tier 2 only).
    pub fix_command: String,
    /// Safety tier of the suggested command fix.
    pub tier: Tier,
}

struct PatternRule {
    category: &'static str,
    patterns: &'static [&'static str],
    fix_generator: fn(cmd: &str, stderr: &str) -> Option<(String, String)>,
}

const KNOWN_PATTERNS: &[PatternRule] = &[
    // 1. Command not found / Not recognized
    PatternRule {
        category: "Command Not Found",
        patterns: &[
            "is not recognized as an internal or external command",
            "command not found",
            "No such file or directory",
            "The term",
            "is not recognized as the name of a cmdlet",
        ],
        fix_generator: |cmd, stderr| {
            let missing_tool = extract_missing_tool(cmd, stderr);
            let fix = format!("winget install {missing_tool}");
            let exp = format!("Tool '{missing_tool}' is not installed or not in PATH.");
            Some((exp, fix))
        },
    },
    // 2. Permission denied / Access is denied
    PatternRule {
        category: "Permission Denied",
        patterns: &[
            "Access is denied",
            "Permission denied",
            "EACCES: permission denied",
            "Operation not permitted",
        ],
        fix_generator: |cmd, _stderr| {
            let fix = format!("Start-Process powershell -Verb RunAs -ArgumentList '-Command', '{cmd}'");
            let exp = "Operation requires elevated administrator privileges.".to_string();
            Some((exp, fix))
        },
    },
    // 3. Port already in use
    PatternRule {
        category: "Port Already In Use",
        patterns: &[
            "address already in use",
            "port already in use",
            "WSAEADDRINUSE",
            "EADDRINUSE",
            "Only one usage of each socket address",
        ],
        fix_generator: |_cmd, stderr| {
            let port = extract_port_number(stderr).unwrap_or(3000);
            let fix = format!("Get-NetTCPConnection -LocalPort {port}");
            let exp = format!("Port {port} is occupied by another process.");
            Some((exp, fix))
        },
    },
    // 4. Missing module / package / crate
    PatternRule {
        category: "Module / Package Not Found",
        patterns: &[
            "ModuleNotFoundError: No module named",
            "Cannot find module",
            "error[E0432]: unresolved import",
            "npm ERR! code MODULE_NOT_FOUND",
        ],
        fix_generator: |_cmd, stderr| {
            if let Some(pkg) = extract_python_module(stderr) {
                return Some((
                    format!("Python module '{pkg}' is missing."),
                    format!("pip install {pkg}"),
                ));
            }
            if let Some(pkg) = extract_npm_module(stderr) {
                return Some((
                    format!("npm package '{pkg}' is missing."),
                    format!("npm install {pkg}"),
                ));
            }
            if let Some(crate_name) = extract_cargo_crate(stderr) {
                return Some((
                    format!("Rust crate '{crate_name}' is missing."),
                    format!("cargo add {crate_name}"),
                ));
            }
            None
        },
    },
    // 5. Script syntax error
    PatternRule {
        category: "Script Syntax Error",
        patterns: &[
            "SyntaxError: invalid syntax",
            "Parse error",
            "syntax error near unexpected token",
            "Uncaught SyntaxError",
        ],
        fix_generator: |cmd, stderr| {
            let file = extract_script_filename(cmd, stderr).unwrap_or_else(|| "script".to_string());
            let line = extract_line_number(stderr);
            let line_str = line.map(|l| format!(" at line {l}")).unwrap_or_default();
            let exp = format!("Syntax error detected in {file}{line_str}.");
            let fix = format!("code -g {file}:{line_number}", line_number = line.unwrap_or(1));
            Some((exp, fix))
        },
    },
    // 6. Merge conflict
    PatternRule {
        category: "Git Merge Conflict",
        patterns: &[
            "Automatic merge failed; fix conflicts and then commit the result",
            "CONFLICT (content): Merge conflict in",
            "unmerged files",
        ],
        fix_generator: |_cmd, _stderr| {
            Some((
                "Git merge conflicts require resolution.".to_string(),
                "git status".to_string(),
            ))
        },
    },
    // 7. Dependency resolution failure
    PatternRule {
        category: "Dependency Resolution Failure",
        patterns: &[
            "npm ERR! code ERESOLVE",
            "Could not resolve dependency",
            "pip ERROR: No matching distribution found for",
        ],
        fix_generator: |cmd, stderr| {
            if stderr.contains("npm") || cmd.contains("npm") {
                Some((
                    "npm dependency conflict encountered.".to_string(),
                    "npm install --legacy-peer-deps".to_string(),
                ))
            } else if stderr.contains("pip") || cmd.contains("pip") {
                Some((
                    "pip package version conflict encountered.".to_string(),
                    "pip install --no-deps".to_string(),
                ))
            } else {
                None
            }
        },
    },
];

/// Match a command's stderr output and exit code against known error patterns.
/// Returns `Some(ErrorHelpFix)` if a pattern matches and the resulting fix is Tier 1 or Tier 2.
/// Returns `None` if stderr does not match any known pattern or if the fix is Tier 3.
pub fn match_error(cmd: &str, exit_code: i32, stderr: &str) -> Option<ErrorHelpFix> {
    if exit_code == 0 && stderr.trim().is_empty() {
        return None;
    }

    for rule in KNOWN_PATTERNS {
        let matches = rule.patterns.iter().any(|p| stderr.contains(p));
        if matches {
            if let Some((exp, fix_cmd)) = (rule.fix_generator)(cmd, stderr) {
                let tier = classify(&fix_cmd);
                // Sourced the same way as command suggestions: Tier 1/2 only, never Tier 3
                if tier == Tier::ReadOnly || tier == Tier::Idempotent {
                    return Some(ErrorHelpFix {
                        category: rule.category,
                        explanation: exp,
                        fix_command: fix_cmd,
                        tier,
                    });
                }
            }
        }
    }
    None
}

fn extract_missing_tool(cmd: &str, stderr: &str) -> String {
    let first_word = cmd.split_whitespace().next().unwrap_or(cmd);
    let base = first_word.trim_end_matches(".exe");
    if stderr.contains("The term '") {
        if let Some(start) = stderr.find("The term '") {
            let rest = &stderr[start + 10..];
            if let Some(end) = rest.find('\'') {
                return rest[..end].to_string();
            }
        }
    }
    base.to_string()
}

fn extract_port_number(stderr: &str) -> Option<u16> {
    let words: Vec<&str> = stderr.split(|c: char| !c.is_numeric()).collect();
    for word in words {
        if let Ok(port) = word.parse::<u16>() {
            if (1024..=65535).contains(&port) {
                return Some(port);
            }
        }
    }
    None
}

fn extract_python_module(stderr: &str) -> Option<String> {
    let marker = "No module named '";
    if let Some(idx) = stderr.find(marker) {
        let rest = &stderr[idx + marker.len()..];
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].split('.').next().unwrap_or(rest).to_string());
        }
    }
    None
}

fn extract_npm_module(stderr: &str) -> Option<String> {
    let marker = "Cannot find module '";
    if let Some(idx) = stderr.find(marker) {
        let rest = &stderr[idx + marker.len()..];
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn extract_cargo_crate(stderr: &str) -> Option<String> {
    let marker = "unresolved import `";
    if let Some(idx) = stderr.find(marker) {
        let rest = &stderr[idx + marker.len()..];
        if let Some(end) = rest.find('`') {
            return Some(rest[..end].split("::").next().unwrap_or(rest).to_string());
        }
    }
    None
}

fn extract_script_filename(cmd: &str, stderr: &str) -> Option<String> {
    if let Some(file_idx) = stderr.find("File \"") {
        let rest = &stderr[file_idx + 6..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    for tok in cmd.split_whitespace() {
        if tok.ends_with(".py") || tok.ends_with(".js") || tok.ends_with(".rs") || tok.ends_with(".ps1") {
            return Some(tok.to_string());
        }
    }
    None
}

fn extract_line_number(stderr: &str) -> Option<u32> {
    let marker = "line ";
    if let Some(idx) = stderr.find(marker) {
        let rest = &stderr[idx + marker.len()..];
        let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        return num_str.parse().ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_missing_python_module() {
        let stderr = "Traceback (most recent call last):\n  File \"app.py\", line 1\nModuleNotFoundError: No module named 'requests'";
        let res = match_error("python app.py", 1, stderr).unwrap();
        assert_eq!(res.category, "Module / Package Not Found");
        assert_eq!(res.fix_command, "pip install requests");
        assert_eq!(res.tier, Tier::Idempotent);
    }

    #[test]
    fn matches_port_in_use() {
        let stderr = "Error: listen EADDRINUSE: address already in use :::8080";
        let res = match_error("node server.js", 1, stderr).unwrap();
        assert_eq!(res.category, "Port Already In Use");
        assert_eq!(res.fix_command, "Get-NetTCPConnection -LocalPort 8080");
        assert_eq!(res.tier, Tier::ReadOnly);
    }

    #[test]
    fn matches_command_not_found() {
        let stderr = "'docker' is not recognized as an internal or external command, operable program or batch file.";
        let res = match_error("docker ps", 1, stderr).unwrap();
        assert_eq!(res.category, "Command Not Found");
        assert_eq!(res.fix_command, "winget install docker");
    }

    #[test]
    fn unknown_stderr_returns_none() {
        let stderr = "Custom unhandled domain error: state machine crashed";
        assert!(match_error("my-app", 1, stderr).is_none());
    }

    #[test]
    fn zero_exit_code_clean_stderr_returns_none() {
        assert!(match_error("ls", 0, "").is_none());
    }
}
