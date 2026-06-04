use crate::actions::CommandAction;
use crate::parser::CommandParser;
use serde::{Deserialize,Serialize};
use std::time::Duration;

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
            1. For GUI applications (e.g., vscode, chrome, discord):\n\
               {{\"action\": \"OpenApp\", \"name\": \"Application Name\"}}\n\n\
            2. For directories/folders (e.g., downloads, desktop, coding, or paths):\n\
               {{\"action\": \"OpenFolder\", \"path\": \"~/path/to/folder\"}}\n\n\
            3. For exiting or closing the terminal:\n\
               {{\"action\": \"Exit\"}}\n\n\
            Rules:\n\
            - Output ONLY the raw JSON object.\n\
            - Do NOT wrap the JSON in other keys (like \"userInput\" or \"parsedJSON\").\n\
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
                    println!("DEBUG: Raw LLM Response: {}", ollama_res.response);
                    match serde_json::from_str::<CommandAction>(&ollama_res.response) {
                        Ok(mut action)=>{
                            if let CommandAction::OpenFolder {ref mut path}=action{
                                if path.starts_with("~"){
                                    if let Ok(suffix)=path.strip_prefix("~"){
                                        if let Some(mut home)=dirs::home_dir(){
                                            home.push(suffix);
                                            *path=home;
                                        }
                                    }
                                }
                            }
                            action
                        },
                        Err(e)=>{
                            eprintln!("Failed to parse LLM response into CommandAction: {}", e);
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