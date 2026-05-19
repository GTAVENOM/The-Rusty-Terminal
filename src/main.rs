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

            "open webstorm" => {
                open_app("WebStorm")
            }

            "open discord" => {
                open_app("Discord")
            }

            "open vscode" => {
                open_app("Visual Studio Code");
            }

            "open steam" => {
                open_app("Steam");
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
        match Command::new("open").arg("-a").arg(app_name).spawn()
        {
            Ok(_) => println!("Opened {app_name}"),
            Err(e) => println!("Failed to open {app_name}: {e}"),
        }
    }

    #[cfg(target_os="linux")]
    {
        let commands = match app_name {
            "WebStorm" => vec!["webstorm", "webstorm.sh"],
            "Discord" => vec!["discord", "Discord"],
            "Visual Studio Code" => vec!["code", "vscode", "codium"],
            "Zen" => vec!["zen", "zen-browser"],
            "Steam" => vec!["steam", "Steam"],
            _ => vec![app_name],
        };

        let mut opened = false;
        let mut last_err = None;

        for cmd in &commands {
            match Command::new(cmd).spawn() {
                Ok(_) => {
                    println!("Opened {app_name}");
                    opened = true;
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        if !opened {
            if let Some(e) = last_err {
                println!("Failed to open {app_name}: {e}");
            } else {
                println!("Failed to open {app_name}: No such file or directory (os error 2)");
            }
        }
    }
}

fn open_folder(folder: &str){
    let mut path=home_dir().expect("Could not find home directory.");
    path.push(folder);
    #[cfg(target_os = "macos")]{
        match Command::new("open").arg(&path).spawn()
        {
            Ok(_) => println!("Opened {}",path.display()),
            Err(e) => println!("Failed to open {}: {e}",path.display())
        }
    }

    #[cfg(target_os = "linux")]
    {
        match Command::new("xdg-open").arg(&path).spawn()
        {
            Ok(_) => println!("Opened {}",path.display()),
            Err(e) => println!("Failed to open {}: {e}",path.display())
        }
    }
}