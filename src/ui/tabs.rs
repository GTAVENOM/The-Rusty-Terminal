use std::collections::BTreeMap;
use std::sync::mpsc::Sender;

use egui_term::PtyEvent;

use crate::terminal::pane::{Pane, PaneId, PaneNode, SplitDir};
use crate::terminal::shell::ShellKind;

pub type TabId = u64;

pub struct Tab {
    /// Stable identifier. Currently keyed by BTreeMap; the field is
    /// retained as the canonical public API for future tab operations.
    #[allow(dead_code)]
    pub id: TabId,
    pub title: String,
    /// User renamed the tab or the shell set a title via OSC.
    pub title_is_custom: bool,
    pub root: PaneNode,
    pub focused_pane: PaneId,
}

impl Tab {
    pub fn set_title_from_pty(&mut self, title: String) {
        if !self.title_is_custom {
            self.title = title;
        }
    }
}

pub struct TabManager {
    next_tab_id: TabId,
    pub active_tab: Option<TabId>,
    pub tabs: BTreeMap<TabId, Tab>,
}

impl TabManager {
    /// Restore the tab manager from a saved session. Skips tabs whose
    /// PTY fails to spawn (the pane tree is still partially restored);
    /// returns the number of tabs restored.
    pub fn restore_from_session(
        &mut self,
        session: &crate::session::restore::SessionData,
        ctx: egui::Context,
        pty_event_sender: Sender<(u64, PtyEvent)>,
    ) -> usize {
        use crate::session::restore::PaneNodeData;
        use crate::terminal::pane::Pane;

        let mut restored = 0;
        let mut tab_ids: Vec<TabId> = Vec::new();

        for saved_tab in &session.tabs {
            let root = match &saved_tab.pane_tree {
                PaneNodeData::Leaf { shell, cwd } => {
                    match Pane::new(
                        ctx.clone(),
                        pty_event_sender.clone(),
                        shell.clone(),
                        cwd.clone(),
                    ) {
                        Ok(p) => PaneNode::Leaf(p),
                        Err(e) => {
                            log::error!(
                                "failed to restore tab '{}': {e}",
                                saved_tab.title
                            );
                            continue;
                        },
                    }
                },
                saved_tree => {
                    match restore_branch(saved_tree, &ctx, &pty_event_sender)
                    {
                        Some(node) => node,
                        None => continue,
                    }
                },
            };

            let tab_id = self.next_tab_id;
            self.next_tab_id += 1;
            let first_pane = root.first_pane_id().unwrap_or(0);
            self.tabs.insert(
                tab_id,
                Tab {
                    id: tab_id,
                    title: saved_tab.title.clone(),
                    title_is_custom: false,
                    root,
                    focused_pane: first_pane,
                },
            );
            tab_ids.push(tab_id);
            restored += 1;
        }

        if let Some(idx) = session.active_tab {
            self.active_tab = tab_ids.get(idx).copied();
        } else {
            self.active_tab = tab_ids.first().copied();
        }
        restored
    }

    pub fn new() -> Self {
        Self {
            next_tab_id: 1,
            active_tab: None,
            tabs: BTreeMap::new(),
        }
    }

    /// Open a new tab hosting `shell`. Returns None (and shows no tab) if
    /// the PTY failed to spawn.
    pub fn open_tab(
        &mut self,
        ctx: egui::Context,
        pty_event_sender: Sender<(u64, PtyEvent)>,
        shell: ShellKind,
    ) -> Option<TabId> {
        let pane =
            match Pane::new(ctx, pty_event_sender, shell.clone(), None) {
                Ok(p) => p,
                Err(err) => {
                    log::error!("failed to spawn {}: {err}", shell.label());
                    return None;
                },
            };
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let pane_id = pane.id;
        self.tabs.insert(
            tab_id,
            Tab {
                id: tab_id,
                title: shell.label(),
                title_is_custom: false,
                root: PaneNode::Leaf(pane),
                focused_pane: pane_id,
            },
        );
        self.active_tab = Some(tab_id);
        Some(tab_id)
    }

    /// Split the focused pane of the active tab.
    pub fn split_focused(
        &mut self,
        ctx: egui::Context,
        pty_event_sender: Sender<(u64, PtyEvent)>,
        dir: SplitDir,
    ) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        let shell = tab
            .root
            .find_pane_mut(tab.focused_pane)
            .map(|p| p.shell.clone())
            .unwrap_or(ShellKind::PowerShell);
        let cwd = tab
            .root
            .find_pane_mut(tab.focused_pane)
            .and_then(|p| p.working_directory.clone());
        match Pane::new(ctx, pty_event_sender, shell, cwd) {
            Ok(new_pane) => {
                let new_id = new_pane.id;
                tab.root.split(tab.focused_pane, dir, new_pane);
                tab.focused_pane = new_id;
            },
            Err(err) => log::error!("failed to split pane: {err}"),
        }
    }

    /// Remove a pane wherever it lives (PTY exited or user closed it).
    /// Empty tabs are closed.
    pub fn remove_pane(&mut self, pane_id: PaneId) {
        let mut emptied_tab = None;
        for (tab_id, tab) in self.tabs.iter_mut() {
            if tab.root.contains(pane_id) {
                tab.root.remove(pane_id);
                if tab.root.is_empty() {
                    emptied_tab = Some(*tab_id);
                } else if tab.focused_pane == pane_id {
                    if let Some(first) = tab.root.first_pane_id() {
                        tab.focused_pane = first;
                    }
                }
                break;
            }
        }
        if let Some(tab_id) = emptied_tab {
            self.close_tab(tab_id);
        }
    }

    pub fn close_tab(&mut self, tab_id: TabId) {
        self.tabs.remove(&tab_id);
        if self.active_tab == Some(tab_id) {
            self.active_tab = self.tabs.keys().next_back().copied();
        }
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        let id = self.active_tab?;
        self.tabs.get_mut(&id)
    }

    /// Route a PTY event to the tab that owns the pane.
    pub fn on_pty_event(&mut self, pane_id: PaneId, event: PtyEvent) {
        match event {
            PtyEvent::Exit => self.remove_pane(pane_id),
            PtyEvent::Title(title) => {
                for tab in self.tabs.values_mut() {
                    if tab.root.contains(pane_id) {
                        tab.set_title_from_pty(title);
                        break;
                    }
                }
            },
            _ => {},
        }
    }
}

/// Recursively create a live PaneNode from a saved PaneNodeData, spawning
/// real PTYs in the saved working directories.
fn restore_branch(
    data: &crate::session::restore::PaneNodeData,
    ctx: &egui::Context,
    sender: &Sender<(u64, PtyEvent)>,
) -> Option<PaneNode> {
    use crate::session::restore::PaneNodeData;
    use crate::terminal::pane::Pane;

    match data {
        PaneNodeData::Leaf { shell, cwd } => {
            match Pane::new(
                ctx.clone(),
                sender.clone(),
                shell.clone(),
                cwd.clone(),
            ) {
                Ok(pane) => Some(PaneNode::Leaf(pane)),
                Err(err) => {
                    log::error!("restore branch leaf failed: {err}");
                    None
                },
            }
        },
        PaneNodeData::Split {
            dir,
            ratio,
            first,
            second,
        } => {
            let first = restore_branch(first, ctx, sender)?;
            let second = restore_branch(second, ctx, sender)?;
            Some(PaneNode::Split {
                dir: *dir,
                ratio: *ratio,
                first: Box::new(first),
                second: Box::new(second),
            })
        },
    }
}
