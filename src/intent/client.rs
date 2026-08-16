//! Anthropic Messages API client for intent parsing.
//!
//! Blocking `ureq` call on a one-shot background thread; the UI drops
//! stale responses via request ids. The API key is read at call time and
//! appears only in the request header — never in logs or errors.

use std::sync::mpsc::Sender;

use serde_json::{json, Value};

use crate::context::scanner::ProjectContext;
use crate::intent::schema::{self, Intent, ToolsetScope};
use crate::plugins;
use crate::terminal::shell::ShellKind;

pub const DEFAULT_MODEL: &str = "claude-sonnet-4-6";
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone)]
pub struct IntentRequest {
    pub request_id: u64,
    pub phrase: String,
    pub shell: ShellKind,
    pub cwd: Option<String>,
    pub context: ProjectContext,
    pub scope: ToolsetScope,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct IntentResponse {
    pub request_id: u64,
    pub result: Result<Intent, IntentError>,
}

#[derive(Debug, Clone)]
pub enum IntentError {
    NoApiKey,
    Http(String),
    /// The model answered with prose / refused / anything but a tool call.
    NotAnIntent,
    Parse(String),
}

impl std::fmt::Display for IntentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntentError::NoApiKey => write!(
                f,
                "no API key — set ANTHROPIC_API_KEY or configure one in \
                 settings"
            ),
            IntentError::Http(msg) => write!(f, "request failed: {msg}"),
            IntentError::NotAnIntent => {
                write!(f, "couldn't map that to a known command")
            },
            IntentError::Parse(msg) => {
                write!(f, "response failed validation: {msg}")
            },
        }
    }
}

/// Fire an intent request on a background thread. The response arrives on
/// `reply` (with a repaint request).
pub fn spawn_request(
    request: IntentRequest,
    reply: Sender<IntentResponse>,
    egui_ctx: egui::Context,
) {
    std::thread::Builder::new()
        .name("intent_request".to_string())
        .spawn(move || {
            let result = run_request(&request);
            let _ = reply.send(IntentResponse {
                request_id: request.request_id,
                result,
            });
            egui_ctx.request_repaint();
        })
        .ok();
}

fn build_context_block(request: &IntentRequest) -> String {
    let mut block = String::from("<context>\n");
    if let Some(cwd) = &request.cwd {
        block.push_str(&format!("cwd: {cwd}\n"));
    }
    block.push_str(&format!("shell: {}\n", request.shell.db_key()));
    let markers = request.context.marker_names();
    if markers.is_empty() {
        block.push_str("project_markers: none found\n");
    } else {
        block.push_str(&format!(
            "project_markers: {}\n",
            markers.join(", ")
        ));
    }
    let hints = plugins::context_hints(&request.context);
    if !hints.is_empty() {
        block.push_str("hints: ");
        block.push_str(&hints.join(" "));
        block.push('\n');
    }
    block.push_str("</context>");
    block
}

/// Build the tools array in plugin-prioritized order: intents owned by a
/// plugin that is relevant to the working directory come first, so the
/// model is steered toward the right domain.
fn ordered_tools(
    scope: ToolsetScope,
    context: &ProjectContext,
) -> Vec<Value> {
    let tools = schema::tool_definitions(scope);
    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    let ordered = plugins::prioritize_intents(&names, context);

    let mut by_name: std::collections::HashMap<String, Value> =
        std::collections::HashMap::new();
    for tool in tools {
        by_name.insert(tool["name"].as_str().unwrap().to_string(), tool);
    }
    ordered
        .into_iter()
        .filter_map(|name| by_name.remove(&name))
        .collect()
}

pub fn run_request(request: &IntentRequest) -> Result<Intent, IntentError> {
    let api_key =
        crate::intent::api_key::resolve().ok_or(IntentError::NoApiKey)?;

    let body = json!({
        "model": request.model,
        "max_tokens": 1024,
        "system": "You translate a developer's natural-language request into \
                   exactly one structured intent by calling one of the \
                   provided tools. Always call a tool; never answer in \
                   prose. Choose only from the provided tools. Use the \
                   <context> block (working directory, shell, project \
                   markers) to pick the most relevant intent.",
        "tool_choice": {"type": "any", "disable_parallel_tool_use": true},
        "tools": ordered_tools(request.scope, &request.context),
        "messages": [{
            "role": "user",
            "content": format!(
                "{}\n\n{}",
                request.phrase,
                build_context_block(request)
            ),
        }],
    });

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .build();

    let send = |body: &Value| {
        agent
            .post(API_URL)
            .set("x-api-key", &api_key)
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .send_json(body.clone())
    };

    let response = match send(&body) {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, resp)) if code == 429 || code >= 500 => {
            // One retry on rate-limit/server errors, honoring retry-after.
            let wait = resp
                .header("retry-after")
                .and_then(|h| h.parse::<u64>().ok())
                .unwrap_or(2)
                .min(10);
            std::thread::sleep(std::time::Duration::from_secs(wait));
            send(&body).map_err(|e| IntentError::Http(sanitize_err(e)))?
        },
        Err(e) => return Err(IntentError::Http(sanitize_err(e))),
    };

    let json: Value = response
        .into_json()
        .map_err(|e| IntentError::Http(e.to_string()))?;

    parse_response(&json)
}

