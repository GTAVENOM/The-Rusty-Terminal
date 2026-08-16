use std::path::PathBuf;
use std::process::Command;

/// The shells a tab/pane can host. WSL distros are detected at startup.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShellKind {
    PowerShell,
    Cmd,
    Wsl(String),
}

impl ShellKind {
    pub fn label(&self) -> String {
        match self {
            ShellKind::PowerShell => "PowerShell".to_string(),
            ShellKind::Cmd => "cmd".to_string(),
            ShellKind::Wsl(distro) => format!("WSL: {distro}"),
        }
    }

    /// Program + args used to spawn this shell under ConPTY.
    pub fn spawn_command(&self) -> (String, Vec<String>) {
        match self {
            ShellKind::PowerShell => {
                ("powershell.exe".to_string(), vec!["-NoLogo".to_string()])
            },
            ShellKind::Cmd => ("cmd.exe".to_string(), vec![]),
            ShellKind::Wsl(distro) => (
                "wsl.exe".to_string(),
                vec!["-d".to_string(), distro.clone()],
            ),
        }
    }

    pub fn backend_settings(
        &self,
        working_directory: Option<PathBuf>,
    ) -> egui_term::BackendSettings {
        let (shell, args) = self.spawn_command();
        let mut env = std::collections::HashMap::new();
        if let ShellKind::Cmd = self {
            // cmd.exe shell integration: PROMPT emits OSC 133 boundary
            // marks + cwd ($e = ESC). No exit codes — cmd limitation.
            env.insert(
                "PROMPT".to_string(),
                crate::learning::shortcuts::CMD_PROMPT_VALUE.to_string(),
            );
        }
        egui_term::BackendSettings {
            shell,
            args,
            working_directory,
            env,
        }
    }

    /// Storage key used in the history DB (`shell` column).
    pub fn db_key(&self) -> String {
        match self {
            ShellKind::PowerShell => "powershell".to_string(),
            ShellKind::Cmd => "cmd".to_string(),
            ShellKind::Wsl(d) => format!("wsl:{d}"),
        }
    }
}

/// All shells available on this system: PowerShell and cmd always, plus any
/// installed WSL distros. Detection is file-presence/CLI based only.
pub fn detect_available_shells() -> Vec<ShellKind> {
    let mut shells = vec![ShellKind::PowerShell, ShellKind::Cmd];
    for distro in detect_wsl_distros() {
        shells.push(ShellKind::Wsl(distro));
    }
    shells
}

/// `wsl.exe -l -q` lists installed distro names, output is UTF-16LE.
fn detect_wsl_distros() -> Vec<String> {
    let output = match Command::new("wsl.exe").args(["-l", "-q"]).output() {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };

    let text = decode_utf16le_lossy(&output.stdout);
    text.lines()
        .map(|l| l.trim().trim_matches('\u{0}').to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn decode_utf16le_lossy(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}
