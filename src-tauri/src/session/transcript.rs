//! `ClaudeTranscriptSource` (part of rg.session-source): the JSONL channel.
//!
//! statusLine does not run in the Desktop Code tab (adr.rg.003), so this is not a
//! fallback — it is the only way a Desktop session reaches the panel, and it is also
//! the only place `entrypoint` says which surface any session lives on.
//!
//! Read-only throughout. The transcript belongs to Claude Code: it is opened, the tail
//! is read, and nothing is ever written, truncated or moved. A growing file's last line
//! is frequently half-written, so a line that does not parse is skipped rather than
//! treated as the end of the data.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::Deserialize;

use super::snapshot::{Origin, SessionSnapshot, Surface};

/// How far back from the end of a transcript to read. Enough to carry several records
/// on any session; reading the whole file would mean megabytes per poll for a long one.
const TAIL_BYTES: u64 = 256 * 1024;

/// `~/.claude/projects`. Both surfaces write here on 2.1.268.
pub fn projects_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("projects"))
}

/// The fields a transcript record can carry that we care about. Everything else in the
/// file — the messages themselves — is ignored: ReviewGlass shows session state, not
/// conversations, and never reads a transcript's content into anything it displays.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Record {
    #[serde(alias = "sessionId")]
    session_id: Option<String>,
    entrypoint: Option<String>,
    cwd: Option<String>,
    version: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    /// The session's title as the desktop app shows it (a `custom-title` record): session
    /// state, the same title the Code pane's header carries (adr.rg.022). Lenient: a
    /// title of another shape is no title, never a lost line.
    #[serde(alias = "customTitle", deserialize_with = "super::payload::lenient")]
    custom_title: Option<String>,
}

/// Titles seen, by session id. A title record recurs every few lines in a live
/// session, but a long transcript can hold one only near its start, out of the tail's
/// reach: once seen, a title is kept (and replaced by a newer one) for the app's life.
fn titles() -> &'static Mutex<HashMap<String, String>> {
    static T: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A session's title as last seen in its transcript, if one has been.
pub fn known_title(session_id: &str) -> Option<String> {
    titles().lock().get(session_id).cloned()
}

/// What one transcript file yields.
#[derive(Debug, Clone)]
pub struct TranscriptFacts {
    pub session_id: String,
    pub surface: Surface,
    pub cwd: Option<String>,
    pub version: Option<String>,
    /// File mtime in epoch ms: the transcript's own timestamps are per-record and the
    /// tail may hold none, but the file is touched on every write.
    pub modified_ms: u64,
    pub assistant_messages: u64,
    /// The session's title from its latest `custom-title` record, or the one seen
    /// before; `None` when no title has been seen.
    pub title: Option<String>,
}

/// Read the tail of one transcript. `None` when the file yields no session id at all,
/// which is the "unreadable" failure mode: the session is absent from the panel rather
/// than shown with empty fields.
pub fn read_facts(path: &Path) -> Option<TranscriptFacts> {
    let meta = fs::metadata(path).ok()?;
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let mut file = File::open(path).ok()?;
    let len = meta.len();
    let from = len.saturating_sub(TAIL_BYTES);
    if from > 0 {
        file.seek(SeekFrom::Start(from)).ok()?;
    }
    let mut buf = Vec::with_capacity((len - from).min(TAIL_BYTES) as usize + 1);
    file.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);

    // A mid-file seek lands inside a line; and the last line of a growing file is often
    // half-written. Both are handled by simply skipping anything that does not parse.
    let mut lines = text.lines().peekable();
    if from > 0 {
        lines.next();
    }

    let mut facts = TranscriptFacts {
        session_id: String::new(),
        surface: Surface::Unknown,
        cwd: None,
        version: None,
        modified_ms,
        assistant_messages: 0,
        title: None,
    };
    for line in lines {
        let Ok(r) = serde_json::from_str::<Record>(line) else {
            continue;
        };
        if let Some(id) = r.session_id {
            if facts.session_id.is_empty() {
                facts.session_id = id;
            }
        }
        if let Some(e) = r.entrypoint {
            facts.surface = Surface::from_entrypoint(&e);
        }
        if r.cwd.is_some() {
            facts.cwd = r.cwd;
        }
        if r.version.is_some() {
            facts.version = r.version;
        }
        if r.kind.as_deref() == Some("assistant") {
            facts.assistant_messages += 1;
        }
        if let Some(t) = r.custom_title.map(|t| t.trim().to_string()) {
            if !t.is_empty() {
                facts.title = Some(t);
            }
        }
        let _ = r.timestamp;
    }

    if facts.session_id.is_empty() {
        // The tail held no identified record. Fall back to the file name, which Claude
        // Code names after the session id — a weaker source, but it keeps a session
        // visible rather than dropping it for want of a field.
        facts.session_id = path.file_stem()?.to_string_lossy().into_owned();
    }
    let mut known = titles().lock();
    match &facts.title {
        Some(t) => {
            known.insert(facts.session_id.clone(), t.clone());
        }
        None => facts.title = known.get(&facts.session_id).cloned(),
    }
    Some(facts)
}

