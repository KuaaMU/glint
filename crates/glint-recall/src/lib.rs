//! Local turn memory for Glint.
//!
//! The "echo" feature answers one question at selection time: has the user
//! asked about this exact concept before, outside the current thread? The
//! answer lives in a single SQLite file on the user's machine. Nothing in this
//! crate touches the network.
//!
//! ## What is stored
//!
//! Only completed turns: the normalized anchor (the concept the user asked
//! about), the conclusion sentence, the app it came from, an optional
//! `doc_key` digest, and a timestamp. Window titles never reach this crate;
//! callers pass the existing `doc_key` from `glint-core`, not the title.
//!
//! ## Retrieval contract
//!
//! `find_echo` runs inside a selection turn, so it has to stay cheap. An exact
//! anchor lookup uses an index; the fallback uses FTS5. Neither path scans the
//! table. A miss returns `None`, and callers must render nothing: the HUD has
//! no empty state for this feature.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// A completed turn worth remembering.
///
/// The caller only records turns that finished successfully. Errors, cancelled
/// turns and turns with no usable conclusion must not be recorded: a conclusion
/// that was cut off mid-stream reads as a fact the model never asserted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRecord {
    /// Conversation the turn belongs to. Echo never matches across a thread
    /// boundary, so a follow-up never re-surfaces its own context.
    pub thread_id: String,
    /// Foreground application class, e.g. `code` or `firefox`.
    pub app_class: String,
    /// Document digest from `glint_core::host`, never the raw window title.
    pub doc_key: Option<String>,
    /// The concept the user selected and asked about.
    pub anchor: String,
    /// The conclusion sentence. Truncated to [`MAX_CONCLUSION_CHARS`].
    pub conclusion: String,
    /// Optional follow-up the user typed in the same thread.
    pub follow_up: Option<String>,
    pub completed_at_ms: i64,
}

/// Where the current selection happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EchoContext {
    pub current_thread_id: String,
    pub current_doc_key: Option<String>,
    pub now_ms: i64,
    /// A previous turn about the same document only counts once it is at least
    /// this old. Re-asking the same question a minute later is not an echo.
    pub min_age_ms: i64,
}

