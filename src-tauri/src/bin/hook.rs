//! ReviewGlass hook collector (rg.hook-collector).
//!
//! Claude Code runs this as a `PostToolUse` hook after every `Edit`, `Write`,
//! `MultiEdit` or `NotebookEdit`, with the hook's JSON on stdin. It writes one event
//! file into the spool and exits. That file is the live diff's trigger (P4): the app
//! sees it appear and runs `git diff` on the path it names.
//!
//! Rules, in order:
//!   1. Always exit 0. A PostToolUse hook's non-zero exit shows as a "blocking error"
//!      in the user's session without blocking anything — pure noise for a display
//!      feature.
//!   2. Always leave a trace. Even an unparseable payload writes an event with the
//!      fields it could not read absent and the reason present, because a hook that
//!      silently did nothing is indistinguishable from a hook that never ran — which
//!      is exactly the question the P4 spike asks per surface.
//!   3. Never store content. The hook input carries the edit itself (`old_string`,
//!      `new_string`, a `Write`'s whole `content`). Only the path, the tool and the
//!      session's identity reach the disk; the diff is read from git, on demand.
//!   4. Print nothing. Stdout of a PostToolUse hook is shown in the transcript in
//!      verbose mode; there is nothing to say.
//!   5. Leave the spool as small as it found it. An event is a trigger, not a record:
//!      one older than `EVENT_TTL` is of no use to a live view, and the hook runs
//!      whether or not ReviewGlass does, so it prunes the old ones itself rather than
//!      relying on a reader that may not be there.
//!
//! A native binary, not a shell script (adr.rg.010), for the same reason as the
//! statusLine collector: on Windows a hook command runs through whichever shell Claude
//! Code finds, and a path through a shell it cannot know is not a path.

use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

use reviewglass_lib::spool;

/// Events older than this are pruned on every run. An hour is long past any live use
/// and short enough that a spool nobody reads stays a handful of files.
const EVENT_TTL: Duration = Duration::from_secs(60 * 60);

/// What one hook invocation leaves in `spool/events/<ts>-<id>.json`. Every field but
/// `ts` may be absent: the payload is read leniently (adr.rg.011), field by field, and a
/// field of an unexpected type counts as absent.
#[derive(Debug, Default, Serialize)]
struct ChangeEvent {
    /// When the hook ran, epoch milliseconds.
    ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    /// The transcript this session writes: the only place its surface (Desktop or
    /// CLI) is recorded, so the app can join the event to the session it already
    /// knows from the transcript channel.
    #[serde(skip_serializing_if = "Option::is_none")]
    transcript_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hook_event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_use_id: Option<String>,
    /// `tool_input.file_path`, or `notebook_path` for NotebookEdit. As given by Claude
    /// Code: absolute, in the platform's own form.
    #[serde(skip_serializing_if = "Option::is_none")]
    file_path: Option<String>,
    /// Bytes read from stdin — evidence of an invocation even when nothing parsed.
    stdin_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_error: Option<String>,
}

fn main() {
    let mut raw = String::new();
    // A read failure means an empty payload and an event that says so.
    let _ = io::stdin().read_to_string(&mut raw);
    let event = read_event(&raw);
    store(&event);
    if let Some(dir) = spool::events_dir() {
        prune(&dir, event.ts);
    }
}

/// Pick the fields out of the payload, each on its own: a missing or mistyped field
/// is absent, never a failure of the whole event.
fn read_event(raw: &str) -> ChangeEvent {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut ev = ChangeEvent {
        ts,
        stdin_bytes: raw.len(),
        ..ChangeEvent::default()
    };
    let v: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            ev.parse_error = Some(e.to_string());
            return ev;
        }
    };
    let s = |key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
    ev.session_id = s("session_id");
    ev.transcript_path = s("transcript_path");
    ev.cwd = s("cwd");
    ev.hook_event = s("hook_event_name");
    ev.tool = s("tool_name");
    ev.tool_use_id = s("tool_use_id");
    ev.file_path = v
        .get("tool_input")
        .and_then(|i| {
            i.get("file_path")
                .or_else(|| i.get("notebook_path"))
                .and_then(Value::as_str)
        })
        .map(str::to_string);
    ev
}

