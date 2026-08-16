//! The dedicated SQLite thread. Owns the single `rusqlite::Connection`;
//! the UI communicates via `DbCommand` messages and receives
//! `ShortcutSuggestion` app events back.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

use super::frequency::{self, SequenceTracker};
use crate::terminal::pane::PaneId;

/// Messages accepted by the DB thread.
pub enum DbCommand {
    /// A command was executed in a pane (Enter pressed in the terminal).
    RecordCommand {
        pane_id: PaneId,
        command: String,
        raw: String,
        shell: String,
        cwd: Option<String>,
        exit_code: Option<i32>,
    },
    /// Update the exit code of the most recent history row for a pane
    /// (arrives later via OSC 133;D).
    RecordExitCode { pane_id: PaneId, exit_code: i32 },
    /// Save a user-approved shortcut.
    SaveShortcut { name: String, commands: Vec<String> },
    DeleteShortcut { name: String },
    /// Fuzzy-ish history search for the Ctrl+R palette; replies on the
    /// provided channel.
    SearchHistory {
        query: String,
        limit: usize,
        reply: Sender<Vec<HistoryEntry>>,
    },
    ListShortcuts {
        reply: Sender<Vec<Shortcut>>,
    },
    /// Future: adjust learning thresholds without a restart.
    #[allow(dead_code)]
    SetPref { key: String, value: String },
    /// Graceful shutdown; sent from DbHandle::drop.
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub command: String,
    /// Timestamp of the most recent execution. Not displayed in the
    /// history palette yet; kept for future "recency" sorting and badges.
    #[allow(dead_code)]
    pub last_ts: i64,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct Shortcut {
    pub name: String,
    pub commands: Vec<String>,
}

/// Events the DB thread pushes back to the UI.
#[derive(Debug, Clone)]
pub enum DbEvent {
    ShortcutSuggestion { commands: Vec<String>, count: u32 },
    Error(String),
}

pub struct DbHandle {
    pub sender: Sender<DbCommand>,
}

impl DbHandle {
    pub fn send(&self, cmd: DbCommand) {
        let _ = self.sender.send(cmd);
    }
}

impl Drop for DbHandle {
    fn drop(&mut self) {
        // Best-effort graceful shutdown of the DB thread. The WAL is
        // checkpointed on next open; no data is lost on a hard kill.
        let _ = self.sender.send(DbCommand::Shutdown);
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Default DB path: %APPDATA%\RustyTerminal\rusty.db
pub fn default_db_path() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("RustyTerminal");
    let _ = std::fs::create_dir_all(&dir);
    dir.push("rusty.db");
    dir
}

/// Spawn the DB thread. `event_sender` + `egui_ctx` deliver events back to
/// the UI (repaint is requested after each event).
pub fn spawn(
    db_path: PathBuf,
    event_sender: Sender<DbEvent>,
    egui_ctx: egui::Context,
) -> std::io::Result<DbHandle> {
    let (tx, rx) = mpsc::channel::<DbCommand>();

    std::thread::Builder::new()
        .name("rusty_db".to_string())
        .spawn(move || {
            let conn = match open_db(&db_path) {

                Ok(c) => c,
                Err(err) => {
                    let _ = event_sender
                        .send(DbEvent::Error(format!("db open failed: {err}")));
                    egui_ctx.request_repaint();
                    return;
                }
            };
            run_loop(conn, rx, event_sender, egui_ctx);
        })?;

    Ok(DbHandle { sender: tx })
}

pub fn open_db(path: &PathBuf) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(

        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);

