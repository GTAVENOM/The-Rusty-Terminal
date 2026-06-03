use std::path::PathBuf;

#[derive(Debug,Clone,PartialEq)]
pub enum CommandAction {
    OpenApp {name: String},
    OpenFolder {path: PathBuf},
    ExecuteSystemCommand {command: String, args: Vec<String>},
    Exit,
    Unknown {original_input: String},
}