use std::path::PathBuf;
use dirs::home_dir;
use regex::Regex;
use crate::actions::CommandAction;
use crate::parser::CommandParser;

pub struct RegexParser {
    exit_regex: Regex,
    open_regex: Regex,
}

impl RegexParser {
    pub fn new()->Self {
        Self {
            exit_regex: Regex::new(r"(?i)^(?:exit|bye|quit)$").unwrap(),
            open_regex: Regex::new(r"(?i)^(?:open|launch|show|goto)\s+(?P<target>.+)$").unwrap(),
        }
    }

    fn resolve_folder_path(&self, target: &str) -> Option<PathBuf> {
        let clean_target=target.replace(r"\ "," ");
        let clean_target=clean_target.trim();
        let expanded_path=if clean_target.starts_with("~/"){
            if let Some(mut home)=home_dir(){
                home.push(&clean_target[2..]);
                Some(home)
            }
            else{
                None
            }
        }
        else if clean_target=="~"{
            home_dir()
        }
        else{
            Some(PathBuf::from(clean_target))
        };

        if let Some(path)=expanded_path{
            let path_str=clean_target.to_lowercase();
            if path.exists()
            || clean_target.starts_with('/')
            || clean_target.starts_with('.')
            || path_str.contains('/')
            || path_str.contains('\\')
            || matches!(path_str.as_str(),"downloads"|"coding"|"documents"|"desktop"){
                if matches!(path_str.as_str(),"downloads"|"coding"|"documents"|"desktop"){
                    if let Some(mut home) = home_dir(){
                        let actual_folder=match path_str.as_str(){
                            "downloads"=>"Downloads",
                            "coding"=>"Coding",
                            "documents"=>"Documents",
                            "desktop"=>"Desktop",
                            _=>unreachable!(),
                        };
                        home.push(actual_folder);
                        return Some(home);
                    }
                }
                return Some(path);
            }
        }
        None
    }
}

impl CommandParser for RegexParser{
    fn parse(&self, input: &str) -> CommandAction {
        let trimmed=input.trim();

        if self.exit_regex.is_match(trimmed){
            return CommandAction::Exit;
        }

        if let Some(captures)=self.open_regex.captures(trimmed){
            if let Some(target_match)=captures.name("target"){
                let target=target_match.as_str();

                if let Some(folder_path)=self.resolve_folder_path(target){
                    return CommandAction::OpenFolder {path: folder_path};
                }
                else{
                    return CommandAction::OpenApp {name: target.to_string(),};
                }
            }
        }

        CommandAction::Unknown {original_input: trimmed.to_string(),}
    }
}