use std::sync::{Arc, Mutex};

/// The single chokepoint through which every byte destined for a pane's PTY
/// passes. In this stage it is a pass-through observer that maintains a
/// per-pane shadow line buffer — later stages use that buffer for the
/// learning engine and enforce the Tier-2 hold-then-write invariant here.
///
/// The gate never blocks or modifies user-typed input; it only observes.
#[derive(Default)]
pub struct InputGate {
    /// Reconstruction of the shell's current input line from the bytes we
    /// have written to the PTY: printables append, backspace pops, Enter
    /// snapshots into `last_submitted` and clears.
    shadow_line: String,
    /// True when we've seen bytes that make the shadow buffer unreliable
    /// (tab-completion, arrow keys, in-shell history search). Later stages
    /// fall back to grid scraping when dirty.
    dirty: bool,
    /// The most recent line snapshot taken when Enter was pressed.
    last_submitted: Option<SubmittedLine>,
}

#[derive(Debug, Clone)]
pub struct SubmittedLine {
    pub text: String,
    /// Whether the shadow buffer was clean when this line was captured.
    /// Useful for the UI to display "from history recall" badges and
    /// is part of the public data model.
    #[allow(dead_code)]
    pub reliable: bool,
}

/// Cloneable handle to a pane's input gate, shared between the UI (which
/// reads submissions) and the backend input observer (which feeds bytes).
pub type InputGateHandle = Arc<Mutex<InputGate>>;

/// Proof-of-confirmation token. Constructible only via `issue()`, and
/// `issue()` is called from exactly one place: the confirm dialog's Run
/// handler. The token is deliberately not `Clone`, `Debug`, or
/// serializable — it cannot be reproduced, logged, or persisted.
///
/// This makes "no Tier-2 command reaches the shell without one explicit
/// user confirmation" a structural property, not a policy: the only PTY
/// code path that appends `\r` to programmatically-injected text requires
/// consuming a `ConfirmationToken` by value.
pub struct ConfirmationToken(());

impl ConfirmationToken {
    /// Mint a new token. Do NOT call from anywhere except the confirm
    /// dialog's Run button handler.
    pub(crate) fn issue() -> Self {
        ConfirmationToken(())
    }
}

pub fn new_gate() -> InputGateHandle {
    Arc::new(Mutex::new(InputGate::default()))
}

impl InputGate {
    /// Observe bytes headed to the PTY. Called for every write, user-typed
    /// or injected. `scraped_line` is the terminal grid's current cursor
    /// line (prompt included), provided when the write contains Enter —
    /// used as the fallback text source when the shadow buffer is dirty
    /// (tab-completion or history recall made it unreliable).
    pub fn observe(&mut self, bytes: &[u8], scraped_line: Option<&str>) {
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            match b {
                b'\r' | b'\n' => {
                    let text = if self.dirty {
                        scraped_line
                            .map(strip_prompt)
                            .unwrap_or_default()
                            .to_string()
                    } else {
                        self.shadow_line.trim().to_string()
                    };
                    if !text.is_empty() {
                        self.last_submitted = Some(SubmittedLine {
                            text,
                            reliable: !self.dirty,
                        });
                    }
                    self.shadow_line.clear();
                    self.dirty = false;
                    i += 1;
                },
                0x7f | 0x08 => {
                    // Backspace / DEL
                    self.shadow_line.pop();
                    i += 1;
                },
                b'\t' => {
                    // Tab completion: shell expands text we never see.
                    self.dirty = true;
                    i += 1;
                },
                0x1b => {
                    // Escape sequence (arrows, history recall, etc.) —
                    // consume it and mark the buffer dirty.
                    self.dirty = true;
                    i += consume_escape_sequence(&bytes[i..]);
                },
                0x12 => {
                    // Ctrl+R: in-shell reverse history search.
                    self.dirty = true;
                    i += 1;
                },
                0x03 => {
                    // Ctrl+C: line abandoned.
                    self.shadow_line.clear();
                    self.dirty = false;
                    i += 1;
                },
                _ if b < 0x20 => {
                    // Other control bytes: don't track, don't trust.
                    self.dirty = true;
                    i += 1;
                },
                _ => {
                    // Printable (start of a UTF-8 sequence or ASCII).
                    let len = utf8_len(b);
                    let end = (i + len).min(bytes.len());
                    if let Ok(s) = std::str::from_utf8(&bytes[i..end]) {
                        self.shadow_line.push_str(s);
                    }
                    i = end;
                },
            }
        }
    }

    /// Take the most recent Enter-submitted line, if any (consumes it).
    pub fn take_submitted(&mut self) -> Option<SubmittedLine> {
        self.last_submitted.take()
    }

    /// Current (not yet submitted) input line reconstruction.
    #[allow(dead_code)]
    pub fn current_line(&self) -> &str {
        &self.shadow_line
    }

    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

