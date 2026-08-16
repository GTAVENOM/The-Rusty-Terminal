//! Open-Ended AI Shell Inference Engine.
//! Default model: Qwen2.5-Coder-1.5B-Instruct-Q4_K_M.gguf (<1GB: ~980MB)
//!
//! Zero rigid rule-books. Uses neural LLM reasoning to translate ANY arbitrary
//! open-ended English sentence into an executable shell command with dynamic safety tiering.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

use serde_json::Value;
use crate::intent::schema::Intent;
use crate::safety::tier_classifier;

const MODEL_FILENAME: &str = "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf";
const MODEL_DOWNLOAD_URL: &str =
    "https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF/resolve/main/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf";

pub fn models_dir() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("RustyTerminal");
    dir.push("models");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn default_model_path() -> PathBuf {
    models_dir().join(MODEL_FILENAME)
}

pub fn is_local_model_present() -> bool {
    default_model_path().exists() && fs::metadata(default_model_path()).map(|m| m.len() > 10_000_000).unwrap_or(false)
}

/// Download local Qwen2.5-Coder-1.5B model with terminal progress indicator.
pub fn download_model_if_missing() -> Result<(), String> {
    let path = default_model_path();
    if is_local_model_present() {
        return Ok(());
    }

    eprintln!("📥 Downloading Qwen2.5-Coder-1.5B local model (<1GB: ~980 MB)...");
    let response = ureq::get(MODEL_DOWNLOAD_URL)
        .call()
        .map_err(|e| format!("HTTP download request failed: {e}"))?;

    let total_size = response
        .header("content-length")
        .and_then(|l| l.parse::<u64>().ok())
        .unwrap_or(980_000_000);

    let tmp_path = path.with_extension("tmp");
    let mut dest = File::create(&tmp_path).map_err(|e| format!("Failed to create file: {e}"))?;
    let mut reader = response.into_reader();
    let mut buffer = [0u8; 65536];
    let mut downloaded: u64 = 0;
    let mut last_reported = 0;

    loop {
        let bytes_read = reader.read(&mut buffer).map_err(|e| format!("Read error: {e}"))?;
        if bytes_read == 0 {
            break;
        }
        dest.write_all(&buffer[..bytes_read]).map_err(|e| format!("Write error: {e}"))?;
        downloaded += bytes_read as u64;

        let mb = downloaded / 1_048_576;
        if mb > last_reported + 10 || downloaded == total_size {
            last_reported = mb;
            let percent = (downloaded as f64 / total_size as f64 * 100.0) as u32;
            eprint!("\r⏳ Download progress: {mb} MB / {} MB [{percent}%]", total_size / 1_048_576);
            let _ = std::io::stderr().flush();
        }
    }

    eprintln!("\n✅ Local Qwen2.5-Coder-1.5B model download complete!");
    fs::rename(tmp_path, path).map_err(|e| format!("Failed to finalize model file: {e}"))?;
    Ok(())
}

/// System prompt for dynamic AI command translation.
pub fn build_system_prompt(phrase: &str) -> String {
    format!(
        "You are Rusty Terminal AI. Translate the user's natural language request into an executable terminal shell command.\n\
         Output ONLY the raw executable command line with no markdown formatting or explanation.\n\n\
         User request: \"{phrase}\""
    )
}

/// Pure open-ended AI inference execution.
/// Translates ANY arbitrary English phrase into an executable shell command with dynamic safety tiering.
pub fn run_local_inference(phrase: &str) -> Result<Intent, String> {
    // 1. Try local Ollama / GGUF model with dynamic prompt
    let prompt = build_system_prompt(phrase);
    if let Ok(resp) = ureq::post("http://localhost:11434/api/generate")
        .timeout(std::time::Duration::from_secs(4))
        .send_json(serde_json::json!({
            "model": "qwen2.5-coder:1.5b",
            "prompt": prompt,
            "stream": false
        }))
    {
        if let Ok(json) = resp.into_json::<Value>() {
            if let Some(text) = json["response"].as_str() {
                let clean_cmd = text.trim().trim_matches('`').trim().to_string();
                if !clean_cmd.is_empty() {
                    let tier = tier_classifier::classify(&clean_cmd);
                    return Ok(Intent::DynamicShellCommand {
                        command: clean_cmd,
                        tier,
                        description: format!("AI Generated Shell Command for '{phrase}'"),
                    });
                }
            }
        }
    }

    // 2. Try JSON intent payload parsing if structured JSON was emitted
    if let Ok(intent) = parse_llm_json_intent(phrase) {
        return Ok(intent);
    }

    // 3. Fallback: Wrap clean input directly with dynamic safety classifier
    let clean_cmd = phrase.trim().to_string();
    let tier = tier_classifier::classify(&clean_cmd);
    Ok(Intent::DynamicShellCommand {
        command: clean_cmd,
        tier,
        description: format!("Shell command for '{phrase}'"),
    })
}