/// One file per invocation, named so two hooks running at once cannot collide: the
/// millisecond and the tool call's own id, or this process's id when the payload had
/// none. Written atomically; the watcher never sees a half file.
fn store(ev: &ChangeEvent) {
    let Some(dir) = spool::events_dir() else {
        return;
    };
    let id = ev
        .tool_use_id
        .as_deref()
        .and_then(spool::safe_file_stem)
        .unwrap_or_else(|| format!("pid{}", std::process::id()));
    if let Ok(bytes) = serde_json::to_vec(ev) {
        let _ = spool::write_atomic(&dir.join(format!("{}-{id}.json", ev.ts)), &bytes);
    }
}

/// Remove event files whose timestamp (the leading number of the name) is older than
/// `EVENT_TTL` before `now_ms`. Anything that is not an event of ours is left alone.
fn prune(dir: &Path, now_ms: u64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let cutoff = now_ms.saturating_sub(EVENT_TTL.as_millis() as u64);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(ts) = name
            .strip_suffix(".json")
            .and_then(|n| n.split('-').next())
            .and_then(|t| t.parse::<u64>().ok())
        else {
            continue;
        };
        if ts < cutoff {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_documented_payload_and_keeps_no_content() {
        let ev = read_event(
            r#"{"session_id":"abc","transcript_path":"C:\\Users\\k\\.claude\\projects\\p\\abc.jsonl",
                "cwd":"E:\\proj","hook_event_name":"PostToolUse","tool_name":"Edit",
                "tool_use_id":"toolu_01X",
                "tool_input":{"file_path":"E:\\proj\\src\\a.rs","old_string":"secret","new_string":"more secret"},
                "tool_response":{"filePath":"E:\\proj\\src\\a.rs"}}"#,
        );
        assert_eq!(ev.session_id.as_deref(), Some("abc"));
        assert_eq!(ev.tool.as_deref(), Some("Edit"));
        assert_eq!(ev.tool_use_id.as_deref(), Some("toolu_01X"));
        assert_eq!(ev.file_path.as_deref(), Some("E:\\proj\\src\\a.rs"));
        assert!(ev.parse_error.is_none());
        let json = serde_json::to_string(&ev).unwrap();
        assert!(
            !json.contains("secret"),
            "the edit's content must never be stored"
        );
    }

    #[test]
    fn a_notebook_edit_names_its_notebook() {
        let ev = read_event(
            r#"{"tool_name":"NotebookEdit","tool_input":{"notebook_path":"E:\\n.ipynb"}}"#,
        );
        assert_eq!(ev.file_path.as_deref(), Some("E:\\n.ipynb"));
    }

    #[test]
    fn prune_removes_only_old_events_of_ours() {
        let d = std::env::temp_dir().join(format!("reviewglass-events-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let now: u64 = 10_000_000_000;
        let old = now - EVENT_TTL.as_millis() as u64 - 1;
        let fresh = now - 1000;
        fs::write(d.join(format!("{old}-toolu_a.json")), b"{}").unwrap();
        fs::write(d.join(format!("{fresh}-toolu_b.json")), b"{}").unwrap();
        fs::write(d.join("notes.txt"), b"keep").unwrap();
        fs::write(d.join("x-toolu_c.json"), b"{}").unwrap();
        prune(&d, now);
        let mut left: Vec<String> = fs::read_dir(&d)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec![
                format!("{fresh}-toolu_b.json"),
                "notes.txt".into(),
                "x-toolu_c.json".into()
            ]
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn wrong_types_and_garbage_still_leave_a_trace() {
        // A mistyped field is absent, the rest is read (adr.rg.011).
        let ev =
            read_event(r#"{"session_id":42,"tool_name":"Write","tool_input":{"file_path":["x"]}}"#);
        assert_eq!(ev.session_id, None);
        assert_eq!(ev.tool.as_deref(), Some("Write"));
        assert_eq!(ev.file_path, None);
        assert!(ev.parse_error.is_none());
        // Not JSON at all: the event records the failure and the byte count.
        let ev = read_event("not json");
        assert!(ev.parse_error.is_some());
        assert_eq!(ev.stdin_bytes, 8);
        assert_eq!(ev.tool, None);
        // Empty stdin likewise.
        let ev = read_event("");
        assert!(ev.parse_error.is_some());
        assert_eq!(ev.stdin_bytes, 0);
    }
}
