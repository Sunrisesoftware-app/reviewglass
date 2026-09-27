//! The frame (adr.rg.026): inside the Claude app the glass takes its frame from the app.
//!
//! While the glass is shown in Follow with the column lock on, a left click inside a
//! window of the desktop app locks the glass to the pane under the click: the Code
//! pane's rectangle as UI Automation reports it, or the page of a session in a window of
//! its own. The engine then keeps Follow's box inside that frame (`Engine::set_frame`),
//! the finder draws the frame on screen, and the locked pane's session becomes the
//! Diff tab's choice. A click on another pane moves the lock; the glass's bar releases
//! it; it ends by itself when the pane is gone.
//!
//! The click is observed, never intercepted: the primary button's state is polled —
//! nothing is hooked, a click is never swallowed, delayed or recorded — and a click
//! outside the app's windows does nothing. What is read through UI Automation is what
//! adr.rg.022 reads: the element at the click and its ancestors up to the pane (class
//! names and rectangles), and the names on the pane's header row.
//!
//! While locked, the pane's rectangle is read again twice a second (the element is
//! kept, so a moved or resized window moves the frame), and the windows above the app's
//! are looked at four times a second: a visible window of another application over the
//! box holds the glass on its last picture, and the bar says whose it is.

use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::capture::{cursor_pos, Engine, Mode, PaneFrame, SourceRect};
use crate::config::Store;

/// Event to the glass when the frame's status changes.
pub const EVENT: &str = "frame:status";
/// How often the button is looked at: a click lasts 50–150 ms.
const TICK: Duration = Duration::from_millis(20);
/// How often a locked pane's rectangle is read again.
const REREAD: Duration = Duration::from_millis(500);
/// How often the windows in front are looked at.
const COVER_CHECK: Duration = Duration::from_millis(250);
/// A note on the bar (no pane at the click, the pane gone) fades after this.
const NOTE_FOR: Duration = Duration::from_secs(6);
/// The first read of a new point can answer with a coarser container than the element
/// under it (measured 27.9.2026); a second read a moment later is exact.
const RETRY_AFTER: Duration = Duration::from_millis(40);

/// What the glass's bar says about the frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct FrameStatus {
    /// A pane is locked.
    pub locked: bool,
    /// The locked pane's session title, when its header (or its window) names one.
    pub title: Option<String>,
    /// The locked pane is a session in a window of its own.
    pub own_window: bool,
    /// Whose window covers the box, when one does.
    pub covered_by: Option<String>,
    /// The cursor is over the Claude app with nothing locked: a click would lock.
    pub in_app: bool,
    /// Why the last click locked nothing, or why the lock ended, in the user's terms.
    pub note: Option<String>,
}

#[derive(Default)]
pub struct FrameState {
    status: Mutex<FrameStatus>,
    /// Set by the bar's release button; the loop lets go on its next tick.
    release: Mutex<bool>,
}

impl FrameState {
    pub fn new() -> Self {
        Self::default()
    }
}

fn frame_of((l, t, r, b): (i32, i32, i32, i32)) -> PaneFrame {
    PaneFrame {
        left: l,
        top: t,
        right: r,
        bottom: b,
    }
}

/// Whether two rectangles share any area: `a` as (left, top, right, bottom), `b` the
/// engine's source rectangle.
fn overlaps((l, t, r, b): (i32, i32, i32, i32), s: SourceRect) -> bool {
    l < s.x + s.w as i32 && s.x < r && t < s.y + s.h as i32 && s.y < b
}

/// Start the loop. Called once from setup.
pub fn spawn(app: AppHandle) {
    thread::Builder::new()
        .name("reviewglass-frame".into())
        .spawn(move || run(app))
        .expect("frame thread");
}

#[cfg(not(windows))]
fn run(_app: AppHandle) {}

