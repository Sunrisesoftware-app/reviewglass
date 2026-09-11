//! File spool (rg.spool): the whole inter-process boundary.
//!
//! The collectors write here and the app reads. Plain files under the user profile —
//! no network listener, no localhost port, no IPC socket (adr.rg.004). N writers, one
//! reader, no locking: session records are replaced whole by an atomic rename, and
//! change events are separate files named so two writers cannot collide.
//!
//! This module is shared by the app and by the collector binaries, so both agree on
//! where the spool is without either configuring the other.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// `%APPDATA%/ReviewGlass/spool`, or the platform equivalent.
pub fn dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ReviewGlass").join("spool"))
}

pub fn sessions_dir() -> Option<PathBuf> {
    dir().map(|d| d.join("sessions"))
}

pub fn events_dir() -> Option<PathBuf> {
    dir().map(|d| d.join("events"))
}

/// Replace a file's contents in one step: write a temp file beside it, then rename.
/// A reader either sees the old file or the new one, never a half-written one — which
/// is what lets the watcher run without locking.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // The pid keeps two collectors writing the same session (which should not happen,
    // but costs nothing to survive) from sharing a temp file.
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    fs::write(&tmp, bytes)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// A session id as it arrives from Claude Code, reduced to something safe to use as a
/// file name. Returns `None` for an id with no usable characters at all, so a garbled
/// payload cannot write to a path of its choosing.
pub fn safe_file_stem(id: &str) -> Option<String> {
    let s: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(128)
        .collect();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_stem_keeps_uuids_and_refuses_paths() {
        assert_eq!(
            safe_file_stem("b99e7a1f-b52d-42f8-b3db-7858547ccca7").as_deref(),
            Some("b99e7a1f-b52d-42f8-b3db-7858547ccca7")
        );
        assert_eq!(
            safe_file_stem("../../etc/passwd").as_deref(),
            Some("etcpasswd")
        );
        assert_eq!(safe_file_stem("..").as_deref(), None);
        assert_eq!(safe_file_stem("").as_deref(), None);
    }

    #[test]
    fn atomic_write_leaves_no_temp_behind() {
        let d = std::env::temp_dir().join(format!("reviewglass-spool-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let f = d.join("a.json");
        write_atomic(&f, b"{\"a\":1}").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "{\"a\":1}");
        write_atomic(&f, b"{\"a\":2}").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "{\"a\":2}");
        let strays: Vec<_> = fs::read_dir(&d)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains("tmp"))
            .collect();
        assert!(strays.is_empty(), "temp files left behind: {strays:?}");
        let _ = fs::remove_dir_all(&d);
    }
}
