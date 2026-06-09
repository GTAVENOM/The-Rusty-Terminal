use crate::actions::CommandAction;
use crate::parser::CommandParser;
use serde::{Deserialize,Serialize};
use std::time::Duration;
use crate::actions::CommandAction::Open;

pub struct AiParser{
    model_name: String,
    ollama_url: String,
}

#[derive(Serialize)]
struct OllamaRequest<'a>{
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    format: &'a str,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions{
    temperature: f32,
}

#[derive(Deserialize)]
struct OllamaResponse{
    response: String,
}

impl AiParser{
    pub fn new(model_name: &str)->Self{
        Self{
            model_name: model_name.to_string(),
            ollama_url: "http://localhost:11434/api/generate".to_string(),
        }
    }


}

impl CommandParser for AiParser{
    fn parse(&self, input: &str)->CommandAction{
        let system_prompt=format!("<instruction>\n\
            You are a system parser. Translate the user input into a JSON matching one of these exact schemas:\n\n\
            1. For opening apps, folders, or files:\n\
               {{\"action\": \"Open\", \"target\": \"target_name\"}}\n\n\
            2. For exiting or closing the terminal:\n\
               {{\"action\": \"Exit\"}}\n\n\
            Rules:\n\
            - Output ONLY the raw JSON object.\n\
            - Do NOT wrap the JSON in other keys.\n\
            - Never add comments or conversational text.\n\
            </instruction>\n\n\
            Input: \"{}\"\n\
            JSON:",input);

        let request_payload=OllamaRequest{
            model: &self.model_name,
            prompt: &system_prompt,
            stream: false,
            format: "json",
            options: OllamaOptions{temperature: 0.0},
        };

        let client=reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).build();

        let client=match client{
            Ok(c)=>c,
            Err(_)=>return CommandAction::Unknown
        };

        let response=client.post(&self.ollama_url).json(&request_payload).send();

        match response{
            Ok(res)=>{
                if let Ok(ollama_res)= res.json::<OllamaResponse>(){
                    match serde_json::from_str::<CommandAction>(&ollama_res.response) {
                        Ok(action)=>{
                            match action{
                                CommandAction::Open{target}=>{
                                    let mut resolved_path=if target.starts_with("~/"){
                                        dirs::home_dir().map(|mut h|{
                                            h.push(&target[2..]);
                                            h
                                        })
                                    }else if target=="~"{
                                        dirs::home_dir()
                                    }else{
                                        Some(std::path::PathBuf::from(&target))
                                    };

                                    if let Some(ref path)=resolved_path{
                                        if !path.exists(){
                                            let folder_lower=target.to_lowercase();
                                            if matches!(folder_lower.as_str(),"downloads"|"coding"|"documents"|"desktop"){
                                                let folder_name=match folder_lower.as_str(){
                                                    "downloads"=>"Downloads",
                                                    "coding"=>"Coding",
                                                    "documents"=>"Documents",
                                                    "desktop"=>"Desktop",
                                                    _=>unreachable!(),
                                                };

                                                if let Some(mut home)=dirs::home_dir(){
                                                    home.push(folder_name);
                                                    resolved_path=Some(home);
                                                }
                                            }
                                        }
                                    }
                                    if let Some(mut path)=resolved_path{
                                        if path.exists(){
                                            CommandAction::OpenFolder {path}
                                        }
                                        else{
                                            CommandAction::OpenApp {name:target}
                                        }
                                    }
                                    else{
                                        CommandAction::OpenApp {name: target}
                                    }
                                }
                                other=>other,
                            }
                        }
                        Err(e)=>{
                            eprintln!("Failed to parse LLM response: {}",e);
                            CommandAction::Unknown
                        }
                    }
                }
                else{
                    CommandAction::Unknown
                }
            }
            Err(_)=> {
                eprintln!("\n⚠️ Warning: Could not connect to local Ollama server. Is it running? (run 'ollama serve')");

                CommandAction::Unknown
            }
        }
    }
}