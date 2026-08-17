use crate::actions::CommandAction;
use crate::parser::CommandParser;
use crate::parser::regex_parser::RegexParser;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct AiParser {
    model_name: String,
    ollama_url: String,
    regex_parser: RegexParser,
}

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    format: &'a str,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl AiParser {
    pub fn new(model_name: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            ollama_url: "http://localhost:11434/api/generate".to_string(),
            regex_parser: RegexParser::new(),
        }
    }

    fn clean_json_str(raw: &str) -> String {
        let mut s = raw.trim();
        if let Some(start) = s.find('{') {
            if let Some(end) = s.rfind('}') {
                if start <= end {
                    s = &s[start..=end];
                }
            }
        }
        s.to_string()
    }

    fn parse_flexible_json(&self, json_str: &str) -> Option<CommandAction> {
        let cleaned = Self::clean_json_str(json_str);
        let v: serde_json::Value = serde_json::from_str(&cleaned).ok()?;
        let action_str = v.get("action")?.as_str()?;

        match action_str {
            "Exit" => Some(CommandAction::Exit),
            "ClearScreen" => Some(CommandAction::ClearScreen),
            "ChangeDirectory" => {
                let path_str = v
                    .get("path")
                    .or_else(|| v.get("target"))
                    .and_then(|p| p.as_str())
                    .unwrap_or("~");
                Some(CommandAction::ChangeDirectory {
                    path: std::path::PathBuf::from(path_str),
                })
            }
            "Open" => {
                let target = v.get("target").and_then(|t| t.as_str())?.to_string();
                if let Some(path) = crate::fuzzy::resolve_fuzzy_path(&target) {
                    if path.is_dir() {
                        Some(CommandAction::OpenFolder { path })
                    } else {
                        Some(CommandAction::OpenApp { name: target })
                    }
                } else {
                    Some(CommandAction::OpenApp { name: target })
                }
            }
            "OpenApp" => {
                let name = v
                    .get("name")
                    .or_else(|| v.get("target"))
                    .and_then(|n| n.as_str())?
                    .to_string();
                Some(CommandAction::OpenApp { name })
            }
            "OpenFolder" => {
                let path_str = v
                    .get("path")
                    .or_else(|| v.get("target"))
                    .and_then(|p| p.as_str())?;
                let path_buf = crate::fuzzy::resolve_fuzzy_path(path_str)
                    .unwrap_or_else(|| std::path::PathBuf::from(path_str));
                Some(CommandAction::OpenFolder { path: path_buf })
            }
            "ExecuteSystemCommand" => {
                let raw_command = v.get("command")?.as_str()?;
                let mut args = Vec::new();

                if let Some(args_val) = v.get("args") {
                    if let Some(arr) = args_val.as_array() {
                        for item in arr {
                            if let Some(s) = item.as_str() {
                                args.push(s.to_string());
                            }
                        }
                    } else if let Some(s) = args_val.as_str() {
                        args.extend(s.split_whitespace().map(|x| x.to_string()));
                    }
                }

                // If command string itself contains spaces (e.g. "ls -la"), tokenize it
                let mut cmd_parts = raw_command.split_whitespace();
                let command = cmd_parts.next().unwrap_or("").to_string();
                let mut extra_args: Vec<String> = cmd_parts.map(|s| s.to_string()).collect();
                extra_args.extend(args);

                if command == "cd" {
                    let target = extra_args.first().cloned().unwrap_or_else(|| "~".to_string());
                    let target_path = if target == "~" || target.is_empty() {
                        dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
                    } else if target.starts_with("~/") {
                        dirs::home_dir()
                            .map(|mut h| {
                                h.push(&target[2..]);
                                h
                            })
                            .unwrap_or_else(|| std::path::PathBuf::from(&target))
                    } else {
                        std::path::PathBuf::from(target)
                    };
                    Some(CommandAction::ChangeDirectory { path: target_path })
                } else if command == "clear" || command == "cls" {
                    Some(CommandAction::ClearScreen)
                } else {
                    Some(CommandAction::ExecuteSystemCommand {
                        command,
                        args: extra_args,
                    })
                }
            }
            _ => None,
        }
    }
}