#[cfg(windows)]
fn run(app: AppHandle) {
    use crate::follow_session::uia;

    struct Lock {
        pane: uia::Locked,
        root: isize,
        claude_pid: u32,
        read_at: Instant,
    }

    let mut reader: Option<uia::Reader> = None;
    let mut lock: Option<Lock> = None;
    let mut was_down = false;
    let mut cover_at = Instant::now();
    let mut told = FrameStatus::default();
    let mut note_at = Instant::now();

    let tell = |status: &FrameStatus, told: &mut FrameStatus| {
        if status != told {
            *app.state::<FrameState>().status.lock() = status.clone();
            let _ = app.emit_to(crate::glass::GLASS_LABEL, EVENT, status.clone());
            *told = status.clone();
        }
    };

    loop {
        thread::sleep(TICK);
        let engine = app.state::<Engine>();
        let cfg = app.state::<Store>().get().glass;
        let active = engine.is_enabled() && engine.mode() == Mode::Follow && cfg.pane_lock;
        let released = std::mem::take(&mut *app.state::<FrameState>().release.lock());
        let mut status = told.clone();
        if status.note.is_some() && note_at.elapsed() >= NOTE_FOR {
            status.note = None;
        }

        if !active || released {
            if lock.take().is_some() || engine.frame().is_some() {
                engine.set_frame(None);
            }
            engine.set_in_app(false);
            was_down = false;
            status = FrameStatus::default();
            tell(&status, &mut told);
            continue;
        }

        let (x, y) = cursor_pos();
        let under = uia::root_at(x, y);
        engine.set_in_app(lock.is_none() && under.claude);
        status.in_app = lock.is_none() && under.claude;

        // A press of the primary button over the app: lock to the pane there.
        let down = primary_down();
        if down && !was_down && under.claude {
            if reader.is_none() {
                match uia::Reader::new() {
                    Ok(r) => reader = Some(r),
                    Err(e) => status.note = Some(e),
                }
            }
            if let Some(r) = reader.as_ref() {
                let found = r.lock_at(x, y).or_else(|| {
                    thread::sleep(RETRY_AFTER);
                    r.lock_at(x, y)
                });
                crate::measure::log(|| {
                    format!("frame click {x},{y} found={}", found.is_some() as u8)
                });
                match found {
                    Some(pane) => {
                        engine.set_frame(Some(frame_of(pane.rect)));
                        engine.set_covered(false);
                        if let Some(t) = pane.title.clone() {
                            crate::follow_session::announce(&app, t);
                        }
                        status = FrameStatus {
                            locked: true,
                            title: pane.title.clone(),
                            own_window: pane.own_window,
                            ..FrameStatus::default()
                        };
                        lock = Some(Lock {
                            pane,
                            root: under.hwnd,
                            claude_pid: under.pid,
                            read_at: Instant::now(),
                        });
                        cover_at = Instant::now() - COVER_CHECK;
                    }
                    None if lock.is_none() => {
                        status.note = Some("No Code pane at the click: click inside a pane to lock the glass to it".into());
                        note_at = Instant::now();
                    }
                    // Already locked, and the click was elsewhere in the app (the
                    // sidebar, a handle): the lock stays.
                    None => {}
                }
            }
        }
        was_down = down;

        if let Some(l) = lock.as_mut() {
            // The pane again: the window may have moved, the pane changed width.
            if l.read_at.elapsed() >= REREAD {
                l.read_at = Instant::now();
                match uia::rect_of(&l.pane.element) {
                    Some(rect) => {
                        if rect != l.pane.rect {
                            l.pane.rect = rect;
                            engine.set_frame(Some(frame_of(rect)));
                        }
                    }
                    None => {
                        lock = None;
                        engine.set_frame(None);
                        status = FrameStatus {
                            note: Some(
                                "The locked pane is gone; click a pane to lock the glass again"
                                    .into(),
                            ),
                            ..FrameStatus::default()
                        };
                        note_at = Instant::now();
                        tell(&status, &mut told);
                        continue;
                    }
                }
            }
            // The windows in front.
            if cover_at.elapsed() >= COVER_CHECK {
                cover_at = Instant::now();
                let by = covering(l.root, l.claude_pid, engine.source());
                engine.set_covered(by.is_some());
                status.covered_by = by;
            }
        }
        tell(&status, &mut told);
    }
}

/// The primary mouse button is down (the physical right button when the user has
/// swapped them).
#[cfg(windows)]
fn primary_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
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