/// Strip a leading shell prompt from a scraped grid line, best-effort:
/// looks for the last occurrence of a common prompt terminator (`> `, `$ `,
/// `# `) in the first half of the line and returns what follows.
fn strip_prompt(line: &str) -> &str {
    let line = line.trim_end();
    let search_end = (line.len() / 2 + 8).min(line.len());
    // Spaced terminators first (PowerShell/bash), then cmd's bare `>`.
    for terminator in ["> ", "$ ", "# ", ">"] {
        if let Some(idx) = line[..search_end].rfind(terminator) {
            return line[idx + terminator.len()..].trim();
        }
    }
    line.trim()
}

/// Byte length of an escape sequence at the start of `bytes` (best-effort:
/// CSI sequences run to their final byte, otherwise assume 2 bytes).
fn consume_escape_sequence(bytes: &[u8]) -> usize {
    if bytes.len() >= 2 && bytes[1] == b'[' {
        // CSI: ESC [ ... final byte in 0x40..=0x7e
        for (idx, b) in bytes.iter().enumerate().skip(2) {
            if (0x40..=0x7e).contains(b) {
                return idx + 1;
            }
        }
        bytes.len()
    } else {
        2.min(bytes.len())
    }
}

fn utf8_len(first_byte: u8) -> usize {
    match first_byte {
        b if b < 0x80 => 1,
        b if b & 0xe0 == 0xc0 => 2,
        b if b & 0xf0 == 0xe0 => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_line_is_captured_on_enter() {
        let mut gate = InputGate::default();
        gate.observe(b"git status", None);
        gate.observe(b"\r", None);
        let sub = gate.take_submitted().unwrap();
        assert_eq!(sub.text, "git status");
        assert!(sub.reliable);
    }

    #[test]
    fn backspace_edits_shadow_line() {
        let mut gate = InputGate::default();
        gate.observe(b"git statuss", None);
        gate.observe(&[0x7f], None);
        gate.observe(b"\r", None);
        assert_eq!(gate.take_submitted().unwrap().text, "git status");
    }

    #[test]
    fn tab_falls_back_to_scraped_line() {
        let mut gate = InputGate::default();
        gate.observe(b"git sta\t", None);
        // Tab completion made the buffer dirty; Enter arrives with the
        // grid line (prompt + completed command).
        gate.observe(b"\r", Some("PS C:\\dev> git status"));
        let sub = gate.take_submitted().unwrap();
        assert_eq!(sub.text, "git status");
        assert!(!sub.reliable);
    }

    #[test]
    fn history_recall_uses_scraped_line() {
        let mut gate = InputGate::default();
        gate.observe(b"\x1b[A", None); // Up arrow: shell recalls history
        gate.observe(b"\r", Some("PS C:\\dev> cargo build"));
        let sub = gate.take_submitted().unwrap();
        assert_eq!(sub.text, "cargo build");
        assert!(!gate.is_dirty());
    }

    #[test]
    fn dirty_enter_without_scrape_yields_nothing() {
        let mut gate = InputGate::default();
        gate.observe(b"\x1b[A", None);
        gate.observe(b"\r", None);
        assert!(gate.take_submitted().is_none());
    }

    #[test]
    fn ctrl_c_abandons_line() {
        let mut gate = InputGate::default();
        gate.observe(b"rm -rf something", None);
        gate.observe(&[0x03], None);
        gate.observe(b"echo hi\r", None);
        assert_eq!(gate.take_submitted().unwrap().text, "echo hi");
    }

    #[test]
    fn strip_prompt_handles_common_prompts() {
        assert_eq!(strip_prompt("PS C:\\dev> git status"), "git status");
        assert_eq!(strip_prompt("user@host:~$ ls -la"), "ls -la");
        assert_eq!(strip_prompt("C:\\dev>echo hi"), "echo hi");
    }
}