impl CommandParser for AiParser {
    fn parse(&self, input: &str) -> CommandAction {
        // Fast path: check RegexParser first for known regex patterns (exit, cd, clear, common commands)
        let regex_action = self.regex_parser.parse(input);
        if regex_action != CommandAction::Unknown {
            return regex_action;
        }

        let system_prompt = format!(
            "<instruction>\n\
            You are a system parser for an intelligent terminal CLI.\n\
            Translate the user's input (whether raw shell command or natural language request) into a single raw JSON object matching one of these exact schemas:\n\n\
            1. For listing files, showing directory contents, running git commands, or any system shell commands:\n\
               {{\"action\": \"ExecuteSystemCommand\", \"command\": \"command_name\", \"args\": [\"arg1\", \"arg2\"]}}\n\
               Examples:\n\
               - \"list all the files in this directory\" -> {{\"action\": \"ExecuteSystemCommand\", \"command\": \"ls\", \"args\": [\"-la\"]}}\n\
               - \"show files\" -> {{\"action\": \"ExecuteSystemCommand\", \"command\": \"ls\", \"args\": []}}\n\
               - \"where am i\" -> {{\"action\": \"ExecuteSystemCommand\", \"command\": \"pwd\", \"args\": []}}\n\
               - \"git status\" -> {{\"action\": \"ExecuteSystemCommand\", \"command\": \"git\", \"args\": [\"status\"]}}\n\n\
            2. For changing directory or navigating to folders:\n\
               {{\"action\": \"ChangeDirectory\", \"path\": \"target_path\"}}\n\
               Examples:\n\
               - \"go to home\" -> {{\"action\": \"ChangeDirectory\", \"path\": \"home\"}}\n\
               - \"go to home directory\" -> {{\"action\": \"ChangeDirectory\", \"path\": \"home\"}}\n\
               - \"go to parent directory\" -> {{\"action\": \"ChangeDirectory\", \"path\": \"..\"}}\n\
               - \"go back\" -> {{\"action\": \"ChangeDirectory\", \"path\": \"..\"}}\n\
               - \"change directory to downloads\" -> {{\"action\": \"ChangeDirectory\", \"path\": \"Downloads\"}}\n\
               - \"cd ..\" -> {{\"action\": \"ChangeDirectory\", \"path\": \"..\"}}\n\n\
            3. For clearing terminal screen:\n\
               {{\"action\": \"ClearScreen\"}}\n\
               Examples:\n\
               - \"clear the terminal\" -> {{\"action\": \"ClearScreen\"}}\n\
               - \"clear screen\" -> {{\"action\": \"ClearScreen\"}}\n\n\
            4. For exiting or closing the terminal:\n\
               {{\"action\": \"Exit\"}}\n\
               Examples:\n\
               - \"exit\" -> {{\"action\": \"Exit\"}}\n\
               - \"bye\" -> {{\"action\": \"Exit\"}}\n\n\
            5. For opening GUI applications (e.g. Spotify, Chrome, Safari, Finder, Zen):\n\
               {{\"action\": \"Open\", \"target\": \"application_name\"}}\n\
               Examples:\n\
               - \"open Zen\" -> {{\"action\": \"Open\", \"target\": \"Zen\"}}\n\
               - \"open Spotify\" -> {{\"action\": \"Open\", \"target\": \"Spotify\"}}\n\n\
            Rules:\n\
            - Output ONLY the raw JSON object.\n\
            - Do NOT wrap the JSON in markdown code blocks or additional text.\n\
            </instruction>\n\n\
            Input: \"{}\"\n\
            JSON:",
            input
        );

        let request_payload = OllamaRequest {
            model: &self.model_name,
            prompt: &system_prompt,
            stream: false,
            format: "json",
            options: OllamaOptions { temperature: 0.0 },
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build();

        let client = match client {
            Ok(c) => c,
            Err(_) => return regex_action,
        };

        let response = client.post(&self.ollama_url).json(&request_payload).send();

        match response {
            Ok(res) => {
                if let Ok(ollama_res) = res.json::<OllamaResponse>() {
                    if let Some(action) = self.parse_flexible_json(&ollama_res.response) {
                        action
                    } else {
                        eprintln!("Failed to parse LLM response JSON");
                        regex_action
                    }
                } else {
                    regex_action
                }
            }
            Err(_) => {
                eprintln!("\n⚠️ Warning: Could not connect to local Ollama server. Is it running? (run 'ollama serve')");
                regex_action
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_parser_exit_without_ollama() {
        let parser = AiParser::new("llama3.2:1b");
        // Should return Exit instantly via RegexParser without needing Ollama server
        assert_eq!(parser.parse("exit"), CommandAction::Exit);
        assert_eq!(parser.parse("bye"), CommandAction::Exit);
        assert_eq!(parser.parse("quit"), CommandAction::Exit);
    }

    #[test]
    fn test_parse_flexible_json() {
        let parser = AiParser::new("llama3.2:1b");

        let json_markdown = "```json\n{\"action\": \"ExecuteSystemCommand\", \"command\": \"ls -la\", \"args\": []}\n```";
        let action = parser.parse_flexible_json(json_markdown);
        assert_eq!(
            action,
            Some(CommandAction::ExecuteSystemCommand {
                command: "ls".to_string(),
                args: vec!["-la".to_string()]
            })
        );

        let json_string_args = "{\"action\": \"ExecuteSystemCommand\", \"command\": \"git\", \"args\": \"status\"}";
        let action = parser.parse_flexible_json(json_string_args);
        assert_eq!(
            action,
            Some(CommandAction::ExecuteSystemCommand {
                command: "git".to_string(),
                args: vec!["status".to_string()]
            })
        );
    }
}