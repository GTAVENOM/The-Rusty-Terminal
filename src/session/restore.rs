//! Session save/restore: persists tab layout (pane tree, shell kinds,
//! working directories) to `%APPDATA%\RustyTerminal\session.json` on
//! graceful exit and restores on next launch.
//!
//! Scrollback content is NOT restored (v1 scope); only the layout
//! topology, selected shells, and working directories come back.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::terminal::pane::{SplitDir, PaneNode};
use crate::terminal::shell::ShellKind;
use crate::ui::tabs::TabManager;

/// Top-level saved session file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// Schema version — bump on breaking changes to the layout format.
    pub version: u32,
    pub active_tab: Option<usize>,
    pub tabs: Vec<TabData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabData {
    pub title: String,
    pub pane_tree: PaneNodeData,
}

/// Recursively serializable mirror of `PaneNode`, holding only the data
/// that can be persisted (no live terminal state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaneNodeData {
    Leaf {
        shell: ShellKind,
        cwd: Option<PathBuf>,
    },
    Split {
        dir: SplitDir,
        ratio: f32,
        first: Box<PaneNodeData>,
        second: Box<PaneNodeData>,
    },
}

/// Default path: `%APPDATA%\RustyTerminal\session.json`
pub fn session_path() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("RustyTerminal");
    let _ = std::fs::create_dir_all(&dir);
    dir.push("session.json");
    dir
}

/// Serialize the live tab manager into a saveable session.
pub fn save(manager: &TabManager) -> SessionData {
    let tabs: Vec<TabData> = manager
        .tabs
        .iter()
        .map(|(_, tab)| TabData {
            title: tab.title.clone(),
            pane_tree: serialize_node(&tab.root),
        })
        .collect();
    let active_tab = manager
        .active_tab
        .and_then(|id| manager.tabs.keys().position(|k| *k == id));
    SessionData {
        version: 1,
        active_tab,
        tabs,
    }
}

fn serialize_node(node: &PaneNode) -> PaneNodeData {
    match node {
        PaneNode::Empty => PaneNodeData::Leaf {
            shell: ShellKind::PowerShell,
            cwd: None,
        },
        PaneNode::Leaf(pane) => PaneNodeData::Leaf {
            shell: pane.shell.clone(),
            cwd: pane.working_directory.clone(),
        },
        PaneNode::Split {
            dir,
            ratio,
            first,
            second,
        } => PaneNodeData::Split {
            dir: *dir,
            ratio: *ratio,
            first: Box::new(serialize_node(first)),
            second: Box::new(serialize_node(second)),
        },
    }
}

/// Write the session to disk.
pub fn persist(manager: &TabManager, path: &Path) -> std::io::Result<()> {
    let data = save(manager);
    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::write(path, json)
}

/// Load a session from disk.
pub fn load(path: &Path) -> std::io::Result<SessionData> {
    let bytes = std::fs::read(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| std::io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_session_data() {
        let session = SessionData {
            version: 1,
            active_tab: Some(0),
            tabs: vec![TabData {
                title: "PowerShell".to_string(),
                pane_tree: PaneNodeData::Split {
                    dir: SplitDir::Vertical,
                    ratio: 0.6,
                    first: Box::new(PaneNodeData::Leaf {
                        shell: ShellKind::PowerShell,
                        cwd: Some(PathBuf::from("C:\\dev")),
                    }),
                    second: Box::new(PaneNodeData::Leaf {
                        shell: ShellKind::Cmd,
                        cwd: None,
                    }),
                },
            }],
        };
        let json = serde_json::to_string(&session).unwrap();
        let restored: SessionData = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.version, 1);
        assert_eq!(restored.active_tab, Some(0));
        assert_eq!(restored.tabs[0].title, "PowerShell");
        match &restored.tabs[0].pane_tree {
            PaneNodeData::Split { dir, ratio, first, second } => {
                assert_eq!(*dir, SplitDir::Vertical);
                assert!((*ratio - 0.6).abs() < f32::EPSILON);
                assert!(matches!(
                    &**first,
                    PaneNodeData::Leaf { shell: ShellKind::PowerShell, .. }
                ));
                assert!(matches!(
                    &**second,
                    PaneNodeData::Leaf { shell: ShellKind::Cmd, .. }
                ));
            },
            _ => panic!("expected Split"),
        }
    }
}