/// One prior answer, ready for the HUD and the prompt budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Echo {
    pub turn_id: i64,
    pub conclusion: String,
    pub app_class: String,
    pub age_ms: i64,
    pub thread_id: String,
    /// The stored anchor, which may be longer than what the user selected.
    pub matched_anchor: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RecallError {
    #[error("recall database: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("recall io: {0}")]
    Io(#[from] std::io::Error),
}

/// Keep the stored conclusion short: it travels in a prompt block with a hard
/// token budget, and the HUD renders one line.
pub const MAX_CONCLUSION_CHARS: usize = 120;

const FTS_CANDIDATE_LIMIT: i64 = 50;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA user_version = 1;

CREATE TABLE IF NOT EXISTS turns (
    id                INTEGER PRIMARY KEY,
    thread_id         TEXT NOT NULL,
    app_class         TEXT NOT NULL,
    doc_key           TEXT,
    anchor            TEXT NOT NULL,
    anchor_key        TEXT NOT NULL,
    anchor_search     TEXT NOT NULL,
    conclusion        TEXT NOT NULL,
    conclusion_search TEXT NOT NULL,
    follow_up         TEXT,
    completed_at_ms   INTEGER NOT NULL,
    UNIQUE (thread_id, anchor_key)
);

CREATE INDEX IF NOT EXISTS turns_anchor_key ON turns(anchor_key, completed_at_ms DESC);
CREATE INDEX IF NOT EXISTS turns_completed  ON turns(completed_at_ms DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS turns_fts USING fts5(
    anchor_search,
    conclusion_search,
    content        = 'turns',
    content_rowid  = 'id',
    tokenize       = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS turns_ai AFTER INSERT ON turns BEGIN
    INSERT INTO turns_fts(rowid, anchor_search, conclusion_search)
    VALUES (new.id, new.anchor_search, new.conclusion_search);
END;

CREATE TRIGGER IF NOT EXISTS turns_ad AFTER DELETE ON turns BEGIN
    INSERT INTO turns_fts(turns_fts, rowid, anchor_search, conclusion_search)
    VALUES ('delete', old.id, old.anchor_search, old.conclusion_search);
END;

CREATE TRIGGER IF NOT EXISTS turns_au AFTER UPDATE ON turns BEGIN
    INSERT INTO turns_fts(turns_fts, rowid, anchor_search, conclusion_search)
    VALUES ('delete', old.id, old.anchor_search, old.conclusion_search);
    INSERT INTO turns_fts(rowid, anchor_search, conclusion_search)
    VALUES (new.id, new.anchor_search, new.conclusion_search);
END;
"#;

pub struct RecallStore {
    conn: Connection,
}

impl RecallStore {
    /// Open (or create) the store at `path`. Parent directories are created.
    pub fn open(path: &Path) -> Result<Self, RecallError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Record one completed turn. Re-asking the same anchor in the same thread
    /// updates that row instead of adding a duplicate.
    pub fn record_turn(&mut self, turn: TurnRecord) -> Result<(), RecallError> {
        let anchor_key = normalize_anchor(&turn.anchor);
        if anchor_key.is_empty() {
            return Ok(());
        }

        let conclusion = truncate_chars(turn.conclusion.trim(), MAX_CONCLUSION_CHARS);
        if conclusion.is_empty() {
            return Ok(());
        }

        let follow_up = turn
            .follow_up
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|text| truncate_chars(text, MAX_CONCLUSION_CHARS));

        self.conn.execute(
            "INSERT INTO turns (
                 thread_id, app_class, doc_key, anchor, anchor_key,
                 anchor_search, conclusion, conclusion_search, follow_up,
                 completed_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(thread_id, anchor_key) DO UPDATE SET
                 app_class         = excluded.app_class,
                 doc_key           = excluded.doc_key,
                 anchor            = excluded.anchor,
                 anchor_search     = excluded.anchor_search,
                 conclusion        = excluded.conclusion,
                 conclusion_search = excluded.conclusion_search,
                 follow_up         = excluded.follow_up,
                 completed_at_ms   = excluded.completed_at_ms",
            params![
                turn.thread_id,
                turn.app_class,
                turn.doc_key.as_deref(),
                turn.anchor.trim(),
                anchor_key,
                expand_for_search(&anchor_key),
                conclusion,
                expand_for_search(&conclusion),
                follow_up.as_deref(),
                turn.completed_at_ms,
            ],
        )?;
        Ok(())
    }

    /// Find the most recent prior answer about `query`, if one qualifies.
    ///
    /// A candidate is skipped when it belongs to the current thread, or when it
    /// comes from the same document and is younger than `ctx.min_age_ms`.
    pub fn find_echo(&self, query: &str, ctx: &EchoContext) -> Result<Option<Echo>, RecallError> {
        let anchor_key = normalize_anchor(query);
        if anchor_key.is_empty() {
            return Ok(None);
        }

        let cutoff = ctx.now_ms.saturating_sub(ctx.min_age_ms.max(0));

        let exact = {
            let mut stmt = self.conn.prepare_cached(
                "SELECT id, thread_id, app_class, anchor, anchor_key, conclusion, completed_at_ms
                 FROM turns
                 WHERE anchor_key = ?1
                   AND thread_id <> ?2
                   AND (
                     completed_at_ms <= ?4
                     OR (doc_key IS NOT ?3 AND doc_key IS NOT NULL AND ?3 IS NOT NULL)
                   )
                 ORDER BY completed_at_ms DESC
                 LIMIT 1",
            )?;
            stmt.query_row(
                params![
                    anchor_key,
                    ctx.current_thread_id,
                    ctx.current_doc_key.as_deref(),
                    cutoff,
                ],
                RawEcho::from_row,
            )
            .optional()?
        };

        if let Some(raw) = exact {
            return Ok(Some(raw.into_echo(ctx.now_ms)));
        }

        let Some(fts_query) = fts_match_query(&anchor_key) else {
            return Ok(None);
        };

        let mut stmt = self.conn.prepare_cached(
            "SELECT t.id, t.thread_id, t.app_class, t.anchor, t.anchor_key,
                    t.conclusion, t.completed_at_ms
             FROM turns_fts f
             JOIN turns t ON t.id = f.rowid
             WHERE turns_fts MATCH ?1
               AND t.thread_id <> ?2
               AND (
                 t.completed_at_ms <= ?4
                 OR (t.doc_key IS NOT ?3 AND t.doc_key IS NOT NULL AND ?3 IS NOT NULL)
               )
             ORDER BY t.completed_at_ms DESC
             LIMIT ?5",
        )?;

        let mut rows = stmt.query(params![
            fts_query,
            ctx.current_thread_id,
            ctx.current_doc_key.as_deref(),
            cutoff,
            FTS_CANDIDATE_LIMIT,
        ])?;

        while let Some(row) = rows.next()? {
            let raw = RawEcho::from_row(row)?;
            if anchors_are_compatible(&anchor_key, &raw.anchor_key) {
                return Ok(Some(raw.into_echo(ctx.now_ms)));
            }
        }

        Ok(None)
    }

    /// Forget every turn recorded for one thread.
    ///
    /// Deleting a conversation from the history panel must also remove what it
    /// left in the echo store; otherwise a deleted answer can still resurface
    /// next time the user selects the same concept.
    pub fn forget_thread(&mut self, thread_id: &str) -> Result<usize, RecallError> {
        let removed = self
            .conn
            .execute("DELETE FROM turns WHERE thread_id = ?1", params![thread_id])?;
        Ok(removed)
    }

    /// Remove the database and its WAL sidecars. Missing files are not an
    /// error: wiping an index that was never created is a no-op.
    pub fn wipe(path: &Path) -> Result<(), RecallError> {
        for target in [
            path.to_path_buf(),
            with_suffix(path, "-wal"),
            with_suffix(path, "-shm"),
        ] {
            match std::fs::remove_file(&target) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
        }
        Ok(())
    }
}

struct RawEcho {
    id: i64,
    thread_id: String,
    app_class: String,
    anchor: String,
    anchor_key: String,
    conclusion: String,
    completed_at_ms: i64,
}

impl RawEcho {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            thread_id: row.get(1)?,
            app_class: row.get(2)?,
            anchor: row.get(3)?,
            anchor_key: row.get(4)?,
            conclusion: row.get(5)?,
            completed_at_ms: row.get(6)?,
        })
    }

    fn into_echo(self, now_ms: i64) -> Echo {
        Echo {
            turn_id: self.id,
            conclusion: self.conclusion,
            app_class: self.app_class,
            age_ms: now_ms.saturating_sub(self.completed_at_ms),
            thread_id: self.thread_id,
            matched_anchor: self.anchor,
        }
    }
}

