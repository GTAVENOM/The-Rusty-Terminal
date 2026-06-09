use std::path::{Path, PathBuf};
use rustyline::completion::{Completer, Pair};
use rustyline::{Context,Helper};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use std::fs;


pub struct FolderCompleter{
    supported_folders: Vec<&'static str>,
}

impl FolderCompleter{
    pub fn new()-> Self{
        Self{
            supported_folders: vec!["downloads","coding","documents","desktop"],
        }
    }

    fn expand_tilde(path_str: &str)->Option<PathBuf>{
        if path_str.starts_with("~/"){
            dirs::home_dir().map(|mut home|{
                home.push(&path_str[2..]);
                home
            })
        }
        else if path_str == "~"{
            dirs::home_dir()
        }
        else {
            Some(PathBuf::from(path_str))
        }
    }
}

impl Completer for FolderCompleter{

    type Candidate=Pair;
    fn complete(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let mut candidates = Vec::new();

        let command_prefixes=["open ","show ","goto ","launch "];

        for prefix in command_prefixes{
            if line.to_lowercase().starts_with(prefix) {
                let typed_path = &line[prefix.len()..pos];
                let (search_dir,file_prefix)=if typed_path.is_empty(){
                    (PathBuf::from("."),"".to_string())
                }
                else {
                    let path=Path::new(typed_path);
                    if typed_path.ends_with('/')||typed_path.ends_with('\\'){
                        let expanded=Self::expand_tilde(typed_path).unwrap_or_else(|| PathBuf::from(typed_path));
                        (expanded,"".to_string())
                    }
                    else {
                        let parent=path.parent().unwrap_or_else(||Path::new(""));
                        let parent_str=parent.to_str().unwrap_or("");
                        let expanded_parent=if parent_str.is_empty(){
                            PathBuf::from(".")
                        }
                        else {
                            Self::expand_tilde(parent_str).unwrap_or_else(|| PathBuf::from(parent_str))
                        };
                        let file_name=path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                        (expanded_parent,file_name)
                    }
                };
                if let Ok(entries)=fs::read_dir(&search_dir){
                    for entry in entries.flatten(){
                        if let Ok(metadata)=entry.metadata(){
                            if metadata.is_dir(){
                                if let Some(name_str)=entry.file_name().to_str(){
                                    if name_str.to_lowercase().starts_with(&file_prefix.to_lowercase()){
                                        let typed_parent=if typed_path.ends_with('/')||typed_path.ends_with('\\'){
                                            typed_path.to_string()
                                        }
                                        else {
                                            let parent=Path::new(typed_path).parent().unwrap_or_else(|| Path::new(""));
                                            let parent_str=parent.to_str().unwrap_or("");
                                            if parent_str.is_empty(){
                                                "".to_string()
                                            }
                                            else {
                                                format!("{}/",parent_str)
                                            }
                                        };
                                        let display_name=name_str.to_string();
                                        let replacement=format!("{}{}{}/",prefix,typed_parent,name_str);
                                        candidates.push(Pair{
                                            display: display_name,
                                            replacement,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok((0,candidates))
    }
}

impl Hinter for FolderCompleter{
    type Hint= String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        None
    }
}

impl Helper for FolderCompleter{}

impl Highlighter for FolderCompleter{}

impl Validator for FolderCompleter{}