use std::error::Error;
use std::path::Path;
use std::process::Command;
use crate::executor::SystemExecutor;

impl SystemExecutor{
    pub fn open_app(&self,app_name:&str)->Result<(), Box<dyn Error>>{
        println!("Opening app: {}", app_name);

        let status=Command::new("open").arg("-a").arg(app_name).status()?;

        if status.success(){
            Ok(())
        }
        else{
            Err(format!("Failed to open application: '{}'",app_name).into())
        }
    }
    pub fn open_folder(&self, path: &Path)->Result<(), Box<dyn Error>>{
        println!("Opening directory: {}",path.display());
        let status=Command::new("open").arg(path).status()?;
        
        if status.success(){
            Ok(())
        }
        else{
            Err(format!("Failed to open folder: '{}'",path.display()).into())
        }
    }

    pub fn run_command(&self, command: &str, args: &[String])->Result<(), Box<dyn Error>>{
        let status=Command::new(command).args(args).status()?;

        if status.success(){
            Ok(())
        }
        else{
            Err(format!("Command '{}' returned non-zero exit code",command).into())
        }
    }
}