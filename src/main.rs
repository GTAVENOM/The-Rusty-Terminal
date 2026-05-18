use dirs::home_dir;
use std::io::{self,Write};
use std::process::Command;

fn main(){
    println!("Welcome to The Rusty Terminal!");
    println!("Type 'bye' to quit.\n");

    loop{
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input =String::new();

        io::stdin().read_line(&mut input).expect("Failed to read input");

        let input=input.trim().to_lowercase();

        match input.as_str() {
            "bye"=>{
                println!("Goodbye.");
                break;
            }

            "open zen" => {
                open_app("Zen");
            }

            "open vscode" => {
                open_app("Visual Studio Code");
            }

            "open downloads" => {
                open_folder("Downloads");
            }

            "open coding" => {
                open_folder("Coding")
            }

            _ =>{
                println!("Unknown Command.");
            }
        }
    }
}

fn open_app(app_name: &str){
    #[cfg(target_os="macos")]
    {
        Command::new("open").arg("-a").arg(app_name).spawn().expect("Failed to open {app_name}");
    }

    #[cfg(target_os="linux")]
    {
        Command::new(app_name).spawn().expect("Failed to open {app_name}");
    }
}

fn open_folder(folder: &str){
    let mut path=home_dir().expect("Could not find home directory.");
    path.push(folder);
    #[cfg(target_os = "macos")]{
        Command::new("open").arg(path).spawn().expect("Failed to open {path}");
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(path).spawn().expect("Failed to open {path}");
    }
}