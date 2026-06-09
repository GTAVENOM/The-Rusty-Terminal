use std::error::Error;
use std::io::{self, Write};
use rustyline::DefaultEditor;
use rustyline::{Config,CompletionType,error::ReadlineError};
use crate::actions::CommandAction;
use crate::executor::SystemExecutor;
use crate::parser::ai_parser::AiParser;
use crate::parser::CommandParser;


mod parser;
mod executor;
mod actions;
mod completer;
fn main() -> Result<(), Box<dyn Error>>{
    println!("Welcome to The Rusty Terminal 🦀");
    println!("Type 'bye' to quit\n");

    let parser=AiParser::new("llama3.2:1b");
    let executor=SystemExecutor::new();

    let config=Config::builder().completion_type(CompletionType::List).build();

    let helper=completer::FolderCompleter::new();

    let mut rl=rustyline::Editor::with_config(config)?;
    rl.set_helper(Some(helper));

    let history_path=dirs::home_dir().map(|mut path| {
        path.push(".rusty_terminal_history");
        path
    });

    if let Some(ref path)=history_path {
        let _ =rl.load_history(path);
    }

    loop{
        let readline=rl.readline("> ");
        match readline{
            Ok(line)=>{
                let trimmed=line.trim();
                if trimmed.is_empty(){
                    continue;
                }

                let _ = rl.add_history_entry(trimmed);
                let action = parser.parse(trimmed);

                if action==CommandAction::Exit{
                    println!("Goodbye.");
                    break;
                }
                if let Err(e) = executor.execute(&action){
                    eprintln!("Error: {}",e);

                }
            }
            Err(ReadlineError::Interrupted)=>{
                println!("Ctrl-C pressed. Type 'bye' to quit.");
            }
            Err(ReadlineError::Eof)=>{
                println!("Goodbye.");
            }
            Err(err)=>{
                println!("Readline Error: {:?}",err);
                break;
            }

        }
    }

    if let Some(ref path)= history_path{
        let _ = rl.save_history(path)?;
    }
    Ok(())
}