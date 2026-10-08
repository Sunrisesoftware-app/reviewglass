//! The stall log: what the glass's picture waited on when it stuttered.
//!
//! The owner's report of 29.9.2026: now and then the text in the glass stutters and
//! lags while the same text appears in the Claude app's input without delay, and after
//! a moment it eases. So the wait is in ReviewGlass's own path from the screen to the
//! canvas, not in the machine or in Claude. This log times each stage of that path and
//! writes a line only when one of them is slow, so a day of ordinary use leaves a short
//! file that says which stage it was:
//!
//! - `late`: the compositor's frame reached the capture thread late (the frame's own
//!   timestamp against the clock);
//! - `crop`: the GPU-to-CPU copy of the box took long (a staging texture and a map
//!   that waits on the GPU);
//! - `main`: the core's main thread answered a probe late. Every synchronous command
//!   runs there, the glass's frame poll among them, so a busy main thread is a glass
//!   with no new picture;
//! - `cmd`: a synchronous command that ran long on the main thread, by name;
//! - `uia`: a UI Automation read on the frame thread that took long;
//! - `covered`: a window of another application over the box held the picture, and for
//!   how long;
//! - `js-*`: the glass page's side, sent over `stall_note`: a frame poll's round trip, a
//!   timer that fired late (the page was busy), a slow draw, a long task;
//! - `minute`: one summary a minute while the glass is shown, from each side.
//!
//! Structure only: timings, sizes, counts and command names — never pixels, never text
//! from a pane. Always on, because a stutter cannot be asked to wait for a Record
//! button; bounded, because an always-on log must not grow: past `MAX_BYTES` the file
//! becomes `stall.log.1` and a new one starts. Under `~/.reviewglass`, never AppData
//! (adr.rg.019): the agent that reads it runs inside the packaged Claude app.
//!
//! One line per event: `<local time> <kind> <fields>`, local time so the owner's "it
//! stuttered at 14:32" finds its line.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{AppHandle, Manager};

/// Past this the file is rolled over: a few days of stalls, not a disk.
const MAX_BYTES: u64 = 2 * 1024 * 1024;
/// How often the main thread is probed while the glass is shown.
const PROBE_EVERY: Duration = Duration::from_millis(100);
/// A main-thread answer slower than this is a stall the glass can show: at 30 frames a
/// second three frames have been missed.
pub const MAIN_SLOW: Duration = Duration::from_millis(100);
/// A synchronous command slower than this is written with its name.
pub const CMD_SLOW: Duration = Duration::from_millis(30);
/// A crop slower than this is written.
pub const CROP_SLOW: Duration = Duration::from_millis(40);
/// A frame that reached the capture thread later than this after the compositor made
/// it is written.
pub const LATE_SLOW: Duration = Duration::from_millis(100);
/// A UI Automation read slower than this is written.
pub const UIA_SLOW: Duration = Duration::from_millis(50);
/// A probe the main thread has not answered in this long is written as such, and the
/// probe moves on.
const PROBE_GIVE_UP: Duration = Duration::from_secs(10);

static FILE: Mutex<Option<File>> = Mutex::new(None);
static NOTES: AtomicU64 = AtomicU64::new(0);

/// The minute's figures from the core's side, reset by each summary.
struct Minute {
    arrived: AtomicU64,
    published: AtomicU64,
    crop_max_us: AtomicU64,
    late_max_us: AtomicU64,
    main_max_us: AtomicU64,
    cmd_max_us: AtomicU64,
}

static MINUTE: Minute = Minute {
    arrived: AtomicU64::new(0),
    published: AtomicU64::new(0),
    crop_max_us: AtomicU64::new(0),
    late_max_us: AtomicU64::new(0),
    main_max_us: AtomicU64::new(0),
    cmd_max_us: AtomicU64::new(0),
};

pub fn path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".reviewglass").join("stall.log"))
}

/// The lines written since the app started.
pub fn notes() -> u64 {
    NOTES.load(Ordering::Relaxed)
}

fn local_time() -> String {
    #[cfg(windows)]
    {
        // SAFETY: GetLocalTime has no preconditions and fills a plain struct.
        let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
        )
    }
    #[cfg(not(windows))]
    {
        String::new()
    }
}

fn open() -> Option<File> {
    let p = path()?;
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(&p).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(&p, p.with_extension("log.1"));
    }
    OpenOptions::new().create(true).append(true).open(&p).ok()
}

/// One line: `<local time> <kind> <fields>`. A line break in the fields is replaced, so
/// a line stays a line.
pub fn note(kind: &str, fields: impl AsRef<str>) {
    let fields = fields.as_ref().replace(['\r', '\n'], " ");
    let line = format!("{} {kind} {fields}\n", local_time());
    let mut file = FILE.lock();
    if file.is_none() {
        *file = open();
    }
    let Some(f) = file.as_mut() else { return };
    if f.write_all(line.as_bytes()).is_err() {
        *file = None;
        return;
    }
    NOTES.fetch_add(1, Ordering::Relaxed);
    // Roll over on the next line once the file is past its size.
    if f.metadata().is_ok_and(|m| m.len() > MAX_BYTES) {
        *file = None;
    }
}

fn max_into(slot: &AtomicU64, d: Duration) {
    slot.fetch_max(d.as_micros() as u64, Ordering::Relaxed);
}