/// How long a session's own transcript may lie still while its subagents are still
/// looked for. A parent writes nothing while it waits for its agents — 7 to 27 minutes
/// measured on 27.9.2026 — so its subagents' transcripts are the only sign it is alive;
/// a session silent for longer than this is not waiting on an agent, and not listing
/// every old session's subagent folder keeps the two-second scan cheap.
const SUBAGENT_LOOKBACK: Duration = Duration::from_secs(12 * 60 * 60);

fn modified_ms(meta: &fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
}

/// The newest write among a session's subagents' transcripts:
/// `<project>/<session>/subagents/*.jsonl` beside `<project>/<session>.jsonl`.
fn subagents_modified_ms(transcript: &Path) -> Option<u64> {
    let dir = transcript.with_extension("").join("subagents");
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jsonl"))
        .filter_map(|e| e.metadata().ok().as_ref().and_then(modified_ms))
        .max()
}

fn epoch_ms_before(d: Duration) -> u64 {
    SystemTime::now()
        .checked_sub(d)
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Every session written within `ttl`, across every project directory.
///
/// Age is the only liveness signal there is: Claude Code writes no close record, so a
/// session that stops being written is a session that ended (the same TTL rule the spool
/// uses). A session's age is its newest write in its own transcript or in one of its
/// subagents': while agents work, the parent's own file lies still, and a session that
/// vanished from the panel whenever it launched agents was the owner's finding of
/// 27.9.2026.
pub fn recent(ttl: Duration) -> Vec<TranscriptFacts> {
    let Some(root) = projects_dir() else {
        return Vec::new();
    };
    recent_in(
        &root,
        epoch_ms_before(ttl),
        epoch_ms_before(SUBAGENT_LOOKBACK),
    )
}

/// `recent` over a given projects directory, with its two cut-offs in epoch ms: a
/// session is live when written at or after `cutoff`, and its subagents are looked at
/// only when its own transcript was written at or after `lookback`.
fn recent_in(root: &Path, cutoff: u64, lookback: u64) -> Vec<TranscriptFacts> {
    let mut out = Vec::new();
    let Ok(projects) = fs::read_dir(root) else {
        return out;
    };
    for project in projects.flatten() {
        let Ok(entries) = fs::read_dir(project.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(own) = entry.metadata().ok().as_ref().and_then(modified_ms) else {
                continue;
            };
            if own < lookback {
                continue;
            }
            let last = own.max(subagents_modified_ms(&path).unwrap_or(0));
            if last < cutoff {
                continue;
            }
            if let Some(mut f) = read_facts(&path) {
                f.modified_ms = last;
                out.push(f);
            }
        }
    }
    out
}

impl TranscriptFacts {
    pub fn into_snapshot(self) -> SessionSnapshot {
        SessionSnapshot {
            session_id: self.session_id,
            surface: self.surface,
            origin: Origin::Transcript,
            observed_at_ms: self.modified_ms,
            session_name: self.title,
            transcript_path: None,
            cwd: self.cwd,
            project_dir: None,
            model_name: None,
            model_id: None,
            version: self.version,
            effort: None,
            fast_mode: None,
            cost_usd: None,
            duration_ms: None,
            lines_added: None,
            lines_removed: None,
            context_used_pct: None,
            context_window_size: None,
            input_tokens: None,
            output_tokens: None,
            // A transcript carries no rate_limits. That is the whole reason the gauge is
            // borrowed from a CLI session (adr.rg.009).
            rate_limits: None,
            prompt_cache: None,
            pr: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(name: &str, lines: &[&str]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rg-transcript-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let p = d.join(name);
        let mut f = File::create(&p).unwrap();
        for l in lines {
            writeln!(f, "{l}").unwrap();
        }
        p
    }

    #[test]
    fn reads_the_surface_from_entrypoint() {
        let p = write(
            "a.jsonl",
            &[
                r#"{"type":"mode","mode":"normal","sessionId":"a-1"}"#,
                r#"{"type":"user","entrypoint":"claude-desktop","cwd":"E:\\p","version":"2.1.268"}"#,
                r#"{"type":"assistant"}"#,
                r#"{"type":"assistant"}"#,
            ],
        );
        let f = read_facts(&p).unwrap();
        assert_eq!(f.session_id, "a-1");
        assert_eq!(f.surface, Surface::Desktop);
        assert_eq!(f.cwd.as_deref(), Some("E:\\p"));
        assert_eq!(f.version.as_deref(), Some("2.1.268"));
        assert_eq!(f.assistant_messages, 2);
    }

    #[test]
    fn a_cli_transcript_reads_as_cli() {
        let p = write(
            "b.jsonl",
            &[
                r#"{"sessionId":"b-1"}"#,
                r#"{"type":"user","entrypoint":"cli"}"#,
            ],
        );
        assert_eq!(read_facts(&p).unwrap().surface, Surface::Cli);
    }

    #[test]
    fn a_half_written_last_line_is_skipped_not_fatal() {
        // The common case for a file being appended to while we read it.
        let p = write(
            "c.jsonl",
            &[
                r#"{"sessionId":"c-1"}"#,
                r#"{"type":"user","entrypoint":"cli"}"#,
                r#"{"type":"assist"#,
            ],
        );
        let f = read_facts(&p).unwrap();
        assert_eq!(f.session_id, "c-1");
        assert_eq!(f.surface, Surface::Cli);
    }

    #[test]
    fn a_transcript_with_no_identified_record_still_names_its_session() {
        let p = write("d-5.jsonl", &[r#"{"noise":true}"#]);
        let f = read_facts(&p).unwrap();
        assert_eq!(f.session_id, "d-5", "the file name is the weaker source");
        assert_eq!(f.surface, Surface::Unknown);
    }

    #[test]
    fn the_latest_custom_title_names_the_session_and_is_remembered() {
        let p = write(
            "t-title.jsonl",
            &[
                r#"{"type":"custom-title","customTitle":"First title","sessionId":"t-1"}"#,
                r#"{"type":"user","entrypoint":"claude-desktop"}"#,
                r#"{"type":"custom-title","customTitle":"  Projektin tilan tarkistus ","sessionId":"t-1"}"#,
                r#"{"type":"custom-title","customTitle":42,"sessionId":"t-1"}"#,
            ],
        );
        let f = read_facts(&p).unwrap();
        // The newest string title, trimmed; a title of another shape does not count.
        assert_eq!(f.title.as_deref(), Some("Projektin tilan tarkistus"));
        assert_eq!(
            f.clone().into_snapshot().session_name.as_deref(),
            Some("Projektin tilan tarkistus")
        );
        // A later tail with no title record keeps the title seen before.
        let p = write(
            "t-title.jsonl",
            &[r#"{"sessionId":"t-1"}"#, r#"{"type":"assistant"}"#],
        );
        assert_eq!(
            read_facts(&p).unwrap().title.as_deref(),
            Some("Projektin tilan tarkistus")
        );
    }

    /// A file under `root` with its modification time set `ago` in the past.
    fn aged(root: &Path, rel: &str, line: &str, ago: Duration) -> PathBuf {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        let mut f = File::create(&p).unwrap();
        writeln!(f, "{line}").unwrap();
        f.set_modified(SystemTime::now() - ago).unwrap();
        p
    }

    #[test]
    fn a_session_waiting_on_its_agents_stays_live() {
        let root = std::env::temp_dir().join(format!("rg-recent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let min = |m: u64| Duration::from_secs(m * 60);
        // Waiting: its own file 27 minutes still, an agent wrote a minute ago.
        aged(&root, "p/w.jsonl", r#"{"sessionId":"w"}"#, min(27));
        aged(
            &root,
            "p/w/subagents/agent-1.jsonl",
            r#"{"sessionId":"w"}"#,
            min(1),
        );
        // Ended: its own file and its agent both long still.
        aged(&root, "p/e.jsonl", r#"{"sessionId":"e"}"#, min(40));
        aged(
            &root,
            "p/e/subagents/agent-2.jsonl",
            r#"{"sessionId":"e"}"#,
            min(35),
        );
        // Plain and live, no agents at all.
        aged(&root, "q/l.jsonl", r#"{"sessionId":"l"}"#, min(2));
        // Silent past the look-back: its agents are not even looked at.
        aged(&root, "q/o.jsonl", r#"{"sessionId":"o"}"#, min(24 * 60));
        aged(
            &root,
            "q/o/subagents/agent-3.jsonl",
            r#"{"sessionId":"o"}"#,
            min(1),
        );

        let mut found = recent_in(
            &root,
            epoch_ms_before(min(10)),
            epoch_ms_before(SUBAGENT_LOOKBACK),
        );
        found.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        let ids: Vec<&str> = found.iter().map(|f| f.session_id.as_str()).collect();
        assert_eq!(ids, ["l", "w"]);
        // The waiting session's age is its agent's write, not its own file's.
        let w = &found[1];
        assert!(w.modified_ms >= epoch_ms_before(min(2)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_file_yields_nothing_rather_than_panicking() {
        assert!(read_facts(Path::new("does-not-exist.jsonl")).is_none());
    }

    #[test]
    fn a_transcript_snapshot_carries_no_quota() {
        let p = write(
            "e.jsonl",
            &[r#"{"sessionId":"e-1","entrypoint":"claude-desktop"}"#],
        );
        let s = read_facts(&p).unwrap().into_snapshot();
        assert_eq!(s.surface, Surface::Desktop);
        assert_eq!(s.origin, Origin::Transcript);
        assert!(s.rate_limits.is_none(), "this is why the gauge is borrowed");
    }
}
