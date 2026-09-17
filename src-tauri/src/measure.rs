//! A measurement log for the column detector and Fit — temporary tooling for one
//! conversation (session 3, observation 1: "Follow still drifts, and the text in the
//! glass changes width"). It records *structure only*: the cursor, the column's edges
//! as the detector read them, the source rectangle, the glass's size and zoom, and
//! Fit's decisions — never pixels and never text. Off by default; switched on from the
//! Settings tab; written under `%TEMP%`; started over at every switch-on and at every
//! start while on, so the file is always the latest run.
//!
//! One line per event: `<seconds since switch-on> <kind> <fields>`. The reader is a
//! script, so the format favours grep over prose.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

static ON: AtomicBool = AtomicBool::new(false);
static LINES: AtomicU64 = AtomicU64::new(0);
static FILE: Mutex<Option<File>> = Mutex::new(None);
static T0: Mutex<Option<Instant>> = Mutex::new(None);

/// A log that grew past this is not a measurement any more, and a forgotten switch
/// must not fill a disk: the log turns itself off here (about three hours of use).
const MAX_LINES: u64 = 200_000;

pub fn path() -> PathBuf {
    std::env::temp_dir().join("reviewglass-follow.log")
}

pub fn is_on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Switch the log on (the file is started over and gets a header) or off.
pub fn set_on(on: bool, header: &str) -> Result<(), String> {
    let mut file = FILE.lock();
    if !on {
        ON.store(false, Ordering::Relaxed);
        *file = None;
        return Ok(());
    }
    let p = path();
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
    *file = Some(f);
    *T0.lock() = Some(Instant::now());
    LINES.store(0, Ordering::Relaxed);
    ON.store(true, Ordering::Relaxed);
    Ok(())
}

/// Append one line. The closure runs only while the log is on, so a call on a hot
/// path costs one atomic load when it is off.
pub fn log(line: impl FnOnce() -> String) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    if LINES.fetch_add(1, Ordering::Relaxed) >= MAX_LINES {
        ON.store(false, Ordering::Relaxed);
        if let Some(f) = FILE.lock().as_mut() {
            let _ = writeln!(f, "# stopped: {MAX_LINES} lines");
        }
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