fn ms(d: Duration) -> String {
    format!("{:.0}", d.as_secs_f64() * 1000.0)
}

/// A frame reached the capture thread `late` after the compositor made it.
pub fn frame_arrived(late: Option<Duration>) {
    MINUTE.arrived.fetch_add(1, Ordering::Relaxed);
    if let Some(late) = late {
        max_into(&MINUTE.late_max_us, late);
        if late >= LATE_SLOW {
            note("late", format!("ms={}", ms(late)));
        }
    }
}

/// A crop of `w`×`h` took `took`.
pub fn crop(took: Duration, what: &str, w: u32, h: u32) {
    max_into(&MINUTE.crop_max_us, took);
    if took >= CROP_SLOW {
        note("crop", format!("ms={} what={what} size={w}x{h}", ms(took)));
    }
}

pub fn frame_published() {
    MINUTE.published.fetch_add(1, Ordering::Relaxed);
}

/// Time a synchronous command's body; a slow one is written with its name and what
/// `detail` says about its size.
pub fn command<T>(name: &str, body: impl FnOnce() -> T, detail: impl FnOnce(&T) -> String) -> T {
    let t0 = Instant::now();
    let out = body();
    let took = t0.elapsed();
    max_into(&MINUTE.cmd_max_us, took);
    if took >= CMD_SLOW {
        note(
            "cmd",
            format!("name={name} ms={} {}", ms(took), detail(&out)),
        );
    }
    out
}

/// Time a UI Automation read on the frame thread.
pub fn uia<T>(what: &str, body: impl FnOnce() -> T) -> T {
    let t0 = Instant::now();
    let out = body();
    let took = t0.elapsed();
    if took >= UIA_SLOW {
        note("uia", format!("what={what} ms={}", ms(took)));
    }
    out
}

/// A note from the glass page (`js-*` kinds only; the fields are the page's own figures).
#[tauri::command]
pub async fn stall_note(kind: String, fields: String) {
    if !kind.starts_with("js-") || kind.len() > 24 || kind.contains(char::is_whitespace) {
        return;
    }
    let fields: String = fields.chars().take(300).collect();
    note(&kind, fields);
}

/// Where the log is and how many lines this run wrote, for the Settings tab.
#[derive(serde::Serialize)]
pub struct StallLogView {
    pub path: Option<String>,
    pub notes: u64,
}

#[tauri::command]
pub fn stall_log_state() -> StallLogView {
    StallLogView {
        path: path().map(|p| p.to_string_lossy().into_owned()),
        notes: notes(),
    }
}

/// Show the log in its folder (Explorer, the file selected). No argument: this opens
/// the stall log and nothing else.
#[tauri::command]
pub fn stall_log_show() -> Result<(), String> {
    let p = path().ok_or("no home folder")?;
    if !p.is_file() {
        return Err("no stall log yet".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", p.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The main-thread probe and the core's minute summary. Asleep while the glass is
/// hidden: a hidden glass has no picture to stall, and the hidden app's idle cost was
/// cut to almost nothing on 28.9.2026.
pub fn spawn(app: AppHandle) {
    note("start", format!("build={}", crate::glass::build_stamp()));
    thread::Builder::new()
        .name("reviewglass-stall-probe".into())
        .spawn(move || {
            let mut minute_at = Instant::now();
            let mut shown_this_minute = false;
            loop {
                thread::sleep(PROBE_EVERY);
                if crate::shutting_down() {
                    return;
                }
                let shown = app.state::<crate::capture::Engine>().is_enabled();
                if shown {
                    shown_this_minute = true;
                    let sent = Instant::now();
                    let (tx, rx) = mpsc::channel();
                    let queued = app.run_on_main_thread(move || {
                        let _ = tx.send(());
                    });
                    if queued.is_ok() {
                        match rx.recv_timeout(PROBE_GIVE_UP) {
                            Ok(()) => {
                                let waited = sent.elapsed();
                                max_into(&MINUTE.main_max_us, waited);
                                if waited >= MAIN_SLOW {
                                    note("main", format!("ms={}", ms(waited)));
                                }
                            }
                            Err(_) => note("main", format!("ms=>{}", PROBE_GIVE_UP.as_millis())),
                        }
                    }
                }
                if minute_at.elapsed() >= Duration::from_secs(60) {
                    if shown_this_minute {
                        let take = |a: &AtomicU64| a.swap(0, Ordering::Relaxed);
                        let us = |v: u64| format!("{:.0}", v as f64 / 1000.0);
                        note(
                            "minute",
                            format!(
                                "side=core arrived={} published={} crop_max={} late_max={} main_max={} cmd_max={}",
                                take(&MINUTE.arrived),
                                take(&MINUTE.published),
                                us(take(&MINUTE.crop_max_us)),
                                us(take(&MINUTE.late_max_us)),
                                us(take(&MINUTE.main_max_us)),
                                us(take(&MINUTE.cmd_max_us)),
                            ),
                        );
                    } else {
                        for a in [
                            &MINUTE.arrived,
                            &MINUTE.published,
                            &MINUTE.crop_max_us,
                            &MINUTE.late_max_us,
                            &MINUTE.main_max_us,
                            &MINUTE.cmd_max_us,
                        ] {
                            a.store(0, Ordering::Relaxed);
                        }
                    }
                    minute_at = Instant::now();
                    shown_this_minute = false;
                }
            }
        })
        .expect("stall probe thread");
}
