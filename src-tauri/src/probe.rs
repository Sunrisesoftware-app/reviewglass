//! The structure recorder (adr.rg.032): what other applications expose about the place
//! a user clicks, measured before P9's lock is built.
//!
//! Started and stopped from Settings > Measurements, and stopping by itself after
//! `MAX_FOR` or `MAX_CLICKS`. While it runs, a thread of its own watches the primary
//! button - observed, never intercepted - and at each press outside ReviewGlass and the
//! Claude app (which has its own reads, adr.rg.022 and adr.rg.026) it asks UI
//! Automation for the element at the click and walks its ancestors, writing for each one
//! its structure only: control type, localized type, class name (cut to 80
//! characters), framework, ARIA role, landmark type, rectangle and whether it is off
//! screen. Never a name, a value, help text, an automation id or any text: those carry
//! what the application shows. Applications on the exclusion list are not read at all.
//!
//! One file per recording, `~/.reviewglass/measurements/apps-<local time>.log`, never
//! under AppData (adr.rg.019): the agent that reads it runs inside the packaged Claude
//! app. One block per click:
//!
//! ```text
//! 12.345 click app=chrome.exe at=812,440 window=0,0,2560,1392 depth=17 read_ms=3.2
//!   0 ct=Text lct="text" fw=Chrome role="" lm=0 off=0 rect=790,431,1210,452 cls=""
//!   1 ct=Group lct="paragraph" fw=Chrome role="paragraph" lm=0 off=0 rect=... cls="..."
//! ```

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;

/// A recording stops by itself after this long: it is a day's measurement, not a habit.
const MAX_FOR: Duration = Duration::from_secs(8 * 60 * 60);
/// ...or after this many clicks.
const MAX_CLICKS: u64 = 2000;
/// Ancestors walked from the element at the click.
const MAX_DEPTH: usize = 40;
/// A class name longer than this is cut: a web page's class list can be long, and only
/// its start says what kind of element it is.
const CLASS_MAX: usize = 80;
/// How often the button is looked at while recording.
const POLL: Duration = Duration::from_millis(15);
/// Applications whose clicks are not read at all (adr.rg.032), by executable name.
const EXCLUDED: &[&str] = &[
    "keepass.exe",
    "keepassxc.exe",
    "1password.exe",
    "bitwarden.exe",
    "credentialuibroker.exe",
    "consent.exe",
];

static ON: AtomicBool = AtomicBool::new(false);
static CLICKS: AtomicU64 = AtomicU64::new(0);
static FILE: Mutex<Option<File>> = Mutex::new(None);
static STARTED: Mutex<Option<(Instant, u64)>> = Mutex::new(None);
static CURRENT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// What Settings shows about the recorder.
#[derive(Debug, Clone, Serialize)]
pub struct ProbeState {
    pub on: bool,
    /// Unix seconds the running recording started at.
    pub since: Option<u64>,
    pub clicks: u64,
    /// The file being written, or the last one written in this run.
    pub path: Option<String>,
}

fn state() -> ProbeState {
    ProbeState {
        on: ON.load(Ordering::Relaxed),
        since: STARTED.lock().map(|(_, s)| s),
        clicks: CLICKS.load(Ordering::Relaxed),
        path: CURRENT.lock().as_ref().map(|p| p.display().to_string()),
    }
}

fn dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".reviewglass").join("measurements"))
}

/// Whether an application is never read, by its executable's file name.
pub fn is_excluded(exe: &str) -> bool {
    EXCLUDED.iter().any(|e| e.eq_ignore_ascii_case(exe))
}

/// A class name as recorded: cut to `CLASS_MAX` characters, quotes and line breaks
/// replaced so a line stays a line.
pub fn class_for_log(class: &str) -> String {
    let cut: String = class.chars().take(CLASS_MAX).collect();
    let mut out = cut.replace(['"', '\r', '\n'], " ");
    if class.chars().count() > CLASS_MAX {
        out.push('…');
    }
    out
}

fn write_line(line: &str) {
    if let Some(f) = FILE.lock().as_mut() {
        let _ = writeln!(f, "{line}");
    }
}

