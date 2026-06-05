use std::path::PathBuf;
use serde::{Serialize,Deserialize};

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(tag="action")]
pub enum CommandAction {
    Open{target:String},
    OpenApp {name: String},
    OpenFolder {path: PathBuf},
    ExecuteSystemCommand {command: String, args: Vec<String>},
    Exit,
    #[serde(other)]
    Unknown,
}