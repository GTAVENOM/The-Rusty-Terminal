//! Intent → exact shell command. The output of this module is what the
//! user sees and what gets inserted into the input line: always the
//! literal command, never a paraphrase.

use crate::intent::schema::Intent;
use crate::terminal::shell::ShellKind;

/// Quote a path argument for the target shell if it needs it.
fn quote_path(path: &str, shell: &ShellKind) -> String {
    let needs_quotes =
        path.contains(' ') || path.contains('(') || path.contains(')');
    if !needs_quotes {
        return path.to_string();
    }
    match shell {
        ShellKind::PowerShell | ShellKind::Wsl(_) => {
            format!("'{}'", path.replace('\'', "''"))
        },
        ShellKind::Cmd => format!("\"{path}\""),
    }
}

/// Render the exact shell command for an intent on a given shell.
pub fn render(intent: &Intent, shell: &ShellKind) -> String {
    match intent {
        Intent::DynamicShellCommand { command, .. } => command.clone(),
        Intent::ListFiles(args) => {
            let path = args
                .path
                .as_deref()
                .map(|p| format!(" {}", quote_path(p, shell)));
            match shell {
                ShellKind::PowerShell => format!(
                    "Get-ChildItem{}{}",
                    if args.all { " -Force" } else { "" },
                    path.unwrap_or_default()
                ),
                ShellKind::Cmd => format!(
                    "dir{}{}",
                    if args.all { " /a" } else { "" },
                    path.unwrap_or_default()
                ),
                ShellKind::Wsl(_) => format!(
                    "ls -l{}{}",
                    if args.all { "a" } else { "" },
                    path.unwrap_or_default()
                ),
            }
        },
        Intent::GitStatus => "git status".to_string(),
        Intent::GitLog(args) => {
            let mut cmd = "git log".to_string();
            if args.oneline {
                cmd.push_str(" --oneline");
            }
            if let Some(n) = args.max_count {
                cmd.push_str(&format!(" -{n}"));
            }
            cmd
        },
        Intent::GitDiff(args) => {
            let mut cmd = "git diff".to_string();
            if let Some(base) = &args.base {
                let base: String = base
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                cmd.push_str(&format!(" {base}"));
            }
            if args.stat {
                cmd.push_str(" --stat");
            }
            cmd
        },
        Intent::GitBranchList => "git branch".to_string(),
        Intent::DockerPs(args) => {
            if args.all {
                "docker ps -a".to_string()
            } else {
                "docker ps".to_string()
            }
        },
        Intent::DockerLogs(args) => {
            let mut cmd = "docker logs".to_string();
            if let Some(tail) = args.tail {
                cmd.push_str(&format!(" --tail {tail}"));
            }
            if args.follow {
                cmd.push_str(" -f");
            }
            // Container names are restricted charsets; strip whitespace to
            // keep the rendered command a single argument.
            let container: String = args
                .container
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            cmd.push_str(&format!(" {container}"));
            cmd
        },
        Intent::DockerPull(args) => {
            let image: String = args.image.chars().filter(|c| !c.is_whitespace()).collect();
            format!("docker pull {image}")
        },
        Intent::OpenFolder(args) => {
            let path = args.path.as_deref().unwrap_or(".");
            match shell {
                ShellKind::Wsl(_) => {
                    format!("explorer.exe {}", quote_path(path, shell))
                },
                _ => format!("explorer {}", quote_path(path, shell)),
            }
        },
        Intent::FindProcessByPort(args) => match shell {
            ShellKind::PowerShell => format!(
                "Get-NetTCPConnection -LocalPort {} | Select-Object -Property LocalPort,State,OwningProcess",
                args.port
            ),
            ShellKind::Cmd => {
                format!("netstat -ano | findstr :{}", args.port)
            },
            ShellKind::Wsl(_) => format!("ss -tlnp | grep :{}", args.port),
        },
        Intent::ShowEnvVars(args) => match shell {
            ShellKind::PowerShell => match &args.filter {
                Some(f) => {
                    let filter: String = f
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    format!("Get-ChildItem Env: | Where-Object Name -like '*{filter}*'")
                },
                None => "Get-ChildItem Env:".to_string(),
            },
            ShellKind::Cmd => match &args.filter {
                Some(f) => {
                    let filter: String = f
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    format!("set | findstr /i {filter}")
                },
                None => "set".to_string(),
            },
            ShellKind::Wsl(_) => match &args.filter {
                Some(f) => {
                    let filter: String = f
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    format!("env | grep -i {filter}")
                },
                None => "env".to_string(),
            },
        },
        Intent::GitPull(args) => {
            let mut cmd = "git pull".to_string();
            if let Some(remote) = &args.remote {
                let remote: String = remote
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                cmd.push_str(&format!(" {remote}"));
                if let Some(branch) = &args.branch {
                    let branch: String = branch
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect();
                    cmd.push_str(&format!(" {branch}"));
                }
            }
            cmd
        },
        Intent::ClearTerminal => match shell {
            ShellKind::PowerShell => "Clear-Host".to_string(),
            ShellKind::Cmd => "cls".to_string(),
            ShellKind::Wsl(_) => "clear".to_string(),
        },
        Intent::SystemInfo => match shell {
            ShellKind::PowerShell => "Get-ComputerInfo".to_string(),
            ShellKind::Cmd => "systeminfo".to_string(),
            ShellKind::Wsl(_) => "uname -a".to_string(),
        },
        Intent::NetworkInfo => match shell {
            ShellKind::PowerShell => "Get-NetIPAddress".to_string(),
            ShellKind::Cmd => "ipconfig".to_string(),
            ShellKind::Wsl(_) => "ip a".to_string(),
        },
        Intent::MakeDirectory(args) => {
            let name = quote_path(&args.name, shell);
            match shell {
                ShellKind::PowerShell => format!("New-Item -ItemType Directory -Name {name}"),
                ShellKind::Cmd => format!("mkdir {name}"),
                ShellKind::Wsl(_) => format!("mkdir -p {name}"),
            }
        },
        Intent::GitAdd(args) => {
            let path = args.path.as_deref().unwrap_or(".");
            format!("git add {path}")
        },
        Intent::GitCommit(args) => {
            format!("git commit -m \"{}\"", args.message.replace('"', "\\\""))
        },
        Intent::GitCheckout(args) => {
            let branch: String = args.branch.chars().filter(|c| !c.is_whitespace()).collect();
            format!("git checkout {branch}")
        },
        Intent::DockerComposeUp(args) => {
            let mut cmd = "docker compose up".to_string();
            if args.detach.unwrap_or(true) {
                cmd.push_str(" -d");
            }
            if args.build {
                cmd.push_str(" --build");
            }
            if let Some(service) = &args.service {
                let service: String = service
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                cmd.push_str(&format!(" {service}"));
            }
            cmd
        },

    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::schema::*;
    use crate::safety::tier_classifier::{classify, Tier};

    #[test]
    fn list_files_per_shell() {
        let intent = Intent::ListFiles(ListFilesArgs {
            path: None,
            all: true,
        });
        assert_eq!(
            render(&intent, &ShellKind::PowerShell),
            "Get-ChildItem -Force"
        );
        assert_eq!(render(&intent, &ShellKind::Cmd), "dir /a");
        assert_eq!(
            render(&intent, &ShellKind::Wsl("Ubuntu".into())),
            "ls -la"
        );
    }

    #[test]
    fn docker_logs_renders_flags() {
        let intent = Intent::DockerLogs(DockerLogsArgs {
            container: "api".into(),
            tail: Some(100),
            follow: true,
        });
        assert_eq!(
            render(&intent, &ShellKind::PowerShell),
            "docker logs --tail 100 -f api"
        );
    }

    #[test]
    fn paths_with_spaces_are_quoted() {
        let intent = Intent::ListFiles(ListFilesArgs {
            path: Some("C:\\My Projects".into()),
            all: false,
        });
        assert_eq!(
            render(&intent, &ShellKind::PowerShell),
            "Get-ChildItem 'C:\\My Projects'"
        );
        assert_eq!(
            render(&intent, &ShellKind::Cmd),
            "dir \"C:\\My Projects\""
        );
    }

    #[test]
    fn port_lookup_per_shell() {
        let intent =
            Intent::FindProcessByPort(FindProcessByPortArgs { port: 3000 });
        assert!(render(&intent, &ShellKind::PowerShell)
            .starts_with("Get-NetTCPConnection -LocalPort 3000"));
        assert_eq!(
            render(&intent, &ShellKind::Cmd),
            "netstat -ano | findstr :3000"
        );
    }

    #[test]
    fn container_names_cannot_smuggle_commands() {
        // Whitespace is stripped from model-supplied identifiers so the
        // rendered command cannot grow extra arguments/operators.
        let intent = Intent::DockerLogs(DockerLogsArgs {
            container: "api; rm -rf /".into(),
            tail: None,
            follow: false,
        });
        let rendered = render(&intent, &ShellKind::PowerShell);
        assert_eq!(rendered, "docker logs api;rm-rf/");
        // And even if something slipped, the injection-time classifier
        // still catches compound destructive commands:
        assert_eq!(classify("docker logs api; rm -rf /"), Tier::Destructive);
    }

    #[test]
    fn every_rendered_tier1_intent_classifies_read_only() {
        let cases = vec![
            Intent::ListFiles(Default::default()),
            Intent::GitStatus,
            Intent::GitLog(GitLogArgs {
                max_count: Some(10),
                oneline: true,
            }),
            Intent::GitDiff(GitDiffArgs {
                base: Some("HEAD~1".into()),
                stat: true,
            }),
            Intent::GitBranchList,
            Intent::DockerPs(DockerPsArgs { all: true }),
            Intent::DockerLogs(DockerLogsArgs {
                container: "api".into(),
                tail: Some(50),
                follow: false,
            }),
            Intent::OpenFolder(Default::default()),
            Intent::FindProcessByPort(FindProcessByPortArgs { port: 8080 }),
            Intent::ShowEnvVars(Default::default()),
        ];
        for shell in [
            ShellKind::PowerShell,
            ShellKind::Cmd,
            ShellKind::Wsl("Ubuntu".into()),
        ] {
            for intent in &cases {
                let rendered = render(intent, &shell);
                assert_eq!(
                    classify(&rendered),
                    Tier::ReadOnly,
                    "intent {} rendered as `{rendered}` on {shell:?} must \
                     classify Tier 1",
                    intent.name()
                );
            }
        }
    }

    #[test]
    fn every_rendered_tier2_intent_classifies_idempotent() {
        let cases = vec![
            Intent::GitPull(GitPullArgs {
                remote: Some("origin".into()),
                branch: Some("main".into()),
            }),
            Intent::DockerComposeUp(DockerComposeUpArgs {
                detach: Some(true),
                service: None,
                build: false,
            }),
        ];
        for shell in [ShellKind::PowerShell, ShellKind::Cmd] {
            for intent in &cases {
                let rendered = render(intent, &shell);
                assert_eq!(
                    classify(&rendered),
                    Tier::Idempotent,
                    "`{rendered}` must classify Tier 2"
                );
            }
        }
    }
}