/// The first window above `root` in the z-order that covers `src`: visible, not
/// minimised, not cloaked, not click-through, and of another application than the
/// Claude app and ReviewGlass. Its title, or its program's name. The app itself
/// minimised counts as covered.
#[cfg(windows)]
fn covering(root: isize, claude_pid: u32, src: SourceRect) -> Option<String> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindow, GetWindowLongW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
        IsIconic, IsWindowVisible, GWL_EXSTYLE, GW_HWNDPREV, WS_EX_TRANSPARENT,
    };

    let own = std::process::id();
    // SAFETY: window queries on handles the system returned, with buffers and sizes
    // valid for each call.
    unsafe {
        let root = HWND(root as *mut _);
        if IsIconic(root).as_bool() {
            return Some("the app is minimised".into());
        }
        let mut h = GetWindow(root, GW_HWNDPREV).unwrap_or_default();
        for _ in 0..512 {
            if h.is_invalid() {
                break;
            }
            let next = GetWindow(h, GW_HWNDPREV).unwrap_or_default();
            let mut pid = 0u32;
            GetWindowThreadProcessId(h, Some(&mut pid));
            let mut cloaked = 0u32;
            let _ = DwmGetWindowAttribute(
                h,
                DWMWA_CLOAKED,
                (&mut cloaked as *mut u32).cast(),
                std::mem::size_of::<u32>() as u32,
            );
            let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
            let candidate = IsWindowVisible(h).as_bool()
                && !IsIconic(h).as_bool()
                && cloaked == 0
                && ex & WS_EX_TRANSPARENT.0 == 0
                && pid != own
                && pid != claude_pid
                && pid != 0;
            if candidate {
                let mut r = RECT::default();
                let got = DwmGetWindowAttribute(
                    h,
                    DWMWA_EXTENDED_FRAME_BOUNDS,
                    (&mut r as *mut RECT).cast(),
                    std::mem::size_of::<RECT>() as u32,
                )
                .is_ok()
                    || GetWindowRect(h, &mut r).is_ok();
                if got && overlaps((r.left, r.top, r.right, r.bottom), src) {
                    let mut buf = [0u16; 128];
                    let n = GetWindowTextW(h, &mut buf);
                    let title = String::from_utf16_lossy(&buf[..n.max(0) as usize])
                        .trim()
                        .to_string();
                    return Some(if title.is_empty() {
                        "another window".into()
                    } else {
                        title
                    });
                }
            }
            h = next;
        }
    }
    None
}

// ---- commands -------------------------------------------------------------

/// The frame's status, for the glass's bar on mount.
#[tauri::command]
pub fn frame_state(state: State<FrameState>) -> FrameStatus {
    state.status.lock().clone()
}

/// Release the lock (the bar's button). The next click on a pane locks again.
#[tauri::command]
pub fn frame_release(state: State<FrameState>) {
    *state.release.lock() = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_covers_the_box_only_where_they_share_area() {
        let src = SourceRect {
            x: 1000,
            y: 500,
            w: 264,
            h: 120,
        };
        assert!(overlaps((1200, 550, 1300, 560), src));
        // Touching edges are not covering.
        assert!(!overlaps((1264, 500, 1400, 620), src));
        assert!(!overlaps((0, 0, 1000, 500), src));
        // A window around the whole box covers it.
        assert!(overlaps((0, 0, 2560, 1392), src));
    }

    #[test]
    fn a_status_change_is_a_change_and_a_repeat_is_not() {
        let a = FrameStatus {
            locked: true,
            title: Some("Atlas".into()),
            ..FrameStatus::default()
        };
        let mut b = a.clone();
        assert_eq!(a, b);
        b.covered_by = Some("Downloads".into());
        assert_ne!(a, b);
    }

    /// What a click would lock at the points in RG_PANE_AT ("x,y;x,y"), and whether a
    /// box there is covered. Read-only, no mouse: run with `cargo test --lib live_lock
    /// -- --ignored --nocapture` when the frame locks to the wrong thing.
    #[cfg(windows)]
    #[test]
    #[ignore = "diagnostic: reads this machine's Claude windows"]
    fn live_lock() {
        use crate::follow_session::uia;
        let points = std::env::var("RG_PANE_AT").unwrap_or_else(|_| "1200,700".into());
        let r = uia::Reader::new().expect("UI Automation");
        for p in points.split(';') {
            let mut it = p.split(',').map(|v| v.trim().parse::<i32>().unwrap_or(0));
            let (x, y) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
            let root = uia::root_at(x, y);
            let t0 = Instant::now();
            let found = r.lock_at(x, y).or_else(|| {
                thread::sleep(RETRY_AFTER);
                r.lock_at(x, y)
            });
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            match found {
                Some(l) => println!(
                    "({x},{y}) claude={} locks {:?} title={:?} own_window={} in {ms:.1} ms",
                    root.claude, l.rect, l.title, l.own_window
                ),
                None => println!("({x},{y}) claude={} locks nothing ({ms:.1} ms)", root.claude),
            }
            if root.claude {
                let src = SourceRect {
                    x: x - 132,
                    y: y - 60,
                    w: 264,
                    h: 120,
                };
                println!("   covered: {:?}", covering(root.hwnd, root.pid, src));
            }
        }
    }

    #[test]
    fn a_pane_rectangle_becomes_the_engines_frame() {
        assert_eq!(
            frame_of((409, 175, 937, 1305)),
            PaneFrame {
                left: 409,
                top: 175,
                right: 937,
                bottom: 1305
            }
        );
    }
}