/// Normalize an anchor for exact matching.
///
/// Deliberately conservative: Unicode NFKC and full-width folding are not done
/// yet, because they need a Unicode table this crate does not carry. ASCII is
/// lowercased, internal whitespace is collapsed, and sentence punctuation is
/// dropped from the edges so a selection ending in a comma still matches.
/// Identifier punctuation (`_`, `-`, `:`, `.`, `<`, `>`) is preserved so
/// `Arc<Mutex<T>>` does not collapse into a different concept.
fn normalize_anchor(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;

    for ch in input.trim().chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.extend(ch.to_lowercase());
    }

    out.trim_matches(is_edge_punctuation).trim().to_string()
}

fn is_edge_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '.' | ','
            | ';'
            | '!'
            | '?'
            | '。'
            | '，'
            | '；'
            | '！'
            | '？'
            | '、'
            | '"'
            | '\''
            | '“'
            | '”'
            | '‘'
            | '’'
            | '('
            | ')'
            | '（'
            | '）'
            | '['
            | ']'
            | '【'
            | '】'
            | '《'
            | '》'
            | '「'
            | '」'
            | '『'
            | '』'
    )
}

/// Expand text into FTS5 tokens.
///
/// `unicode61` treats a run of Chinese characters as a single token, so a query
/// for `向量` would never match a stored `向量检索`. Overlapping bigrams make
/// every two-character window searchable. Latin and digit runs stay whole and
/// are lowercased.
fn expand_for_search(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    let mut run: Vec<char> = Vec::new();
    let mut word = String::new();

    for ch in input.chars() {
        if is_cjk(ch) {
            flush_word(&mut out, &mut word);
            run.push(ch);
        } else if ch.is_alphanumeric() || ch == '_' {
            flush_cjk(&mut out, &mut run);
            word.extend(ch.to_lowercase());
        } else {
            flush_cjk(&mut out, &mut run);
            flush_word(&mut out, &mut word);
        }
    }

    flush_cjk(&mut out, &mut run);
    flush_word(&mut out, &mut word);
    out
}

fn flush_cjk(out: &mut String, run: &mut Vec<char>) {
    if run.is_empty() {
        return;
    }
    if run.len() == 1 {
        push_token(out, &run[0].to_string());
    } else {
        for pair in run.windows(2) {
            let token: String = pair.iter().collect();
            push_token(out, &token);
        }
    }
    run.clear();
}

fn flush_word(out: &mut String, word: &mut String) {
    if !word.is_empty() {
        push_token(out, word);
        word.clear();
    }
}

