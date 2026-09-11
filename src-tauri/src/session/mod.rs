//! Session source (rg.session-source): the boundary where the two Claude Code surfaces
//! are reconciled into one list of sessions.
//!
//! v1 ships both implementations, not one plus a fallback (adr.rg.003). statusLine runs
//! in the terminal CLI only, so `ClaudeStatusLineSource` reaches CLI sessions through
//! the spool, and `ClaudeTranscriptSource` reads the JSONL under `~/.claude/projects/`,
//! which is the only channel reaching a Desktop session at all — and the only place
//! `entrypoint` says which surface a session lives on.
//!
//! Consumers see one `Vec<SessionSnapshot>` and never learn which channel a session came
//! down. That is the whole point of the boundary: a Codex adapter, or a Claude Code that
//! starts running statusLine on Desktop, changes this module and nothing else.

pub mod payload;
pub mod snapshot;
pub mod transcript;

use std::collections::HashMap;
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use snapshot::{Origin, SessionSnapshot, Surface};

use crate::spool;
use payload::SpoolRecord;

/// How long after its last write a session is still considered live.
///
/// Claude Code writes no close record on either channel, so age is the only signal a
/// session ended. Ten minutes is long enough that a session sitting idle while its user
/// thinks does not vanish from the panel, and short enough that yesterday's work does
/// not crowd it.
pub const LIVE_TTL: Duration = Duration::from_secs(10 * 60);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Everything one read of both channels found, including what it could not read.
#[derive(Debug, Default)]
pub struct Reading {
    pub sessions: Vec<SessionSnapshot>,
    /// True once the collector has ever written a session record. The panel needs this
    /// to tell "no CLI session is running" from "the collector is not installed" — two
    /// empty tables with entirely different remedies.
    pub collector_installed: bool,
    /// Session files that could not be parsed on this pass. Reported rather than hidden:
    /// a session the panel silently drops looks identical to a session that ended.
    pub unreadable: usize,
}

/// Read both channels and reconcile them.
pub fn read_all() -> Reading {
    let mut reading = Reading::default();

    let mut from_status: HashMap<String, SessionSnapshot> = HashMap::new();
    let cutoff = now_ms().saturating_sub(LIVE_TTL.as_millis() as u64);

    if let Some(dir) = spool::sessions_dir() {
        if let Ok(entries) = fs::read_dir(&dir) {
            reading.collector_installed = true;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                match read_spool_record(&path) {
                    Some(record) => {
                        if record.observed_at_ms < cutoff {
                            continue;
                        }
                        let Some(id) = record.status.session_id.clone() else {
                            continue;
                        };
                        let snap = SessionSnapshot::from_status_line(
                            id.clone(),
                            record.observed_at_ms,
                            &record.status,
                        );
                        // Two records for one id should not happen; if it does, the
                        // newer observation is the one worth keeping.
                        from_status
                            .entry(id)
                            .and_modify(|existing| {
                                if snap.observed_at_ms > existing.observed_at_ms {
                                    *existing = snap.clone();
                                }
                            })
                            .or_insert(snap);
                    }
                    None => reading.unreadable += 1,
                }
            }
        }
    }

    let mut from_transcript: HashMap<String, SessionSnapshot> = transcript::recent(LIVE_TTL)
        .into_iter()
        .map(|f| {
            let s = f.into_snapshot();
            (s.session_id.clone(), s)
        })
        .collect();

    let mut ids: Vec<String> = from_status.keys().cloned().collect();
    for id in from_transcript.keys() {
        if !from_status.contains_key(id) {
            ids.push(id.clone());
        }
    }

    for id in ids {
        let Some(mut s) =
            SessionSnapshot::merge(from_status.remove(&id), from_transcript.remove(&id))
        else {
            continue;
        };
        // A session can be reporting statusLine while its transcript sits outside the
        // window the scan covers — the scan is for finding sessions, and this one was
        // already found. Its payload names its own transcript, so the surface is read
        // straight from the file rather than left unknown for want of a scan hit.
        if s.surface == Surface::Unknown {
            if let Some(path) = s.transcript_path.clone() {
                if let Some(facts) = transcript::read_facts(std::path::Path::new(&path)) {
                    s.surface = facts.surface;
                    s.cwd = s.cwd.or(facts.cwd);
                    s.version = s.version.or(facts.version);
                }
            }
        }
        reading.sessions.push(s);
    }
    // Busiest first: the panel's question is "which session is eating my limit", and the
    // answer should not need sorting by hand.
    reading
        .sessions
        .sort_by_key(|s| std::cmp::Reverse(s.observed_at_ms));
    reading
}

/// Read one spool record, retrying once. A file caught mid-rename is the normal case,
/// not an error: the collector writes atomically, so the retry sees the finished file.
fn read_spool_record(path: &std::path::Path) -> Option<SpoolRecord> {
    for attempt in 0..2 {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(record) = serde_json::from_slice::<SpoolRecord>(&bytes) {
                return Some(record);
            }
        }
        if attempt == 0 {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_is_long_enough_to_survive_thinking_and_short_enough_to_forget_yesterday() {
        assert!(LIVE_TTL >= Duration::from_secs(5 * 60));
        assert!(LIVE_TTL <= Duration::from_secs(30 * 60));
    }

    /// Prints what both channels see on this machine right now. Ignored by default
    /// because it asserts nothing about a machine it cannot control; run it with
    /// `cargo test -- --ignored --nocapture live_reading` when the panel looks wrong and
    /// you need to know whether the fault is in the reading or in the rendering.
    #[test]
    #[ignore = "diagnostic: reads this machine's real sessions"]
    fn live_reading() {
        let r = read_all();
        println!(
            "collector_installed={} unreadable={} sessions={}",
            r.collector_installed,
            r.unreadable,
            r.sessions.len()
        );
        for s in &r.sessions {
            println!(
                "  {:8} {:?} {:?} model={:?} cost={:?} ctx={:?} quota={}",
                &s.session_id[..s.session_id.len().min(8)],
                s.surface,
                s.origin,
                s.model_name.as_deref().unwrap_or("-"),
                s.cost_usd,
                s.context_used_pct,
                s.rate_limits.is_some()
            );
        }
    }

    #[test]
    fn reading_both_channels_never_panics_on_a_machine_with_neither() {
        // The fresh-install path: no spool, possibly no ~/.claude at all.
        let r = read_all();
        assert!(r.sessions.len() < 10_000);
    }
}
