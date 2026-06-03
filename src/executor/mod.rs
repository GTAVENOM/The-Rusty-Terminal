use std::error::Error;
use crate::actions::CommandAction;

pub struct SystemExecutor;
impl SystemExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self, action: &CommandAction) -> Result<(), Box<dyn Error>>{
        match action{
            CommandAction::OpenApp {name}=>self.open_app(name),
            CommandAction::OpenFolder {path}=>self.open_folder(path),
            CommandAction::ExecuteSystemCommand {command,args}=>self.run_command(command,args),
            CommandAction::Exit=>Ok(()),
            CommandAction::Unknown {original_input}=>{
                println!("Unknown command: '{}'",original_input);
                Ok(())
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod macos;