fn push_token(out: &mut String, token: &str) {
    if token.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(token);
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF     // CJK Unified Ideographs Extension A
        | 0x4E00..=0x9FFF   // CJK Unified Ideographs
        | 0xF900..=0xFAFF   // CJK Compatibility Ideographs
        | 0x3040..=0x30FF   // Hiragana and Katakana
        | 0xAC00..=0xD7AF   // Hangul syllables
        | 0x20000..=0x2FA1F // CJK Extension B and beyond
    )
}

/// Build an FTS5 MATCH expression from the expanded anchor.
///
/// Every token is quoted and the terms are joined by whitespace, which FTS5
/// reads as an implicit AND. The bigrams of a query must all be present, which
/// is what keeps a short query from matching a long, unrelated anchor.
fn fts_match_query(anchor_key: &str) -> Option<String> {
    let expanded = expand_for_search(anchor_key);
    let terms: Vec<String> = expanded
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{}\"", term.replace('"', "")))
        .collect();

    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

/// The FTS fallback accepts a candidate only when one anchor contains the
/// other and the two are within a factor of two in length. Without the ratio
/// check, a query for `cache` would echo every turn that mentioned caching.
fn anchors_are_compatible(query: &str, candidate: &str) -> bool {
    if query.is_empty() || candidate.is_empty() {
        return false;
    }

    let (short, long) = if query.chars().count() <= candidate.chars().count() {
        (query, candidate)
    } else {
        (candidate, query)
    };

    if short.chars().count() < 2 || !long.contains(short) {
        return false;
    }

    let ratio = short.chars().count() as f32 / long.chars().count() as f32;
    ratio >= 0.5
}

