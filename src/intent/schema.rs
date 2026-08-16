//! The fixed intent schema. Every intent Rusty can suggest is enumerated
//! here with an authoritative tier — the tier is NEVER taken from the
//! model's output. No intent is added without a tier.
//!
//! Tier 3 intents do not exist by design: the schema is the first gate
//! (the model physically cannot express a destructive intent through it),
//! and the tier classifier re-checks the rendered command at injection
//! time as the backstop.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::safety::tier_classifier::Tier;

/// All intents Rusty understands, with their validated arguments.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    // ---- Dynamic Shell Command (AI Generated) ----
    DynamicShellCommand {
        command: String,
        tier: Tier,
        description: String,
    },
    // ---- Tier 1 (read-only) ----
    ListFiles(ListFilesArgs),
    ClearTerminal,
    GitStatus,
    GitLog(GitLogArgs),
    GitDiff(GitDiffArgs),
    GitBranchList,
    DockerPs(DockerPsArgs),
    DockerLogs(DockerLogsArgs),
    OpenFolder(OpenFolderArgs),
    FindProcessByPort(FindProcessByPortArgs),
    ShowEnvVars(ShowEnvVarsArgs),
    SystemInfo,
    NetworkInfo,
    // ---- Tier 2 (idempotent; require confirmation) ----
    GitPull(GitPullArgs),
    DockerComposeUp(DockerComposeUpArgs),
    DockerPull(DockerPullArgs),
    MakeDirectory(MakeDirectoryArgs),
    GitAdd(GitAddArgs),
    GitCommit(GitCommitArgs),
    GitCheckout(GitCheckoutArgs),
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerPullArgs {
    pub image: String,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MakeDirectoryArgs {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitAddArgs {
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitCommitArgs {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitCheckoutArgs {
    pub branch: String,
}


#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListFilesArgs {
    /// Directory to list; defaults to the current directory.
    pub path: Option<String>,
    /// Include hidden files.
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitLogArgs {
    /// Limit the number of commits shown.
    pub max_count: Option<u32>,
    /// One line per commit.
    #[serde(default)]
    pub oneline: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitDiffArgs {
    /// Compare against this ref (e.g. `HEAD~1`, `main`); omit for
    /// unstaged working tree changes.
    pub base: Option<String>,
    /// Only summarize (`--stat`).
    #[serde(default)]
    pub stat: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerPsArgs {
    /// Include stopped containers.
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerLogsArgs {
    /// Container name or id. Required.
    pub container: String,
    /// Only show the last N lines.
    pub tail: Option<u32>,
    /// Follow the log stream.
    #[serde(default)]
    pub follow: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenFolderArgs {
    /// Folder to open; defaults to the current directory.
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindProcessByPortArgs {
    /// TCP port to look up. Required.
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShowEnvVarsArgs {
    /// Only show variables whose name contains this filter.
    pub filter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitPullArgs {
    /// Remote to pull from (default remote when omitted).
    pub remote: Option<String>,
    /// Branch to pull (current branch when omitted).
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerComposeUpArgs {
    /// Run detached (-d). Defaults to true.
    pub detach: Option<bool>,
    /// Only start this service.
    pub service: Option<String>,
    /// Rebuild images before starting.
    #[serde(default)]
    pub build: bool,
}

impl Intent {
    /// The authoritative intent → tier table. The model has no say here.
    pub fn tier(&self) -> Tier {
        match self {
            Intent::DynamicShellCommand { tier, .. } => *tier,
            Intent::ListFiles(_)
            | Intent::ClearTerminal
            | Intent::GitStatus
            | Intent::GitLog(_)
            | Intent::GitDiff(_)
            | Intent::GitBranchList
            | Intent::DockerPs(_)
            | Intent::DockerLogs(_)
            | Intent::OpenFolder(_)
            | Intent::FindProcessByPort(_)
            | Intent::ShowEnvVars(_)
            | Intent::SystemInfo
            | Intent::NetworkInfo => Tier::ReadOnly,
            Intent::GitPull(_)
            | Intent::DockerComposeUp(_)
            | Intent::DockerPull(_)
            | Intent::MakeDirectory(_)
            | Intent::GitAdd(_)
            | Intent::GitCommit(_)
            | Intent::GitCheckout(_) => Tier::Idempotent,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Intent::DynamicShellCommand { .. } => "dynamic_shell_command",
            Intent::ListFiles(_) => "list_files",
            Intent::ClearTerminal => "clear_terminal",
            Intent::GitStatus => "git_status",
            Intent::GitLog(_) => "git_log",
            Intent::GitDiff(_) => "git_diff",
            Intent::GitBranchList => "git_branch_list",
            Intent::DockerPs(_) => "docker_ps",
            Intent::DockerLogs(_) => "docker_logs",
            Intent::DockerPull(_) => "docker_pull",
            Intent::OpenFolder(_) => "open_folder",
            Intent::FindProcessByPort(_) => "find_process_by_port",
            Intent::ShowEnvVars(_) => "show_env_vars",
            Intent::SystemInfo => "system_info",
            Intent::NetworkInfo => "network_info",
            Intent::GitPull(_) => "git_pull",
            Intent::DockerComposeUp(_) => "docker_compose_up",
            Intent::MakeDirectory(_) => "make_directory",
            Intent::GitAdd(_) => "git_add",
            Intent::GitCommit(_) => "git_commit",
            Intent::GitCheckout(_) => "git_checkout",
        }
    }


    /// Parse a validated intent from a tool_use block's name + input.
    /// Unknown names and schema-violating inputs are errors — nothing is
    /// coerced or guessed.
    pub fn from_tool_use(
        name: &str,
        input: &Value,
    ) -> Result<Intent, IntentParseError> {
        fn de<T: for<'de> Deserialize<'de>>(
            v: &Value,
        ) -> Result<T, IntentParseError> {
            serde_json::from_value(v.clone())
                .map_err(|e| IntentParseError::BadArgs(e.to_string()))
        }
        match name {
            "list_files" => Ok(Intent::ListFiles(de(input)?)),
            "clear_terminal" => Ok(Intent::ClearTerminal),
            "git_status" => Ok(Intent::GitStatus),
            "git_log" => Ok(Intent::GitLog(de(input)?)),
            "git_diff" => Ok(Intent::GitDiff(de(input)?)),
            "git_branch_list" => Ok(Intent::GitBranchList),
            "docker_ps" => Ok(Intent::DockerPs(de(input)?)),
            "docker_logs" => Ok(Intent::DockerLogs(de(input)?)),
            "docker_pull" => Ok(Intent::DockerPull(de(input)?)),
            "open_folder" => Ok(Intent::OpenFolder(de(input)?)),
            "find_process_by_port" => {
                Ok(Intent::FindProcessByPort(de(input)?))
            },
            "show_env_vars" => Ok(Intent::ShowEnvVars(de(input)?)),
            "system_info" => Ok(Intent::SystemInfo),
            "network_info" => Ok(Intent::NetworkInfo),
            "git_pull" => Ok(Intent::GitPull(de(input)?)),
            "docker_compose_up" => Ok(Intent::DockerComposeUp(de(input)?)),
            "make_directory" => Ok(Intent::MakeDirectory(de(input)?)),
            "git_add" => Ok(Intent::GitAdd(de(input)?)),
            "git_commit" => Ok(Intent::GitCommit(de(input)?)),
            "git_checkout" => Ok(Intent::GitCheckout(de(input)?)),
            other => Err(IntentParseError::UnknownIntent(other.to_string())),
        }
    }

}

#[derive(Debug, Clone, PartialEq)]
pub enum IntentParseError {
    UnknownIntent(String),
    BadArgs(String),
}

impl std::fmt::Display for IntentParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntentParseError::UnknownIntent(name) => {
                write!(f, "model returned unknown intent '{name}'")
            },
            IntentParseError::BadArgs(err) => {
                write!(f, "intent arguments failed validation: {err}")
            },
        }
    }
}

/// Which tiers to include in the tools array. Stage (c) ships Tier 1 only;
/// stage (d) adds Tier 2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolsetScope {
    /// Sent by stage (c). Kept for tests and backward compat.
    #[allow(dead_code)]
    Tier1Only,
    Tier1And2,
}

/// The `tools` array for the Anthropic Messages API call. Every tool is
/// strict (`additionalProperties: false`); descriptions steer the model
/// toward relevant intents based on project context.
pub fn tool_definitions(scope: ToolsetScope) -> Vec<Value> {
    let mut tools = vec![
        json!({
            "name": "list_files",
            "description": "List files in a directory (read-only).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {"type": "string", "description": "Directory to list; omit for the current directory."},
                    "all": {"type": "boolean", "description": "Include hidden files."}
                }
            }
        }),
        json!({
            "name": "git_status",
            "description": "Show git working-tree status (read-only).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }
        }),
        json!({
            "name": "git_log",
            "description": "Show git commit history (read-only).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "max_count": {"type": "integer", "minimum": 1, "description": "Max commits to show."},
                    "oneline": {"type": "boolean", "description": "One line per commit."}
                }
            }
        }),
        json!({
            "name": "git_diff",
            "description": "Show git diff of working-tree or against a ref (read-only).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "base": {"type": "string", "description": "Ref to diff against, e.g. HEAD~1."},
                    "stat": {"type": "boolean", "description": "Show a summary --stat only."}
                }
            }
        }),
        json!({
            "name": "git_branch_list",
            "description": "List git branches (read-only).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }
        }),
        json!({
            "name": "docker_ps",
            "description": "List docker containers (read-only).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "all": {"type": "boolean", "description": "Include stopped containers."}
                }
            }
        }),
        json!({
            "name": "docker_logs",
            "description": "Show logs of a docker container (read-only).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["container"],
                "properties": {
                    "container": {"type": "string", "description": "Container name or id."},
                    "tail": {"type": "integer", "minimum": 1, "description": "Only the last N lines."},
                    "follow": {"type": "boolean", "description": "Follow the log stream."}
                }
            }
        }),
        json!({
            "name": "open_folder",
            "description": "Open a folder in the Windows file explorer (read-only).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {"type": "string", "description": "Folder to open; omit for the current directory."}
                }
            }
        }),
        json!({
            "name": "find_process_by_port",
            "description": "Show which process is listening on a TCP port (read-only).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["port"],
                "properties": {
                    "port": {"type": "integer", "minimum": 1, "maximum": 65535, "description": "TCP port number."}
                }
            }
        }),
        json!({
            "name": "show_env_vars",
            "description": "Show environment variables (read-only).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "filter": {"type": "string", "description": "Only variables whose name contains this text."}
                }
            }
        }),
    ];

    if scope == ToolsetScope::Tier1And2 {
        tools.push(json!({
            "name": "git_pull",
            "description": "Pull latest changes from a git remote (state-changing but idempotent; requires user confirmation).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "remote": {"type": "string", "description": "Remote name, e.g. origin."},
                    "branch": {"type": "string", "description": "Branch to pull."}
                }
            }
        }));
        tools.push(json!({
            "name": "docker_compose_up",
            "description": "Start services defined in docker-compose.yml (state-changing but idempotent; requires user confirmation).",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "detach": {"type": "boolean", "description": "Run detached (-d). Defaults to true."},
                    "service": {"type": "string", "description": "Only start this service."},
                    "build": {"type": "boolean", "description": "Rebuild images before starting."}
                }
            }
        }));
    }

    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_tool_use_parses() {
        let intent = Intent::from_tool_use(
            "docker_logs",
            &json!({"container": "api", "tail": 50}),
        )
        .unwrap();
        assert_eq!(
            intent,
            Intent::DockerLogs(DockerLogsArgs {
                container: "api".to_string(),
                tail: Some(50),
                follow: false,
            })
        );
    }

    #[test]
    fn unknown_intent_is_rejected() {
        let err = Intent::from_tool_use("delete_everything", &json!({}))
            .unwrap_err();
        assert!(matches!(err, IntentParseError::UnknownIntent(_)));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        // deny_unknown_fields: a model hallucinating extra args fails
        // validation rather than being silently accepted.
        let err = Intent::from_tool_use(
            "git_log",
            &json!({"max_count": 5, "force": true}),
        )
        .unwrap_err();
        assert!(matches!(err, IntentParseError::BadArgs(_)));
    }

    #[test]
    fn missing_required_field_is_rejected() {
        let err =
            Intent::from_tool_use("docker_logs", &json!({})).unwrap_err();
        assert!(matches!(err, IntentParseError::BadArgs(_)));

        let err = Intent::from_tool_use("find_process_by_port", &json!({}))
            .unwrap_err();
        assert!(matches!(err, IntentParseError::BadArgs(_)));
    }

    #[test]
    fn wrong_type_is_rejected() {
        let err = Intent::from_tool_use(
            "find_process_by_port",
            &json!({"port": "eighty"}),
        )
        .unwrap_err();
        assert!(matches!(err, IntentParseError::BadArgs(_)));
    }

    #[test]
    fn every_intent_has_a_tier() {
        // The tier table is total: constructing each variant and asking
        // for its tier must never panic, and Tier 3 must be unreachable.
        let intents = vec![
            Intent::ListFiles(Default::default()),
            Intent::GitStatus,
            Intent::GitLog(Default::default()),
            Intent::GitDiff(Default::default()),
            Intent::GitBranchList,
            Intent::DockerPs(Default::default()),
            Intent::DockerLogs(DockerLogsArgs {
                container: "x".into(),
                tail: None,
                follow: false,
            }),
            Intent::OpenFolder(Default::default()),
            Intent::FindProcessByPort(FindProcessByPortArgs { port: 80 }),
            Intent::ShowEnvVars(Default::default()),
            Intent::GitPull(Default::default()),
            Intent::DockerComposeUp(Default::default()),
        ];
        for intent in intents {
            let tier = intent.tier();
            assert_ne!(
                tier,
                crate::safety::tier_classifier::Tier::Destructive,
                "no schema intent may be Tier 3"
            );
        }
    }

    #[test]
    fn tier1_scope_excludes_tier2_tools() {
        let t1 = tool_definitions(ToolsetScope::Tier1Only);
        assert!(t1.iter().all(|t| {
            let name = t["name"].as_str().unwrap();
            name != "git_pull" && name != "docker_compose_up"
        }));
        assert_eq!(t1.len(), 10);

        let t12 = tool_definitions(ToolsetScope::Tier1And2);
        assert_eq!(t12.len(), 12);
    }
}
