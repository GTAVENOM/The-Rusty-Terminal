use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

use egui_term::{PtyEvent, TerminalBackend};

use super::input_gate::{self, InputGateHandle};
use super::shell::ShellKind;

/// Globally unique pane identifier (also used as the PTY backend id).
pub type PaneId = u64;

static NEXT_PANE_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_pane_id() -> PaneId {
    NEXT_PANE_ID.fetch_add(1, Ordering::SeqCst)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SplitDir {
    Horizontal,
    Vertical,
}

/// A binary tree of splits; leaves are terminal panes. `Empty` exists only
/// transiently during tree surgery and is never left in the tree.
pub enum PaneNode {
    Empty,
    Leaf(Pane),
    Split {
        dir: SplitDir,
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

pub struct Pane {
    pub id: PaneId,
    pub shell: ShellKind,
    pub working_directory: Option<PathBuf>,
    pub backend: TerminalBackend,
    pub input_gate: InputGateHandle,
}

impl Pane {
    pub fn new(
        ctx: egui::Context,
        pty_event_sender: Sender<(u64, PtyEvent)>,
        shell: ShellKind,
        working_directory: Option<PathBuf>,
    ) -> std::io::Result<Self> {
        let id = next_pane_id();
        let mut backend = TerminalBackend::new(
            id,
            ctx,
            pty_event_sender,
            shell.backend_settings(working_directory.clone()),
        )?;

        // Every byte written to this PTY flows through the input gate.
        let gate = input_gate::new_gate();
        let observer_gate = gate.clone();
        backend.set_input_observer(Box::new(move |bytes, scraped_line| {
            if let Ok(mut g) = observer_gate.lock() {
                g.observe(bytes, scraped_line);
            }
        }));

        Ok(Self {
            id,
            shell,
            working_directory,
            backend,
            input_gate: gate,
        })
    }

    /// Insert text into this pane's input line. Never appends a newline —
    /// executing is always the user's own keystroke.
    pub fn insert_text(&mut self, text: &str) {
        debug_assert!(
            !text.contains('\r') && !text.contains('\n'),
            "insert_text must never carry a newline"
        );
        let sanitized: String = text
            .chars()
            .filter(|c| *c != '\r' && *c != '\n')
            .collect();
        self.backend.process_command(egui_term::BackendCommand::Write(
            sanitized.into_bytes(),
        ));
    }

    /// Write a suggested command plus Enter — the ONLY code path in the
    /// program that appends `\r` to programmatically-injected text.
    /// Requires a `ConfirmationToken`, which is minted only by the confirm
    /// dialog's Run handler, so calling this without user confirmation is
    /// a type error.
    pub fn run_confirmed(
        &mut self,
        command: &str,
        _token: crate::terminal::input_gate::ConfirmationToken,
    ) {
        // Sanitize: reject embedded newlines so a single Run press cannot
        // execute a second command.
        let sanitized: String = command
            .chars()
            .filter(|c| *c != '\r' && *c != '\n')
            .collect();
        let mut bytes = sanitized.into_bytes();
        bytes.push(b'\r');
        self.backend
            .process_command(egui_term::BackendCommand::Write(bytes));
    }
}

impl PaneNode {
    /// Find a mutable reference to the leaf pane with the given id.
    pub fn find_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        match self {
            PaneNode::Empty => None,
            PaneNode::Leaf(pane) => (pane.id == id).then_some(pane),
            PaneNode::Split { first, second, .. } => first
                .find_pane_mut(id)
                .or_else(|| second.find_pane_mut(id)),
        }
    }

    pub fn first_pane_id(&self) -> Option<PaneId> {
        match self {
            PaneNode::Empty => None,
            PaneNode::Leaf(pane) => Some(pane.id),
            PaneNode::Split { first, .. } => first.first_pane_id(),
        }
    }

    pub fn pane_ids(&self) -> Vec<PaneId> {
        match self {
            PaneNode::Empty => vec![],
            PaneNode::Leaf(pane) => vec![pane.id],
            PaneNode::Split { first, second, .. } => {
                let mut ids = first.pane_ids();
                ids.extend(second.pane_ids());
                ids
            },
        }
    }

    pub fn contains(&self, id: PaneId) -> bool {
        match self {
            PaneNode::Empty => false,
            PaneNode::Leaf(pane) => pane.id == id,
            PaneNode::Split { first, second, .. } => {
                first.contains(id) || second.contains(id)
            },
        }
    }

    #[allow(dead_code)]
    pub fn pane_count(&self) -> usize {
        match self {
            PaneNode::Empty => 0,
            PaneNode::Leaf(_) => 1,
            PaneNode::Split { first, second, .. } => {
                first.pane_count() + second.pane_count()
            },
        }
    }

    /// Split the leaf with `target` id: the existing pane stays first, the
    /// new pane goes second.
    pub fn split(&mut self, target: PaneId, dir: SplitDir, new_pane: Pane) {
        match self {
            PaneNode::Leaf(pane) if pane.id == target => {
                let old = std::mem::replace(self, PaneNode::Empty);
                *self = PaneNode::Split {
                    dir,
                    ratio: 0.5,
                    first: Box::new(old),
                    second: Box::new(PaneNode::Leaf(new_pane)),
                };
            },
            PaneNode::Split { first, second, .. } => {
                if first.contains(target) {
                    first.split(target, dir, new_pane);
                } else if second.contains(target) {
                    second.split(target, dir, new_pane);
                }
            },
            _ => {},
        }
    }

    /// Remove the pane with the given id, collapsing its parent split. If
    /// the root itself is the target leaf, it becomes `Empty` (the caller
    /// then closes the tab).
    pub fn remove(&mut self, id: PaneId) {
        match self {
            PaneNode::Leaf(pane) if pane.id == id => {
                *self = PaneNode::Empty;
            },
            PaneNode::Split { first, second, .. } => {
                if matches!(&**first, PaneNode::Leaf(p) if p.id == id) {
                    let survivor =
                        std::mem::replace(&mut **second, PaneNode::Empty);
                    *self = survivor;
                } else if matches!(&**second, PaneNode::Leaf(p) if p.id == id)
                {
                    let survivor =
                        std::mem::replace(&mut **first, PaneNode::Empty);
                    *self = survivor;
                } else {
                    first.remove(id);
                    second.remove(id);
                }
            },
            _ => {},
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, PaneNode::Empty)
    }
}