fn truncate_chars(input: &str, max: usize) -> String {
    input
        .chars()
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut raw = path.as_os_str().to_owned();
    raw.push(suffix);
    PathBuf::from(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const HOUR: i64 = 3_600_000;
    const DAY: i64 = 24 * HOUR;
    const NOW: i64 = 1_000 * DAY;

    fn store() -> (TempDir, RecallStore) {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = RecallStore::open(&dir.path().join("recall.sqlite3")).expect("open store");
        (dir, store)
    }

    fn turn(thread: &str, app: &str, doc: Option<&str>, anchor: &str, at: i64) -> TurnRecord {
        TurnRecord {
            thread_id: thread.to_string(),
            app_class: app.to_string(),
            doc_key: doc.map(str::to_string),
            anchor: anchor.to_string(),
            conclusion: format!("{anchor} 的结论"),
            follow_up: None,
            completed_at_ms: at,
        }
    }

    fn ctx(thread: &str, doc: Option<&str>) -> EchoContext {
        EchoContext {
            current_thread_id: thread.to_string(),
            current_doc_key: doc.map(str::to_string),
            now_ms: NOW,
            min_age_ms: DAY,
        }
    }

    #[test]
    fn empty_store_has_no_echo() {
        let (_dir, store) = store();
        let echo = store.find_echo("cache", &ctx("new", Some("doc-b")));
        assert_eq!(echo.expect("query"), None);
    }

    #[test]
    fn exact_anchor_across_documents_matches() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn(
                "old-thread",
                "code",
                Some("doc-a"),
                "cache invalidation",
                NOW - HOUR,
            ))
            .expect("record");

        let echo = store
            .find_echo("cache invalidation", &ctx("new-thread", Some("doc-b")))
            .expect("query")
            .expect("echo");

        assert_eq!(echo.app_class, "code");
        assert_eq!(echo.thread_id, "old-thread");
        assert_eq!(echo.age_ms, HOUR);
    }

    #[test]
    fn same_thread_never_matches() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn(
                "thread-1",
                "code",
                Some("doc-a"),
                "borrow checker",
                NOW - DAY,
            ))
            .expect("record");

        let echo = store
            .find_echo("borrow checker", &ctx("thread-1", Some("doc-b")))
            .expect("query");
        assert_eq!(echo, None);
    }

    #[test]
    fn same_document_within_window_is_skipped() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn("old", "code", Some("doc-a"), "page table", NOW - HOUR))
            .expect("record");

        let echo = store
            .find_echo("page table", &ctx("new", Some("doc-a")))
            .expect("query");
        assert_eq!(echo, None);
    }

    #[test]
    fn same_document_after_window_matches() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn(
                "old",
                "code",
                Some("doc-a"),
                "page table",
                NOW - 2 * DAY,
            ))
            .expect("record");

        let echo = store
            .find_echo("page table", &ctx("new", Some("doc-a")))
            .expect("query");
        assert!(echo.is_some(), "a two-day-old turn should qualify");
    }

    #[test]
    fn unknown_document_still_requires_age() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn("old", "code", None, "page table", NOW - HOUR))
            .expect("record");

        // No stored doc_key: it cannot be proven to be a different document, so
        // a one-hour-old turn is too recent to echo.
        let recent = store
            .find_echo("page table", &ctx("new", Some("doc-b")))
            .expect("query");
        assert_eq!(recent, None);

        let later = store
            .find_echo(
                "page table",
                &EchoContext {
                    current_thread_id: "new".to_string(),
                    current_doc_key: Some("doc-b".to_string()),
                    now_ms: NOW + 2 * DAY,
                    min_age_ms: DAY,
                },
            )
            .expect("query");
        assert!(later.is_some(), "an old turn still qualifies");
    }

    #[test]
    fn unknown_current_document_requires_age() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn("old", "code", Some("doc-a"), "page table", NOW - HOUR))
            .expect("record");

        // The current document is unknown, which is not evidence that this is a
        // different one.
        let recent = store
            .find_echo("page table", &ctx("new", None))
            .expect("query");
        assert_eq!(recent, None);
    }

    #[test]
    fn chinese_bigram_recall() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn(
                "old",
                "firefox",
                Some("doc-a"),
                "向量检索的代价",
                NOW - DAY,
            ))
            .expect("record");

        let echo = store
            .find_echo("向量检索", &ctx("new", Some("doc-b")))
            .expect("query")
            .expect("echo should match through bigrams");

        assert_eq!(echo.matched_anchor, "向量检索的代价");
    }

    #[test]
    fn short_word_does_not_match_unrelated_anchor() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn(
                "old",
                "code",
                Some("doc-a"),
                "cache invalidation is hard",
                NOW - DAY,
            ))
            .expect("record");

        let echo = store
            .find_echo("cache", &ctx("new", Some("doc-b")))
            .expect("query");
        assert_eq!(
            echo, None,
            "a five-character query must not echo a long unrelated anchor"
        );
    }

    #[test]
    fn reasking_updates_the_remembered_conclusion() {
        let (_dir, mut store) = store();
        for (at, conclusion) in [(NOW - 2 * DAY, "first"), (NOW - DAY, "second")] {
            store
                .record_turn(TurnRecord {
                    conclusion: conclusion.to_string(),
                    ..turn("thread-1", "code", Some("doc-a"), "tlb", at)
                })
                .expect("record");
        }

        // Re-asking in one thread leaves one memory, and it is the latest one.
        let echo = store
            .find_echo("tlb", &ctx("thread-2", Some("doc-b")))
            .expect("query")
            .expect("echo");
        assert_eq!(echo.conclusion, "second");
    }

    #[test]
    fn forget_thread_removes_only_that_thread() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn(
                "thread-a",
                "code",
                Some("doc-a"),
                "borrow checker",
                NOW - HOUR,
            ))
            .expect("record a");
        store
            .record_turn(turn(
                "thread-b",
                "code",
                Some("doc-b"),
                "borrow checker",
                NOW - DAY,
            ))
            .expect("record b");

        assert_eq!(store.forget_thread("thread-a").expect("forget"), 1);

        // Thread A was the most recent; if it were still there it would win.
        let echo = store
            .find_echo("borrow checker", &ctx("thread-new", Some("doc-c")))
            .expect("query")
            .expect("thread b is still remembered");
        assert_eq!(echo.thread_id, "thread-b");
    }

    #[test]
    fn wipe_removes_database() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("recall.sqlite3");

        {
            let mut store = RecallStore::open(&path).expect("open");
            store
                .record_turn(turn("thread", "code", None, "witness", NOW))
                .expect("record");
        }

        assert!(path.exists());
        RecallStore::wipe(&path).expect("wipe");
        assert!(!path.exists());
    }

    #[test]
    fn identifiers_match_despite_edge_punctuation_and_case() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn(
                "old",
                "code",
                Some("doc-a"),
                "Arc<Mutex<T>>",
                NOW - DAY,
            ))
            .expect("record");

        let echo = store
            .find_echo("arc<mutex<t>>,", &ctx("new", Some("doc-b")))
            .expect("query")
            .expect("echo");
        assert_eq!(echo.matched_anchor, "Arc<Mutex<T>>");
    }

    #[test]
    fn a_two_character_query_matches_through_bigrams() {
        let (_dir, mut store) = store();
        store
            .record_turn(turn("old", "code", Some("doc-a"), "检索树", NOW - DAY))
            .expect("record");

        let echo = store
            .find_echo("检索", &ctx("new", Some("doc-b")))
            .expect("query")
            .expect("a two-character Chinese query must be searchable");
        assert_eq!(echo.matched_anchor, "检索树");
    }
}
