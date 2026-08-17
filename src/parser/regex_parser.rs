use std::path::PathBuf;
use dirs::home_dir;
use regex::Regex;
use crate::actions::CommandAction;
use crate::parser::CommandParser;

pub struct RegexParser {
    exit_regex: Regex,
    clear_regex: Regex,
    cd_regex: Regex,
    open_regex: Regex,
    system_cmd_regex: Regex,
}

impl RegexParser {
    pub fn new() -> Self {
        Self {
            exit_regex: Regex::new(r"(?i)^(?:exit|bye|quit)$").unwrap(),
            clear_regex: Regex::new(r"(?i)^(?:clear|cls)$").unwrap(),
            cd_regex: Regex::new(r"(?i)^cd(?:\s+(?P<target>.+))?$").unwrap(),
            open_regex: Regex::new(r"(?i)^(?:open|launch)\s+(?P<target>.+)$").unwrap(),
            system_cmd_regex: Regex::new(
                r"(?i)^(?:ls|pwd|git|cat|echo|mkdir|rm|rmdir|touch|cp|mv|grep|find|cargo|python|python3|node|npm|npx|which|head|tail|curl|whoami|date|ps|top|kill|man|vim|nano|code)(?:\s+.*)?$",
            ).unwrap(),
        }
    }

    fn resolve_folder_path(&self, target: &str) -> Option<PathBuf> {
        crate::fuzzy::resolve_fuzzy_path(target)
    }

    fn parse_command_args(input: &str) -> (String, Vec<String>) {
        let mut parts = input.split_whitespace();
        let command = parts.next().unwrap_or("").to_string();
        let args = parts.map(|s| s.to_string()).collect();
        (command, args)
    }
}

impl CommandParser for RegexParser {
    fn parse(&self, input: &str) -> CommandAction {
        let trimmed = input.trim();

        if self.exit_regex.is_match(trimmed) {
            return CommandAction::Exit;
        }

        if self.clear_regex.is_match(trimmed) {
            return CommandAction::ClearScreen;
        }

        if let Some(captures) = self.cd_regex.captures(trimmed) {
            let target = captures
                .name("target")
                .map(|m| m.as_str().trim())
                .unwrap_or("~");

            let target_path = if target == "~" || target.is_empty() {
                home_dir().unwrap_or_else(|| PathBuf::from("."))
            } else if target.starts_with("~/") {
                if let Some(mut home) = home_dir() {
                    home.push(&target[2..]);
                    home
                } else {
                    PathBuf::from(target)
                }
            } else {
                PathBuf::from(target)
            };

            return CommandAction::ChangeDirectory { path: target_path };
        }

        if let Some(captures) = self.open_regex.captures(trimmed) {
            if let Some(target_match) = captures.name("target") {
                let target = target_match.as_str();

                if let Some(folder_path) = self.resolve_folder_path(target) {
                    return CommandAction::OpenFolder { path: folder_path };
                } else {
                    return CommandAction::OpenApp {
                        name: target.to_string(),
                    };
                }
            }
        }

        // Support prefixing with ! or $ for direct system commands
        let raw_cmd = if trimmed.starts_with('!') || trimmed.starts_with('$') {
            trimmed[1..].trim()
        } else if self.system_cmd_regex.is_match(trimmed) {
            trimmed
        } else {
            ""
        };

        if !raw_cmd.is_empty() {
            let (command, args) = Self::parse_command_args(raw_cmd);
            if !command.is_empty() {
                return CommandAction::ExecuteSystemCommand { command, args };
            }
        }

        CommandAction::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_commands() {
        let parser = RegexParser::new();
        assert_eq!(parser.parse("exit"), CommandAction::Exit);
        assert_eq!(parser.parse("bye"), CommandAction::Exit);
        assert_eq!(parser.parse("quit"), CommandAction::Exit);
        assert_eq!(parser.parse(" EXIT "), CommandAction::Exit);
    }

    #[test]
    fn test_clear_commands() {
        let parser = RegexParser::new();
        assert_eq!(parser.parse("clear"), CommandAction::ClearScreen);
        assert_eq!(parser.parse("cls"), CommandAction::ClearScreen);
    }

    #[test]
    fn test_cd_commands() {
        let parser = RegexParser::new();
        let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
        assert_eq!(
            parser.parse("cd"),
            CommandAction::ChangeDirectory { path: home.clone() }
        );
        assert_eq!(
            parser.parse("cd ~"),
            CommandAction::ChangeDirectory { path: home }
        );
        assert_eq!(
            parser.parse("cd /tmp"),
            CommandAction::ChangeDirectory {
                path: PathBuf::from("/tmp")
            }
        );
    }

    #[test]
    fn test_system_commands() {
        let parser = RegexParser::new();
        assert_eq!(
            parser.parse("ls -la"),
            CommandAction::ExecuteSystemCommand {
                command: "ls".to_string(),
                args: vec!["-la".to_string()]
            }
        );
        assert_eq!(
            parser.parse("pwd"),
            CommandAction::ExecuteSystemCommand {
                command: "pwd".to_string(),
                args: vec![]
            }
        );
        assert_eq!(
            parser.parse("!echo hello"),
            CommandAction::ExecuteSystemCommand {
                command: "echo".to_string(),
                args: vec!["hello".to_string()]
            }
        );
    }
}