use rustyline::completion::{Completer, Pair};
use rustyline::{Context,Helper};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;


pub struct FolderCompleter{
    supported_folders: Vec<&'static str>,
}

impl FolderCompleter{
    pub fn new()-> Self{
        Self{
            supported_folders: vec!["downloads","coding","documents","desktop"],
        }
    }
}

impl Completer for FolderCompleter{

    type Candidate=Pair;
    fn complete(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Result<(usize, Vec<Pair>), ReadlineResultError> {
        let mut candidates = Vec::new();

        let command_prefixes=["open ","show ","goto ","launch "];

        for prefix in command_prefixes{
            if line.to_lowercase().starts_with(prefix){
                let typed_so_far=&line[prefix.len()..pos];
                let typed_lower=typed_so_far.to_lowercase();

                for folder in &self.supported_folders{
                    if folder.starts_with(&typed_lower){
                        candidates.push(Pair{
                            display: folder.to_string(),
                            replacement: format!("{}{}",prefix,folder),
                        });
                    }
                }
            }
        }
        Ok((0,candidates))
    }
}
type ReadlineResultError=ReadlineError;

impl Hinter for FolderCompleter{
    type Hint= String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        None
    }
}

impl Helper for FolderCompleter{}

impl Highlighter for FolderCompleter{}

impl Validator for FolderCompleter{}