         CREATE TABLE IF NOT EXISTS command_history (
           id INTEGER PRIMARY KEY,
           command    TEXT NOT NULL,
           raw        TEXT NOT NULL,
           shell      TEXT NOT NULL,
           cwd        TEXT,
           exit_code  INTEGER,
           pane_id    INTEGER NOT NULL DEFAULT 0,
           ts         INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_history_cmd ON command_history(command);
         CREATE INDEX IF NOT EXISTS idx_history_ts  ON command_history(ts);

         CREATE TABLE IF NOT EXISTS command_freq (
           command TEXT PRIMARY KEY,
           count   INTEGER NOT NULL,
           last_ts INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS sequence_freq (
           seq_hash  TEXT PRIMARY KEY,
           commands  TEXT NOT NULL,
           count     INTEGER NOT NULL,
           last_ts   INTEGER NOT NULL,
           suggested INTEGER NOT NULL DEFAULT 0
         );

         CREATE TABLE IF NOT EXISTS shortcuts (
           id INTEGER PRIMARY KEY,
           name       TEXT NOT NULL UNIQUE,
           commands   TEXT NOT NULL,
           created_ts INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS prefs (
           key TEXT PRIMARY KEY,
           value TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS ai_suggestions (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           phrase       TEXT NOT NULL,
           rendered_cmd TEXT NOT NULL,
           tier         INTEGER NOT NULL,
           provider     TEXT NOT NULL,
           ts           INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS ai_accepted (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           suggestion_id INTEGER NOT NULL,
           exit_code     INTEGER,
           executed_ts   INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS ai_rejected (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           suggestion_id INTEGER NOT NULL,
           inferred_ts   INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS ai_chat_history (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           role    TEXT NOT NULL,
           content TEXT NOT NULL,
           ts      INTEGER NOT NULL
         );",
    )?;
    let has_version: Option<i64> = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| {
            r.get(0)
        })
        .ok();
    if has_version.is_none() {
        conn.execute("INSERT INTO schema_version (version) VALUES (1)", [])?;
    }
    Ok(conn)
}

pub fn open_default_db() -> rusqlite::Result<Connection> {
    open_db(&default_db_path())
}



fn get_pref_u32(conn: &Connection, key: &str, default: u32) -> u32 {
    conn.query_row(
        "SELECT value FROM prefs WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(default)
}

fn run_loop(
    conn: Connection,
    rx: Receiver<DbCommand>,
    event_sender: Sender<DbEvent>,
    egui_ctx: egui::Context,
) {
    let mut tracker = SequenceTracker::default();

    while let Ok(cmd) = rx.recv() {
        let result = match cmd {
            DbCommand::Shutdown => break,
            DbCommand::RecordCommand {
                pane_id,
                command,
                raw,
                shell,
                cwd,
                exit_code,
            } => record_command(
                &conn,
                &mut tracker,
                &event_sender,
                &egui_ctx,
                pane_id,
                command,
                raw,
                shell,
                cwd,
                exit_code,
            ),
            DbCommand::RecordExitCode { pane_id, exit_code } => conn
                .execute(
                    "UPDATE command_history SET exit_code = ?1
                     WHERE id = (SELECT id FROM command_history
                                 WHERE pane_id = ?2 ORDER BY ts DESC LIMIT 1)
                       AND exit_code IS NULL",
                    params![exit_code, pane_id as i64],
                )
                .map(|_| ()),
            DbCommand::SaveShortcut { name, commands } => {
                let json = serde_json::to_string(&commands)
                    .unwrap_or_else(|_| "[]".to_string());
                conn.execute(
                    "INSERT OR REPLACE INTO shortcuts
                     (name, commands, created_ts) VALUES (?1, ?2, ?3)",
                    params![name, json, now_ms()],
                )
                .map(|_| ())
            },
            DbCommand::DeleteShortcut { name } => conn
                .execute("DELETE FROM shortcuts WHERE name = ?1", params![name])
                .map(|_| ()),
            DbCommand::SearchHistory {
                query,
                limit,
                reply,
            } => {
                let entries = search_history(&conn, &query, limit)
                    .unwrap_or_default();
                let _ = reply.send(entries);
                Ok(())
            },
            DbCommand::ListShortcuts { reply } => {
                let shortcuts = list_shortcuts(&conn).unwrap_or_default();
                let _ = reply.send(shortcuts);
                Ok(())
            },
            DbCommand::SetPref { key, value } => conn
                .execute(
                    "INSERT OR REPLACE INTO prefs (key, value) VALUES (?1, ?2)",
                    params![key, value],
                )
                .map(|_| ()),
        };

        if let Err(err) = result {
            let _ = event_sender.send(DbEvent::Error(err.to_string()));
            egui_ctx.request_repaint();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn record_command(
    conn: &Connection,
    tracker: &mut SequenceTracker,
    event_sender: &Sender<DbEvent>,
    egui_ctx: &egui::Context,
    pane_id: PaneId,
    command: String,
    raw: String,
    shell: String,
    cwd: Option<String>,
    exit_code: Option<i32>,
) -> rusqlite::Result<()> {
    let ts = now_ms();
    let normalized = frequency::normalize(&command);
    if normalized.is_empty() {
        return Ok(());
    }

    conn.execute(
        "INSERT INTO command_history
         (command, raw, shell, cwd, exit_code, pane_id, ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![normalized, raw, shell, cwd, exit_code, pane_id as i64, ts],
    )?;

    conn.execute(
        "INSERT INTO command_freq (command, count, last_ts)
         VALUES (?1, 1, ?2)
         ON CONFLICT(command)
         DO UPDATE SET count = count + 1, last_ts = ?2",
        params![normalized, ts],
    )?;

    // Sequence tracking: upsert every suffix-window candidate and fire a
    // suggestion when one crosses the threshold.
    let threshold = get_pref_u32(
        conn,
        "sequence_threshold",
        frequency::DEFAULT_SEQUENCE_THRESHOLD,
    );
    for seq in tracker.record(pane_id, &normalized, ts) {
        let hash = frequency::sequence_hash(&seq);
        let json = serde_json::to_string(&seq)
            .unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO sequence_freq (seq_hash, commands, count, last_ts)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(seq_hash)
             DO UPDATE SET count = count + 1, last_ts = ?3",
            params![hash, json, ts],
        )?;

        let (count, suggested): (u32, bool) = conn.query_row(
            "SELECT count, suggested FROM sequence_freq WHERE seq_hash = ?1",
            params![hash],
            |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)),
        )?;

        if count >= threshold && !suggested {
            conn.execute(
                "UPDATE sequence_freq SET suggested = 1 WHERE seq_hash = ?1",
                params![hash],
            )?;
            let _ = event_sender.send(DbEvent::ShortcutSuggestion {
                commands: seq,
                count,
            });
            egui_ctx.request_repaint();
        }
    }

    Ok(())
}

fn search_history(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> rusqlite::Result<Vec<HistoryEntry>> {
    // Substring match over the frequency rollup, most recent first.
    // Fuzzy scoring (subsequence match) happens UI-side on this candidate
    // set; SQL narrows with LIKE when a query is present.
    let mut entries = Vec::new();
    if query.is_empty() {
        let mut stmt = conn.prepare(
            "SELECT command, last_ts, count FROM command_freq
             ORDER BY last_ts DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok(HistoryEntry {
                command: r.get(0)?,
                last_ts: r.get(1)?,
                count: r.get(2)?,
            })
        })?;
        for row in rows {
            entries.push(row?);
        }
    } else {
        let like = format!("%{}%", query.replace('%', "\\%"));
        let mut stmt = conn.prepare(
            "SELECT command, last_ts, count FROM command_freq
             WHERE command LIKE ?1 ESCAPE '\\'
             ORDER BY count DESC, last_ts DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![like, limit as i64], |r| {
            Ok(HistoryEntry {
                command: r.get(0)?,
                last_ts: r.get(1)?,
                count: r.get(2)?,
            })
        })?;
        for row in rows {
            entries.push(row?);
        }
    }
    Ok(entries)
}

fn list_shortcuts(conn: &Connection) -> rusqlite::Result<Vec<Shortcut>> {
    let mut stmt = conn
        .prepare("SELECT name, commands FROM shortcuts ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        let name: String = r.get(0)?;
        let json: String = r.get(1)?;
        Ok((name, json))
    })?;
    let mut shortcuts = Vec::new();
    for row in rows {
        let (name, json) = row?;
        let commands: Vec<String> =
            serde_json::from_str(&json).unwrap_or_default();
        shortcuts.push(Shortcut { name, commands });
    }
    Ok(shortcuts)
}

#[derive(Debug, Clone)]
pub struct AiSuggestionRecord {
    pub id: i64,
    pub phrase: String,
    pub rendered_cmd: String,
    pub tier: u8,
    pub provider: String,
    pub ts: i64,
}

#[derive(Debug, Clone)]
pub struct AiAcceptedRecord {
    pub id: i64,
    pub suggestion_id: i64,
    pub phrase: String,
    pub rendered_cmd: String,
    pub exit_code: Option<i32>,
    pub executed_ts: i64,
}

#[derive(Debug, Clone)]
pub struct AiRejectedRecord {
    pub id: i64,
    pub suggestion_id: i64,
    pub phrase: String,
    pub rendered_cmd: String,
    pub inferred_ts: i64,
}

#[derive(Debug, Clone)]
pub struct AiChatMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub ts: i64,
}

pub fn record_ai_suggestion(
    conn: &Connection,
    phrase: &str,
    rendered_cmd: &str,
    tier: u8,
    provider: &str,
) -> rusqlite::Result<i64> {
    let ts = now_ms();
    conn.execute(
        "INSERT INTO ai_suggestions (phrase, rendered_cmd, tier, provider, ts) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![phrase, rendered_cmd, tier as i64, provider, ts],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn record_ai_accepted(
    conn: &Connection,
    suggestion_id: i64,
    exit_code: Option<i32>,
) -> rusqlite::Result<()> {
    let ts = now_ms();
    conn.execute(
        "INSERT INTO ai_accepted (suggestion_id, exit_code, executed_ts) VALUES (?1, ?2, ?3)",
        params![suggestion_id, exit_code, ts],
    )?;
    Ok(())
}

pub fn record_ai_rejected(
    conn: &Connection,
    suggestion_id: i64,
) -> rusqlite::Result<()> {
    let ts = now_ms();
    conn.execute(
        "INSERT INTO ai_rejected (suggestion_id, inferred_ts) VALUES (?1, ?2)",
        params![suggestion_id, ts],
    )?;
    Ok(())
}

pub fn get_ai_suggestions(
    conn: &Connection,
    limit: usize,
) -> rusqlite::Result<Vec<AiSuggestionRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, phrase, rendered_cmd, tier, provider, ts FROM ai_suggestions ORDER BY ts DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |r| {
        Ok(AiSuggestionRecord {
            id: r.get(0)?,
            phrase: r.get(1)?,
            rendered_cmd: r.get(2)?,
            tier: r.get::<_, i64>(3)? as u8,
            provider: r.get(4)?,
            ts: r.get(5)?,
        })
    })?;
    let mut list = Vec::new();
    for row in rows {
        list.push(row?);
    }
    Ok(list)
}

pub fn get_ai_accepted(
    conn: &Connection,
    limit: usize,
) -> rusqlite::Result<Vec<AiAcceptedRecord>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.suggestion_id, s.phrase, s.rendered_cmd, a.exit_code, a.executed_ts
         FROM ai_accepted a
         JOIN ai_suggestions s ON a.suggestion_id = s.id
         ORDER BY a.executed_ts DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |r| {
        Ok(AiAcceptedRecord {
            id: r.get(0)?,
            suggestion_id: r.get(1)?,
            phrase: r.get(2)?,
            rendered_cmd: r.get(3)?,
            exit_code: r.get(4)?,
            executed_ts: r.get(5)?,
        })
    })?;
    let mut list = Vec::new();
    for row in rows {
        list.push(row?);
    }
    Ok(list)
}

pub fn get_ai_rejected(
    conn: &Connection,
    limit: usize,
) -> rusqlite::Result<Vec<AiRejectedRecord>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.suggestion_id, s.phrase, s.rendered_cmd, r.inferred_ts
         FROM ai_rejected r
         JOIN ai_suggestions s ON r.suggestion_id = s.id
         ORDER BY r.inferred_ts DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |r| {
        Ok(AiRejectedRecord {
            id: r.get(0)?,
            suggestion_id: r.get(1)?,
            phrase: r.get(2)?,
            rendered_cmd: r.get(3)?,
            inferred_ts: r.get(4)?,
        })
    })?;
    let mut list = Vec::new();
    for row in rows {
        list.push(row?);
    }
    Ok(list)
}

pub fn save_chat_message(
    conn: &Connection,
    role: &str,
    content: &str,
) -> rusqlite::Result<()> {
    let ts = now_ms();
    conn.execute(
        "INSERT INTO ai_chat_history (role, content, ts) VALUES (?1, ?2, ?3)",
        params![role, content, ts],
    )?;
    Ok(())
}

pub fn get_chat_history(
    conn: &Connection,
    limit: usize,
) -> rusqlite::Result<Vec<AiChatMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, role, content, ts FROM ai_chat_history ORDER BY ts ASC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |r| {
        Ok(AiChatMessage {
            id: r.get(0)?,
            role: r.get(1)?,
            content: r.get(2)?,
            ts: r.get(3)?,
        })
    })?;
    let mut list = Vec::new();
    for row in rows {
        list.push(row?);
    }
    Ok(list)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE command_history (
               id INTEGER PRIMARY KEY, command TEXT NOT NULL,
               raw TEXT NOT NULL, shell TEXT NOT NULL, cwd TEXT,
               exit_code INTEGER, pane_id INTEGER NOT NULL DEFAULT 0,
               ts INTEGER NOT NULL);
             CREATE TABLE command_freq (
               command TEXT PRIMARY KEY, count INTEGER NOT NULL,
               last_ts INTEGER NOT NULL);
             CREATE TABLE sequence_freq (
               seq_hash TEXT PRIMARY KEY, commands TEXT NOT NULL,
               count INTEGER NOT NULL, last_ts INTEGER NOT NULL,
               suggested INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE shortcuts (
               id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE,
               commands TEXT NOT NULL, created_ts INTEGER NOT NULL);
             CREATE TABLE prefs (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .unwrap();
        conn
    }

    fn record(
        conn: &Connection,
        tracker: &mut SequenceTracker,
        sender: &Sender<DbEvent>,
        ctx: &egui::Context,
        cmd: &str,
    ) {
        record_command(
            conn,
            tracker,
            sender,
            ctx,
            1,
            cmd.to_string(),
            cmd.to_string(),
            "powershell".to_string(),
            None,
            None,
        )
        .unwrap();
    }

    #[test]
    fn sequence_suggestion_fires_at_threshold_once() {
        let conn = test_conn();
        let mut tracker = SequenceTracker::default();
        let (tx, rx) = mpsc::channel();
        let ctx = egui::Context::default();

        // Default threshold is 3: run the pair 3 times.
        for _ in 0..3 {
            record(&conn, &mut tracker, &tx, &ctx, "git status");
            record(&conn, &mut tracker, &tx, &ctx, "git pull");
        }

        let mut suggestions = vec![];
        while let Ok(ev) = rx.try_recv() {
            if let DbEvent::ShortcutSuggestion { commands, .. } = ev {
                suggestions.push(commands);
            }
        }
        assert!(
            suggestions
                .iter()
                .any(|s| s == &vec!["git status", "git pull"]),
            "expected the pair suggestion, got {suggestions:?}"
        );

        // Run it 3 more times: no re-suggestion (suggested flag set).
        for _ in 0..3 {
            record(&conn, &mut tracker, &tx, &ctx, "git status");
            record(&conn, &mut tracker, &tx, &ctx, "git pull");
        }
        let mut again = vec![];
        while let Ok(ev) = rx.try_recv() {
            if let DbEvent::ShortcutSuggestion { commands, .. } = ev {
                again.push(commands);
            }
        }
        assert!(
            !again
                .iter()
                .any(|s| s == &vec!["git status", "git pull"]),
            "pair must not be re-suggested, got {again:?}"
        );
    }

    #[test]
    fn threshold_pref_is_respected() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO prefs (key, value) VALUES ('sequence_threshold', '5')",
            [],
        )
        .unwrap();
        let mut tracker = SequenceTracker::default();
        let (tx, rx) = mpsc::channel();
        let ctx = egui::Context::default();

        for _ in 0..4 {
            record(&conn, &mut tracker, &tx, &ctx, "cargo build");
            record(&conn, &mut tracker, &tx, &ctx, "cargo test");
        }
        assert!(
            rx.try_recv().is_err(),
            "no suggestion should fire below threshold 5"
        );

        record(&conn, &mut tracker, &tx, &ctx, "cargo build");
        record(&conn, &mut tracker, &tx, &ctx, "cargo test");
        let got = matches!(
            rx.try_recv(),
            Ok(DbEvent::ShortcutSuggestion { .. })
        );
        assert!(got, "suggestion should fire at threshold 5");
    }

    #[test]
    fn command_freq_accumulates() {
        let conn = test_conn();
        let mut tracker = SequenceTracker::default();
        let (tx, _rx) = mpsc::channel();
        let ctx = egui::Context::default();

        record(&conn, &mut tracker, &tx, &ctx, "git status");
        record(&conn, &mut tracker, &tx, &ctx, "git  status"); // extra ws
        let count: u32 = conn
            .query_row(
                "SELECT count FROM command_freq WHERE command = 'git status'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2, "normalization should merge the two spellings");
    }

    #[test]
    fn exit_code_updates_latest_row() {
        let conn = test_conn();
        let mut tracker = SequenceTracker::default();
        let (tx, _rx) = mpsc::channel();
        let ctx = egui::Context::default();

        record(&conn, &mut tracker, &tx, &ctx, "cargo build");
        conn.execute(
            "UPDATE command_history SET exit_code = ?1
             WHERE id = (SELECT id FROM command_history
                         WHERE pane_id = ?2 ORDER BY ts DESC LIMIT 1)
               AND exit_code IS NULL",
            params![101, 1i64],
        )
        .unwrap();
        let code: i32 = conn
            .query_row(
                "SELECT exit_code FROM command_history ORDER BY ts DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(code, 101);
    }

    #[test]
    fn shortcut_roundtrip() {
        let conn = test_conn();
        let json = serde_json::to_string(&vec!["git status", "git pull"])
            .unwrap();
        conn.execute(
            "INSERT INTO shortcuts (name, commands, created_ts)
             VALUES ('sync', ?1, 0)",
            params![json],
        )
        .unwrap();
        let shortcuts = list_shortcuts(&conn).unwrap();
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].name, "sync");
        assert_eq!(shortcuts[0].commands, vec!["git status", "git pull"]);
    }

    #[test]
    fn history_search_matches_substring() {
        let conn = test_conn();
        let mut tracker = SequenceTracker::default();
        let (tx, _rx) = mpsc::channel();
        let ctx = egui::Context::default();

        record(&conn, &mut tracker, &tx, &ctx, "git status");
        record(&conn, &mut tracker, &tx, &ctx, "cargo build");
        let hits = search_history(&conn, "git", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].command, "git status");
    }
}
