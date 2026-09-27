//! The events half of the spool watcher (rg.spool-watcher): what the hook collector
//! left in `spool/events/`, read into `ChangeEvent`s.
//!
//! One file per edit, written atomically by `reviewglass-hook.exe`, so a reader never
//! sees a half file — but it may see a file the collector is about to rename over, or
//! one written by a version of the collector this build does not know, so every field
//! is read leniently (adr.rg.011) and a file that will not parse is retried once and
//! then left for the collector's own prune.
//!
//! Events are read, never deleted: they stay for the hour the hook keeps them
//! (`spool::EVENT_TTL`; the hook prunes older ones on every run), and the reader
//! remembers the names it has read. So a restart — an update, a reboot, a crash —
//! finds the last hour's edits again (adr.rg.024), whether or not an app was running
//! when they were made. Deleting on read, as the reader first did, left a restarted
//! app an empty list (measured 27.9.2026: the previous instance had consumed every
//! event, and the new one showed "No edit yet" with four sessions at work).

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

/// One edit an agent made, as the hook reported it. Every field but `ts` may be
/// absent.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct ChangeEvent {
    /// When the hook ran, epoch milliseconds.
    pub ts: u64,
    pub session_id: Option<String>,
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub tool: Option<String>,
    pub tool_use_id: Option<String>,
    /// The edited file as Claude Code named it: absolute, in the platform's form.
    pub file_path: Option<String>,
}

/// What one pass over the events directory found.
#[derive(Debug, Default)]
pub struct Batch {
    /// Events at or after `since_ms` not read before, oldest first.
    pub events: Vec<ChangeEvent>,
    /// Files that could not be parsed on this pass (left in place, tried again).
    pub unreadable: usize,
    /// Events from before `since_ms`, passed over without being reported.
    pub stale: usize,
}

/// Read every event file in `dir` whose name is not in `seen`, and add the names read
/// to it. Nothing is deleted. Events with `ts < since_ms` are passed over silently; a
/// file that will not parse is not marked, so the next pass tries it again. Names of
/// files that have gone (pruned by the hook) are forgotten.
pub fn take(dir: &Path, since_ms: u64, seen: &mut HashSet<String>) -> Batch {
    let mut batch = Batch::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return batch;
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    // Oldest first: the names begin with the millisecond.
    paths.sort();
    let present: HashSet<String> = paths
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    seen.retain(|n| present.contains(n));
    for path in paths {
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if seen.contains(&name) {
            continue;
        }
        match read_event(&path) {
            Some(ev) => {
                seen.insert(name);
                if ev.ts < since_ms {
                    batch.stale += 1;
                } else {
                    batch.events.push(ev);
                }
            }
            None => batch.unreadable += 1,
        }
    }
    batch
}

/// Read one event file, retrying once. `None` when it does not parse either time; the
/// file is left where it is.
fn read_event(path: &Path) -> Option<ChangeEvent> {
    for attempt in 0..2 {
        if let Ok(bytes) = fs::read(path) {
            if let Some(ev) = parse(&bytes) {
                return Some(ev);
            }
        }
        if attempt == 0 {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    None
}

/// Field by field: a missing or mistyped field is absent, never a failure. Only `ts`
/// is required, since a file with no timestamp cannot be placed before or after the
/// app's start.
fn parse(bytes: &[u8]) -> Option<ChangeEvent> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    let ts = v.get("ts").and_then(Value::as_u64)?;
    let s = |key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
    Some(ChangeEvent {
        ts,
        session_id: s("session_id"),
        transcript_path: s("transcript_path"),
        cwd: s("cwd"),
        tool: s("tool"),
        tool_use_id: s("tool_use_id"),
        file_path: s("file_path"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("reviewglass-events-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn events_are_read_oldest_first_once_and_left_for_a_restart() {
        let d = temp_dir("order");
        fs::write(
            d.join("2000-toolu_b.json"),
            br#"{"ts":2000,"session_id":"s","tool":"Edit","file_path":"E:\\p\\b.rs"}"#,
        )
        .unwrap();
        fs::write(
            d.join("1000-toolu_a.json"),
            br#"{"ts":1000,"session_id":"s","tool":"Write","file_path":"E:\\p\\a.rs"}"#,
        )
        .unwrap();
        let mut seen = HashSet::new();
        let b = take(&d, 0, &mut seen);
        assert_eq!(b.events.len(), 2);
        assert_eq!(b.events[0].file_path.as_deref(), Some("E:\\p\\a.rs"));
        assert_eq!(b.events[1].tool.as_deref(), Some("Edit"));
        // Read once: the next pass reports nothing, and the files are still there.
        assert!(take(&d, 0, &mut seen).events.is_empty());
        assert_eq!(
            fs::read_dir(&d).unwrap().count(),
            2,
            "events are left in place"
        );
        // A restart (a reader that has seen nothing) finds them again.
        assert_eq!(take(&d, 0, &mut HashSet::new()).events.len(), 2);
        // A file the hook pruned is forgotten.
        fs::remove_file(d.join("1000-toolu_a.json")).unwrap();
        take(&d, 0, &mut seen);
        assert_eq!(seen.len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn events_from_before_the_look_back_are_passed_over_silently() {
        let d = temp_dir("stale");
        fs::write(
            d.join("1000-toolu_a.json"),
            br#"{"ts":1000,"file_path":"x"}"#,
        )
        .unwrap();
        fs::write(
            d.join("5000-toolu_b.json"),
            br#"{"ts":5000,"file_path":"y"}"#,
        )
        .unwrap();
        let mut seen = HashSet::new();
        let b = take(&d, 3000, &mut seen);
        assert_eq!(b.stale, 1);
        assert_eq!(b.events.len(), 1);
        assert_eq!(b.events[0].file_path.as_deref(), Some("y"));
        // Passed over once, not counted again.
        assert_eq!(take(&d, 3000, &mut seen).stale, 0);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_that_will_not_parse_is_counted_and_left() {
        let d = temp_dir("bad");
        fs::write(d.join("1000-toolu_a.json"), b"not json").unwrap();
        // A wrong type on a field is absence, not failure; a missing ts is failure.
        fs::write(
            d.join("1001-toolu_b.json"),
            br#"{"ts":1001,"tool":42,"file_path":"z"}"#,
        )
        .unwrap();
        fs::write(d.join("1002-toolu_c.json"), br#"{"tool":"Edit"}"#).unwrap();
        fs::write(d.join("notes.txt"), b"ignored").unwrap();
        let mut seen = HashSet::new();
        let b = take(&d, 0, &mut seen);
        assert_eq!(b.unreadable, 2);
        assert_eq!(b.events.len(), 1);
        assert_eq!(b.events[0].tool, None);
        assert_eq!(b.events[0].file_path.as_deref(), Some("z"));
        let mut left: Vec<String> = fs::read_dir(&d)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec![
                "1000-toolu_a.json",
                "1001-toolu_b.json",
                "1002-toolu_c.json",
                "notes.txt"
            ]
        );
        // The unreadable ones are tried again on the next pass.
        assert_eq!(take(&d, 0, &mut seen).unreadable, 2);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_directory_is_an_empty_batch() {
        let b = take(Path::new("Z:\\no\\such\\dir"), 0, &mut HashSet::new());
        assert!(b.events.is_empty());
        assert_eq!(b.unreadable, 0);
    }
}