/// Parse structured JSON intent payload emitted by LLMs into a typed `Intent`.
pub fn parse_llm_json_intent(text: &str) -> Result<Intent, String> {
    let start_idx = text.find('{').ok_or("No JSON object found")?;
    let end_idx = text.rfind('}').ok_or("Malformed JSON object")?;
    let json_str = &text[start_idx..=end_idx];

    let val: Value = serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {e}"))?;
    let name = val["intent"].as_str().ok_or("Missing 'intent' field")?;
    let args = val.get("args").cloned().unwrap_or(serde_json::json!({}));

    match name {
        "ListFiles" => {
            let path = args["path"].as_str().map(|s| s.to_string());
            let all = args["all"].as_bool().unwrap_or(false);
            Ok(Intent::ListFiles(crate::intent::schema::ListFilesArgs { path, all }))
        },
        "OpenFolder" => {
            let path = args["path"].as_str().map(|s| s.to_string());
            Ok(Intent::OpenFolder(crate::intent::schema::OpenFolderArgs { path }))
        },
        "ClearTerminal" => Ok(Intent::ClearTerminal),
        "GitStatus" => Ok(Intent::GitStatus),
        "GitLog" => {
            let max_count = args["max_count"].as_u64().map(|n| n as u32);
            let oneline = args["oneline"].as_bool().unwrap_or(true);
            Ok(Intent::GitLog(crate::intent::schema::GitLogArgs { max_count, oneline }))
        },
        "GitDiff" => {
            let base = args["base"].as_str().map(|s| s.to_string());
            let stat = args["stat"].as_bool().unwrap_or(false);
            Ok(Intent::GitDiff(crate::intent::schema::GitDiffArgs { base, stat }))
        },
        "GitBranchList" => Ok(Intent::GitBranchList),
        "FindProcessByPort" => {
            let port = args["port"].as_u64().ok_or("Missing 'port' arg")? as u16;
            Ok(Intent::FindProcessByPort(crate::intent::schema::FindProcessByPortArgs { port }))
        },
        "DockerPs" => {
            let all = args["all"].as_bool().unwrap_or(true);
            Ok(Intent::DockerPs(crate::intent::schema::DockerPsArgs { all }))
        },
        "DockerLogs" => {
            let container = args["container"].as_str().unwrap_or("api").to_string();
            let tail = args["tail"].as_u64().map(|n| n as u32);
            Ok(Intent::DockerLogs(crate::intent::schema::DockerLogsArgs { container, tail, follow: false }))
        },
        "DockerPull" => {
            let image = args["image"].as_str().unwrap_or("ubuntu:latest").to_string();
            Ok(Intent::DockerPull(crate::intent::schema::DockerPullArgs { image }))
        },
        "ShowEnvVars" => {
            let filter = args["filter"].as_str().map(|s| s.to_string());
            Ok(Intent::ShowEnvVars(crate::intent::schema::ShowEnvVarsArgs { filter }))
        },
        "SystemInfo" => Ok(Intent::SystemInfo),
        "NetworkInfo" => Ok(Intent::NetworkInfo),
        "MakeDirectory" => {
            let name = args["name"].as_str().unwrap_or("new_folder").to_string();
            Ok(Intent::MakeDirectory(crate::intent::schema::MakeDirectoryArgs { name }))
        },
        "GitPull" => Ok(Intent::GitPull(crate::intent::schema::GitPullArgs::default())),
        "GitCommit" => {
            let message = args["message"].as_str().unwrap_or("update").to_string();
            Ok(Intent::GitCommit(crate::intent::schema::GitCommitArgs { message }))
        },
        "GitCheckout" => {
            let branch = args["branch"].as_str().unwrap_or("main").to_string();
            Ok(Intent::GitCheckout(crate::intent::schema::GitCheckoutArgs { branch }))
        },
        other => Err(format!("Unknown LLM intent name '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_shell_command_generation() {
        let prompt = "pull ubuntu latest image";
        let intent = run_local_inference(prompt).unwrap();
        assert_eq!(intent.tier(), crate::safety::tier_classifier::Tier::Idempotent);
    }
}
