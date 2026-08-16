//! Code generation agent for natural-language goals ("I want to achieve X").
//!
//! Generates code via LLM, writes output to a file in CWD or `.rusty_scratch/` with
//! collision avoidance (numeric suffix: `script.py` -> `script_1.py`), requires explicit
//! confirmation if overwriting an existing project file, and reports the file path
//! back without executing or prepping for execution.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CodeGenResult {
    pub file_path: PathBuf,
    pub filename: String,
    pub language: String,
    pub code_content: String,
    pub is_scratch: bool,
    pub overwrite_requires_confirmation: bool,
    pub status_message: String,
}

/// Detect whether a user chat prompt is a code generation goal (e.g., "I want...", "Create a script...", "Write a REST...").
pub fn is_code_gen_request(prompt: &str) -> bool {
    let lower = prompt.trim().to_lowercase();
    lower.starts_with("i want")
        || lower.starts_with("create a")
        || lower.starts_with("write a")
        || lower.starts_with("generate a")
        || lower.starts_with("build a")
        || lower.contains("script that")
        || lower.contains("endpoint that")
        || lower.contains("program that")
}

/// Derives a clean base filename and extension from the prompt or generated code.
pub fn derive_filename_and_ext(prompt: &str, code: &str) -> (String, String) {
    let lower_prompt = prompt.to_lowercase();
    let lower_code = code.to_lowercase();

    let ext = if lower_prompt.contains("python") || lower_code.contains("import ") || lower_code.contains("def ") {
        "py"
    } else if lower_prompt.contains("rust") || lower_code.contains("fn main") {
        "rs"
    } else if lower_prompt.contains("powershell") || lower_prompt.contains("ps1") || lower_code.contains("$env:") {
        "ps1"
    } else if lower_prompt.contains("json") || (code.trim().starts_with('{') && code.trim().ends_with('}')) {
        "json"
    } else if lower_prompt.contains("bash") || lower_prompt.contains("shell") || code.starts_with("#!/bin/bash") {
        "sh"
    } else if lower_prompt.contains("html") || lower_code.contains("<html>") {
        "html"
    } else if lower_prompt.contains("javascript") || lower_prompt.contains("node") || lower_code.contains("const ") || lower_code.contains("function ") {
        "js"
    } else {
        "py" // default fallback script type
    };

    let base_name = prompt
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty() && *s != "i" && *s != "want" && *s != "a" && *s != "to" && *s != "achieve" && *s != "script" && *s != "that" && *s != "create" && *s != "write")
        .take(3)
        .collect::<Vec<&str>>()
        .join("_");

    let clean_base = if base_name.is_empty() {
        "generated_script".to_string()
    } else {
        base_name
    };

    (clean_base, ext.to_string())
}

/// Computes a non-colliding file path by appending a numeric suffix (`_1`, `_2`, ...) if necessary.
pub fn resolve_non_colliding_path(dir: &Path, base_name: &str, ext: &str) -> PathBuf {
    let target = dir.join(format!("{base_name}.{ext}"));
    if !target.exists() {
        return target;
    }

    let mut counter = 1;
    loop {
        let candidate = dir.join(format!("{base_name}_{counter}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

/// Main execution flow for code generation request.
/// Generates file, writes code, returns summary message.
pub fn process_code_gen(
    prompt: &str,
    generated_code: &str,
    target_dir: Option<PathBuf>,
    force_project_overwrite: bool,
) -> Result<CodeGenResult, String> {
    let cwd = target_dir
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    let scratch_dir = cwd.join(".rusty_scratch");
    let _ = fs::create_dir_all(&scratch_dir);

    let (base_name, ext) = derive_filename_and_ext(prompt, generated_code);

    let scratch_file_path = resolve_non_colliding_path(&scratch_dir, &base_name, &ext);
    let direct_project_file = cwd.join(format!("{base_name}.{ext}"));

    let is_project_overwrite = direct_project_file.exists();

    if is_project_overwrite && !force_project_overwrite {
        return Ok(CodeGenResult {
            file_path: direct_project_file.clone(),
            filename: format!("{base_name}.{ext}"),
            language: ext,
            code_content: generated_code.to_string(),
            is_scratch: false,
            overwrite_requires_confirmation: true,
            status_message: format!(
                "⚠️ Target project file '{}' already exists. Require explicit confirmation before overwriting.",
                direct_project_file.display()
            ),
        });
    }

    let final_path = scratch_file_path;
    fs::write(&final_path, generated_code).map_err(|e| format!("Failed to write generated code file: {e}"))?;

    let filename = final_path.file_name().unwrap_or_default().to_string_lossy().to_string();

    Ok(CodeGenResult {
        file_path: final_path.clone(),
        filename,
        language: ext,
        code_content: generated_code.to_string(),
        is_scratch: true,
        overwrite_requires_confirmation: false,
        status_message: format!("📄 Code generated and written to: {}\n(Not executed)", final_path.display()),
    })
}

/// Cleans up old scratch files in `.rusty_scratch/`.
pub fn clean_scratch_directory(dir: Option<PathBuf>) -> Result<usize, String> {
    let cwd = dir.or_else(|| std::env::current_dir().ok()).unwrap_or_else(|| PathBuf::from("."));
    let scratch_dir = cwd.join(".rusty_scratch");

    if !scratch_dir.exists() {
        return Ok(0);
    }

    let entries = fs::read_dir(&scratch_dir).map_err(|e| format!("Failed to read scratch dir: {e}"))?;
    let mut count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if fs::remove_file(&path).is_ok() {
                count += 1;
            }
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_code_gen_intent() {
        assert!(is_code_gen_request("I want a Python script to rename files"));
        assert!(is_code_gen_request("I want to achieve a REST endpoint"));
        assert!(is_code_gen_request("Create a script that parses logs"));
        assert!(!is_code_gen_request("git status"));
    }

    #[test]
    fn derives_filename_and_ext() {
        let (name, ext) = derive_filename_and_ext("I want a Python script to rename files", "import os\ndef rename(): pass");
        assert_eq!(ext, "py");
        assert!(name.contains("rename"));
    }

    #[test]
    fn numeric_suffix_on_collision() {
        let temp_dir = std::env::temp_dir().join("rusty_test_collision");
        let _ = fs::create_dir_all(&temp_dir);

        let f1 = temp_dir.join("test.py");
        fs::write(&f1, "hello").unwrap();

        let resolved = resolve_non_colliding_path(&temp_dir, "test", "py");
        assert_eq!(resolved.file_name().unwrap().to_str().unwrap(), "test_1.py");

        let _ = fs::remove_dir_all(temp_dir);
    }
}
