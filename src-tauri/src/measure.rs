//! A measurement log for the column detector and Fit — temporary tooling for one
//! conversation (session 3, observation 1: "Follow still drifts, and the text in the
//! glass changes width"). It records *structure only*: the cursor, the column's edges
//! as the detector read them, the source rectangle, the glass's size and zoom, and
//! Fit's decisions — never pixels and never text.
//!
//! A recording is started and stopped from the Settings tab like in any recorder: each
//! one is its own file under `%TEMP%`, named by its local start time, so a recording is
//! never overwritten by the next one or by a restart. A recording ends with the app.
//!
//! One line per event: `<seconds since start> <kind> <fields>`. The reader is
//! `scripts/analyze-follow-log.py`, so the format favours grep over prose.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use windows::Win32::System::SystemInformation::GetLocalTime;

static ON: AtomicBool = AtomicBool::new(false);
static LINES: AtomicU64 = AtomicU64::new(0);
/// Unix seconds the current recording started at; 0 when none is running.
static SINCE: AtomicU64 = AtomicU64::new(0);
static FILE: Mutex<Option<File>> = Mutex::new(None);
static T0: Mutex<Option<Instant>> = Mutex::new(None);
/// The file being written, or the last one written in this run.
static CURRENT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// A log that grew past this is not a measurement any more, and a forgotten recorder
/// must not fill a disk: the recording stops itself here (about three hours of use).
const MAX_LINES: u64 = 200_000;

pub fn is_on() -> bool {
    ON.load(Ordering::Relaxed)
}

pub fn lines() -> u64 {
    LINES.load(Ordering::Relaxed)
}

/// When the running recording started, unix seconds; none when not recording.
pub fn since() -> Option<u64> {
    match SINCE.load(Ordering::Relaxed) {
        0 => None,
        s => Some(s),
    }
}

/// The file being written, or the last one written in this run; none before the
/// first recording.
pub fn path() -> Option<PathBuf> {
    CURRENT.lock().clone()
}

/// A file name from the local clock, so recordings sort by time in the folder and a
/// name says when it was taken.
fn file_name() -> String {
    // SAFETY: GetLocalTime has no preconditions and fills a plain struct.
    let t = unsafe { GetLocalTime() };
    format!(
        "reviewglass-follow-{:04}{:02}{:02}-{:02}{:02}{:02}.log",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

/// Start a recording into a new file. The header carries what the caller knows about
/// the build and the settings that shape Fit.
pub fn start(header: &str) -> Result<PathBuf, String> {
    let p = std::env::temp_dir().join(file_name());
    let mut f = File::create(&p).map_err(|e| format!("cannot write {}: {e}", p.display()))?;
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = writeln!(
        f,
        "# ReviewGlass follow log; started at unix {epoch}; {header}"
    );
    let _ = writeln!(
        f,
        "# <t seconds since start> <kind> <fields>: scan (every detector run), src (the source rectangle, at most 10/s), \
         pane-event (what the glass was told), fit (Fit's decision, from the glass), view, resized, hover, hold, release, mode, lock"
    );
    let mut file = FILE.lock();
    *file = Some(f);
    *T0.lock() = Some(Instant::now());
    *CURRENT.lock() = Some(p.clone());
    LINES.store(0, Ordering::Relaxed);
    SINCE.store(epoch, Ordering::Relaxed);
    ON.store(true, Ordering::Relaxed);
    Ok(p)
}

/// Stop the recording; the file stays.
pub fn stop() {
    ON.store(false, Ordering::Relaxed);
    SINCE.store(0, Ordering::Relaxed);
    *FILE.lock() = None;
}

/// Append one line. The closure runs only while recording, so a call on a hot path
/// costs one atomic load when it is not.
pub fn log(line: impl FnOnce() -> String) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    if LINES.fetch_add(1, Ordering::Relaxed) >= MAX_LINES {
        if let Some(f) = FILE.lock().as_mut() {
            let _ = writeln!(f, "# stopped: {MAX_LINES} lines");
        }
        stop();
        return;
    }
    let t = T0
        .lock()
        .map(|t0| t0.elapsed().as_secs_f64())
        .unwrap_or(0.0);
    if let Some(f) = FILE.lock().as_mut() {
        let _ = writeln!(f, "{t:.3} {}", line());
    }
}