fn start() -> Result<(), String> {
    let d = dir().ok_or("no home folder")?;
    std::fs::create_dir_all(&d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
    #[cfg(windows)]
    let stamp = {
        // SAFETY: GetLocalTime has no preconditions and fills a plain struct.
        let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        format!(
            "{:04}{:02}{:02}-{:02}{:02}{:02}",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
        )
    };
    #[cfg(not(windows))]
    let stamp = String::from("now");
    let p = d.join(format!("apps-{stamp}.log"));
    let mut f = File::create(&p).map_err(|e| format!("cannot write {}: {e}", p.display()))?;
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = writeln!(
        f,
        "# ReviewGlass structure recorder (adr.rg.032); build {}; started at unix {epoch}",
        crate::glass::build_stamp()
    );
    let _ = writeln!(
        f,
        "# structure only: ct (control type), lct (localized type), fw (framework), role (ARIA), lm (landmark), off (off screen), rect, cls (class, cut to {CLASS_MAX}); never names, values or text"
    );
    *FILE.lock() = Some(f);
    *CURRENT.lock() = Some(p);
    *STARTED.lock() = Some((Instant::now(), epoch));
    CLICKS.store(0, Ordering::Relaxed);
    ON.store(true, Ordering::Relaxed);
    #[cfg(windows)]
    thread::Builder::new()
        .name("reviewglass-structure-recorder".into())
        .spawn(run)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn stop(reason: &str) {
    if ON.swap(false, Ordering::Relaxed) {
        write_line(&format!("# stopped: {reason}"));
    }
    *FILE.lock() = None;
    *STARTED.lock() = None;
}

#[cfg(windows)]
fn run() {
    let reader = match read::Reader::new() {
        Ok(r) => r,
        Err(e) => {
            write_line(&format!("# UI Automation is not available: {e}"));
            stop("no UI Automation");
            return;
        }
    };
    let own = std::process::id();
    let mut was_down = false;
    while ON.load(Ordering::Relaxed) {
        thread::sleep(POLL);
        let started = STARTED.lock().map(|(t, _)| t);
        let Some(t0) = started else { break };
        if t0.elapsed() >= MAX_FOR {
            stop("eight hours");
            break;
        }
        let down = read::primary_down();
        if down && !was_down {
            let (x, y) = crate::capture::cursor_pos();
            if let Some(block) = reader.click(x, y, own, t0) {
                write_line(&block);
                if CLICKS.fetch_add(1, Ordering::Relaxed) + 1 >= MAX_CLICKS {
                    stop("2000 clicks");
                }
            }
        }
        was_down = down;
    }
}

#[tauri::command]
pub fn probe_state() -> ProbeState {
    state()
}

#[tauri::command]
pub fn probe_set(on: bool) -> Result<ProbeState, String> {
    if on && !ON.load(Ordering::Relaxed) {
        start()?;
    } else if !on {
        stop("stopped by the user");
    }
    Ok(state())
}

/// Show the recording in its folder (Explorer, the file selected): the current or the
/// last one, nothing else.
#[tauri::command]
pub fn probe_show() -> Result<(), String> {
    let p = CURRENT.lock().clone().ok_or("no recording yet")?;
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", p.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
mod read {
    use std::time::Instant;

    use windows::core::Interface;
    use windows::Win32::Foundation::{CloseHandle, POINT};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
        UIA_LandmarkTypePropertyId,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetAncestor, GetWindowRect, GetWindowThreadProcessId, WindowFromPoint, GA_ROOT,
    };

    use super::{class_for_log, is_excluded, MAX_DEPTH};

    pub fn primary_down() -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SWAPBUTTON};
        // SAFETY: plain state queries.
        unsafe {
            let vk = if GetSystemMetrics(SM_SWAPBUTTON) != 0 {
                VK_RBUTTON
            } else {
                VK_LBUTTON
            };
            (GetAsyncKeyState(vk.0 as i32) as u16) & 0x8000 != 0
        }
    }

    /// The executable's file name of a process, lower case.
    fn exe_of(pid: u32) -> Option<String> {
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        // SAFETY: a limited query handle, closed before return; the buffer and its
        // length are valid for the call.
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let ok = QueryFullProcessImageNameW(
                h,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(h);
            ok.ok()?;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/'])
            .next()
            .map(|f| f.to_ascii_lowercase())
    }

    pub struct Reader {
        uia: IUIAutomation,
        raw: IUIAutomationTreeWalker,
    }

    impl Reader {
        pub fn new() -> Result<Self, String> {
            // SAFETY: COM for this thread (a thread of its own), multithreaded; the
            // instance and the walker are used only here.
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED)
                    .ok()
                    .map_err(|e| format!("COM could not start: {e}"))?;
                let uia: IUIAutomation =
                    CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                        .map_err(|e| e.to_string())?;
                let raw = uia.RawViewWalker().map_err(|e| e.to_string())?;
                Ok(Self { uia, raw })
            }
        }

        /// One element's structure, as one line: never its name, value or text.
        fn line(&self, depth: usize, el: &IUIAutomationElement) -> String {
            // SAFETY: read-only properties of an element this thread holds.
            unsafe {
                let ct = el.CurrentControlType().map(|c| c.0).unwrap_or(0);
                let lct = el
                    .CurrentLocalizedControlType()
                    .map(|b| b.to_string())
                    .unwrap_or_default();
                let fw = el
                    .CurrentFrameworkId()
                    .map(|b| b.to_string())
                    .unwrap_or_default();
                let role = el
                    .CurrentAriaRole()
                    .map(|b| b.to_string())
                    .unwrap_or_default();
                let lm = el
                    .GetCurrentPropertyValue(UIA_LandmarkTypePropertyId)
                    .ok()
                    .and_then(|v| i32::try_from(&v).ok())
                    .unwrap_or(0);
                let off = el
                    .CurrentIsOffscreen()
                    .map(|b| b.as_bool())
                    .unwrap_or(false);
                let r = el.CurrentBoundingRectangle().unwrap_or_default();
                let cls = el
                    .CurrentClassName()
                    .map(|b| class_for_log(&b.to_string()))
                    .unwrap_or_default();
                format!(
                    "  {depth} ct={ct} lct=\"{}\" fw={fw} role=\"{}\" lm={lm} off={} rect={},{},{},{} cls=\"{cls}\"",
                    class_for_log(&lct),
                    class_for_log(&role),
                    off as u8,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top
                )
            }
        }

        /// The block for a click at (x, y): `None` over ReviewGlass's own windows, the
        /// Claude app (its reads are adr.rg.022's and adr.rg.026's) and nothing at all.
        pub fn click(&self, x: i32, y: i32, own: u32, t0: Instant) -> Option<String> {
            // SAFETY: plain window queries with a point and the handle they returned.
            let (pid, wr) = unsafe {
                let w = WindowFromPoint(POINT { x, y });
                let root = GetAncestor(w, GA_ROOT);
                let mut pid = 0u32;
                GetWindowThreadProcessId(root, Some(&mut pid));
                let mut r = Default::default();
                let _ = GetWindowRect(root, &mut r);
                (pid, r)
            };
            if pid == 0 || pid == own {
                return None;
            }
            let exe = exe_of(pid).unwrap_or_else(|| "unknown".into());
            if crate::follow_session::is_claude_image(&exe) {
                return None;
            }
            let t = t0.elapsed().as_secs_f64();
            let head = format!(
                "{t:.3} click app={exe} at={x},{y} window={},{},{},{}",
                wr.left,
                wr.top,
                wr.right - wr.left,
                wr.bottom - wr.top
            );
            if is_excluded(&exe) {
                return Some(format!("{head} excluded"));
            }
            let started = Instant::now();
            // SAFETY: a read-only query of the element at a screen point.
            let Ok(mut el) = (unsafe { self.uia.ElementFromPoint(POINT { x, y }) }) else {
                return Some(format!("{head} no-element"));
            };
            let mut lines = Vec::new();
            for depth in 0..MAX_DEPTH {
                lines.push(self.line(depth, &el));
                // SAFETY: tree navigation from an element this thread holds.
                match unsafe { self.raw.GetParentElement(&el) } {
                    Ok(p) if !p.as_raw().is_null() => el = p,
                    _ => break,
                }
            }
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            Some(format!(
                "{head} depth={} read_ms={ms:.1}\n{}",
                lines.len(),
                lines.join("\n")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_managers_and_credential_dialogs_are_never_read() {
        assert!(is_excluded("KeePassXC.exe"));
        assert!(is_excluded("consent.exe"));
        assert!(is_excluded("1Password.exe"));
        assert!(!is_excluded("chrome.exe"));
    }

    #[test]
    fn a_class_name_is_cut_and_kept_on_one_line() {
        let long = "a".repeat(200);
        let out = class_for_log(&long);
        assert_eq!(out.chars().count(), CLASS_MAX + 1);
        assert!(out.ends_with('…'));
        assert_eq!(class_for_log("x \"y\"\nz"), "x  y  z");
        assert_eq!(class_for_log(""), "");
    }
}
