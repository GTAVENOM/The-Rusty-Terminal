//! Command and command-sequence frequency logic.
//!
//! Pure functions + a small in-memory window tracker; all SQLite access
//! happens on the DB thread (`db.rs`), which calls into this module.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::terminal::pane::PaneId;

/// Two commands belong to the same "sequence window" when run in the same
/// pane no more than this many milliseconds apart.
pub const SEQUENCE_WINDOW_MS: i64 = 120_000;

/// Sequence lengths we track (suffix windows of the rolling history).
pub const MIN_SEQ_LEN: usize = 2;
pub const MAX_SEQ_LEN: usize = 4;

/// Default number of times a sequence must repeat before we offer to save
/// it as a shortcut. Configurable via prefs.
pub const DEFAULT_SEQUENCE_THRESHOLD: u32 = 3;

/// Normalize a command line for frequency matching: trim + collapse
/// internal whitespace runs.
pub fn normalize(command: &str) -> String {
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Stable hash for a normalized command sequence, used as the primary key
/// in `sequence_freq`.
pub fn sequence_hash(commands: &[String]) -> String {
    let mut hasher = DefaultHasher::new();
    for c in commands {
        c.hash(&mut hasher);
        0xffu8.hash(&mut hasher); // separator
    }
    format!("{:016x}", hasher.finish())
}

/// Rolling per-pane window of recent commands used to derive sequence
/// candidates. Lives on the DB thread.
#[derive(Default)]
pub struct SequenceTracker {
    windows: HashMap<PaneId, Vec<TimedCommand>>,
}

struct TimedCommand {
    command: String,
    ts_ms: i64,
}

impl SequenceTracker {
    /// Record a command execution and return every sequence candidate
    /// (suffix windows of length MIN..=MAX) that ends with it.
    pub fn record(
        &mut self,
        pane: PaneId,
        command: &str,
        ts_ms: i64,
    ) -> Vec<Vec<String>> {
        let normalized = normalize(command);
        if normalized.is_empty() {
            return vec![];
        }

        let window = self.windows.entry(pane).or_default();

        // Break the window if too much time has passed since the last
        // command.
        if let Some(last) = window.last() {
            if ts_ms - last.ts_ms > SEQUENCE_WINDOW_MS {
                window.clear();
            }
        }

        window.push(TimedCommand {
            command: normalized,
            ts_ms,
        });
        if window.len() > MAX_SEQ_LEN {
            window.remove(0);
        }

        let mut candidates = vec![];
        for len in MIN_SEQ_LEN..=window.len().min(MAX_SEQ_LEN) {
            let seq: Vec<String> = window[window.len() - len..]
                .iter()
                .map(|t| t.command.clone())
                .collect();
            // A sequence of identical commands repeated (e.g. `ls`, `ls`)
            // is not a useful shortcut candidate.
            if seq.windows(2).all(|w| w[0] == w[1]) {
                continue;
            }
            candidates.push(seq);
        }
        candidates
    }

    #[allow(dead_code)]
    pub fn clear_pane(&mut self, pane: PaneId) {
        self.windows.remove(&pane);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize("  git   status  "), "git status");
    }

    #[test]
    fn sequence_hash_is_stable_and_order_sensitive() {
        let a = vec!["git status".to_string(), "git pull".to_string()];
        let b = vec!["git status".to_string(), "git pull".to_string()];
        let c = vec!["git pull".to_string(), "git status".to_string()];
        assert_eq!(sequence_hash(&a), sequence_hash(&b));
        assert_ne!(sequence_hash(&a), sequence_hash(&c));
    }

    #[test]
    fn tracker_emits_suffix_windows() {
        let mut tracker = SequenceTracker::default();
        assert!(tracker.record(1, "git status", 0).is_empty());
        let candidates = tracker.record(1, "git pull", 1000);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], vec!["git status", "git pull"]);

        let candidates = tracker.record(1, "cargo build", 2000);
        // Suffixes: [pull, build] and [status, pull, build]
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0], vec!["git pull", "cargo build"]);
        assert_eq!(
            candidates[1],
            vec!["git status", "git pull", "cargo build"]
        );
    }

    #[test]
    fn tracker_breaks_window_on_timeout() {
        let mut tracker = SequenceTracker::default();
        tracker.record(1, "git status", 0);
        let candidates =
            tracker.record(1, "git pull", SEQUENCE_WINDOW_MS + 1);
        assert!(candidates.is_empty());
    }

    #[test]
    fn tracker_is_per_pane() {
        let mut tracker = SequenceTracker::default();
        tracker.record(1, "git status", 0);
        let candidates = tracker.record(2, "git pull", 100);
        assert!(candidates.is_empty());
    }

    #[test]
    fn identical_command_runs_are_not_candidates() {
        let mut tracker = SequenceTracker::default();
        tracker.record(1, "ls", 0);
        let candidates = tracker.record(1, "ls", 100);
        assert!(candidates.is_empty());
    }
}