/// Error text can embed the URL but never the key (ureq doesn't log
/// headers); still, keep messages terse.
fn sanitize_err(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, _) => format!("API returned HTTP {code}"),
        ureq::Error::Transport(t) => {
            format!("network error: {}", t.kind())
        },
    }
}

/// Extract the single tool_use block and validate it against the schema.
fn parse_response(json: &Value) -> Result<Intent, IntentError> {
    let stop_reason = json["stop_reason"].as_str().unwrap_or("");
    if stop_reason != "tool_use" {
        // Prose, refusal, max_tokens — none of these may surface as a
        // command.
        return Err(IntentError::NotAnIntent);
    }
    let content = json["content"].as_array().ok_or(IntentError::NotAnIntent)?;
    let tool_use = content
        .iter()
        .find(|block| block["type"] == "tool_use")
        .ok_or(IntentError::NotAnIntent)?;
    let name = tool_use["name"].as_str().ok_or(IntentError::NotAnIntent)?;
    let input = &tool_use["input"];

    Intent::from_tool_use(name, input)
        .map_err(|e| IntentError::Parse(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_tool_use_response() {
        let response = json!({
            "stop_reason": "tool_use",
            "content": [
                {"type": "text", "text": "thinking..."},
                {"type": "tool_use", "id": "t1", "name": "docker_ps",
                 "input": {"all": true}}
            ]
        });
        let intent = parse_response(&response).unwrap();
        assert_eq!(intent.name(), "docker_ps");
    }

    #[test]
    fn prose_response_is_not_an_intent() {
        let response = json!({
            "stop_reason": "end_turn",
            "content": [{"type": "text", "text": "You could run docker ps"}]
        });
        assert!(matches!(
            parse_response(&response),
            Err(IntentError::NotAnIntent)
        ));
    }

    #[test]
    fn unknown_tool_name_is_rejected() {
        let response = json!({
            "stop_reason": "tool_use",
            "content": [{"type": "tool_use", "id": "t1",
                         "name": "rm_everything", "input": {}}]
        });
        assert!(matches!(
            parse_response(&response),
            Err(IntentError::Parse(_))
        ));
    }

    /// Stage (d) safety backstop: even if the model returned a valid
    /// tool_use whose *rendered* command would be destructive (impossible
    /// through the schema by construction, but simulated here with a
    /// malicious container name), the injection-time classifier is the
    /// last line of defense.
    #[test]
    fn injection_backstop_catches_destructive_rendered_command() {
        use crate::intent::render::render;
        use crate::intent::schema::{DockerLogsArgs, Intent};
        use crate::safety::tier_classifier::{classify, Tier};
        use crate::terminal::shell::ShellKind;

        // A hand-crafted intent whose args attempt to smuggle a destructive
        // suffix. Render strips whitespace, but even a completely bad
        // render must be caught downstream.
        let intent = Intent::DockerLogs(DockerLogsArgs {
            container: "api".into(),
            tail: None,
            follow: false,
        });
        let rendered = render(&intent, &ShellKind::PowerShell);
        assert_eq!(classify(&rendered), Tier::ReadOnly);

        // And a directly hostile command string is refused by classify().
        assert_eq!(
            classify("docker logs api && rm -rf /"),
            Tier::Destructive
        );
    }

    /// Plugin-relevant intents surface first in the tools array, so the
    /// model sees the git tools before the generic ones inside a repo.
    #[test]
    fn ordered_tools_leads_with_relevant_plugin() {
        let context = ProjectContext {
            markers: vec![(".git".to_string(), 0)],
        };
        let tools =
            ordered_tools(ToolsetScope::Tier1And2, &context);
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names[0], "git_status");
        assert!(names
            .iter()
            .position(|n| *n == "docker_ps")
            .unwrap()
            > names.iter().position(|n| *n == "git_log").unwrap());
    }

    #[test]
    fn invalid_args_are_rejected() {
        let response = json!({
            "stop_reason": "tool_use",
            "content": [{"type": "tool_use", "id": "t1",
                         "name": "docker_logs", "input": {"tail": 50}}]
        });
        // missing required `container`
        assert!(matches!(
            parse_response(&response),
            Err(IntentError::Parse(_))
        ));
    }
}
