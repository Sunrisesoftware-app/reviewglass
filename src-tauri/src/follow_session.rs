//! Follow chooses the session (adr.rg.022): while the glass is shown in Follow, the
//! Code pane under the cursor is identified through UI Automation, by the title on its
//! header row, and the session with that title becomes the choice that filters the
//! Diff tab. The Diff tab follows the eye.
//!
//! What is read, and nothing more: the element under the cursor and its ancestors up
//! to the enclosing Code pane (class name and bounding rectangle), and the names of
//! elements at a few points on that pane's header row until one is named
//! "<title>, rename session". Nothing below the header row is read; nothing is written
//! into the desktop app; nothing is kept but the current pane's rectangle and title.
//!
//! A read happens only when the cursor leaves the pane it was last found in (or its
//! window changes), plus a re-check every few seconds; between reads the loop costs a
//! cursor query and a rectangle test. Never in Lens or Still, never while the glass is
//! hidden, never with the switch off.

use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::capture::{cursor_pos, Engine, Mode};
use crate::config::Store;
use crate::panel::PanelState;

/// Event to the dock when the pane under Follow names another session.
pub const EVENT: &str = "follow:session";
/// How often the loop looks at the cursor.
const POLL: Duration = Duration::from_millis(300);
/// A pane that still holds the cursor is read again after this: a session renamed, or
/// panes resized under a still cursor.
const RECHECK: Duration = Duration::from_secs(4);
/// The header button's name ends with this; the title is what comes before it.
const RENAME_SUFFIX: &str = ", rename session";
/// The class token the desktop app gives each Code pane.
const PANE_CLASS: &str = "dframe-pane";
/// Ancestors walked from the element under the cursor before giving up.
const MAX_DEPTH: usize = 40;
/// A point that is not in a Code pane (the sidebar, another app) is not asked again
/// until the cursor has moved this far from it, or `MISS_HOLD` has passed: outside the
/// panes the loop would otherwise read every poll.
const MISS_RADIUS: i32 = 40;
const MISS_HOLD: Duration = Duration::from_millis(1500);

/// What Follow last saw: the title on the pane's header, and the session it matched.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct FollowSaw {
    pub title: String,
    /// The matched session; `None` when no session in the table has this title.
    pub session_id: Option<String>,
    /// The session's name as the table shows it, when matched.
    pub name: Option<String>,
}

/// The loop's state, for the page: the last sighting and how the reads went.
#[derive(Default)]
pub struct FollowSessionState {
    inner: Mutex<Status>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Status {
    /// The switch (config `glass.follow_session`).
    pub on: bool,
    /// Follow is running now: the glass is shown, in Follow, not held.
    pub active: bool,
    pub saw: Option<FollowSaw>,
    /// UI Automation reads made since start, and the last one's duration: the cost the
    /// decision says is measured.
    pub reads: u64,
    pub last_read_ms: Option<f64>,
    /// Why no read is possible, when UI Automation itself could not be started.
    pub unavailable: Option<String>,
}

impl FollowSessionState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The title on a pane header button's name: what comes before ", rename session".
pub fn title_from_name(name: &str) -> Option<&str> {
    let t = name.strip_suffix(RENAME_SUFFIX)?.trim();
    (!t.is_empty()).then_some(t)
}

/// Whether an element's class names a Code pane: one of its space-separated tokens is
/// exactly the pane class.
pub fn is_pane_class(class: &str) -> bool {
    class.split_whitespace().any(|t| t == PANE_CLASS)
}

/// Points on a pane's header row, most likely first: the header's button sat 24 px
/// under the pane's top and 30 % in from its left on 20.9.2026. Every point is inside
/// the pane, at most 40 px from its top: nothing below the header row is ever asked.
pub fn header_points(left: i32, top: i32, right: i32, bottom: i32) -> Vec<(i32, i32)> {
    let w = (right - left).max(0);
    let mut out = Vec::new();
    for dy in [24, 16, 32, 40] {
        let y = top + dy;
        if y >= bottom {
            continue;
        }
        for fx in [0.30, 0.20, 0.45, 0.10, 0.60] {
            out.push((left + (w as f64 * fx) as i32, y));
        }
    }
    out
}

/// The session with this title: an exact match first, then one that differs only in
/// case. Titles are the transcript's `custom-title`, the same text the header shows.
pub fn match_title(title: &str, sessions: &[(String, Option<String>)]) -> Option<(String, String)> {
    let named = sessions
        .iter()
        .filter_map(|(id, n)| n.as_deref().map(|n| (id, n)));
    let exact = named.clone().find(|(_, n)| n.trim() == title);
    exact
        .or_else(|| {
            named
                .clone()
                .find(|(_, n)| n.trim().eq_ignore_ascii_case(title))
        })
        .map(|(id, n)| (id.clone(), n.to_string()))
}

/// Where the last read found no pane.
#[derive(Clone, Copy, Debug)]
struct Miss {
    at: (i32, i32),
    root: isize,
    when: Instant,
}

impl Miss {
    /// Still too close, in time and place, to the point that found nothing.
    fn holds(&self, (x, y): (i32, i32), root: isize, now: Instant) -> bool {
        root == self.root
            && (x - self.at.0).abs() <= MISS_RADIUS
            && (y - self.at.1).abs() <= MISS_RADIUS
            && now.duration_since(self.when) < MISS_HOLD
    }
}

/// The pane the cursor was last found in.
#[derive(Clone, Debug)]
struct Known {
    rect: (i32, i32, i32, i32),
    root: isize,
    at: Instant,
}

impl Known {
    /// No read is needed: the cursor is still in this pane's rectangle, over the same
    /// window, and the re-check is not due.
    fn holds(&self, (x, y): (i32, i32), root: isize, now: Instant) -> bool {
        let (l, t, r, b) = self.rect;
        root == self.root
            && x >= l
            && x < r
            && y >= t
            && y < b
            && now.duration_since(self.at) < RECHECK
    }
}

#[cfg(windows)]
mod uia {
    use windows::core::Interface;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetAncestor, GetWindowThreadProcessId, WindowFromPoint, GA_ROOT,
    };

    use super::{header_points, is_pane_class, title_from_name, MAX_DEPTH};

    /// A UI Automation client, on the thread that made it.
    pub struct Reader {
        uia: IUIAutomation,
        walker: IUIAutomationTreeWalker,
    }

    impl Reader {
        pub fn new() -> Result<Self, String> {
            // SAFETY: COM is initialised once on this thread (a thread of its own) for
            // the multithreaded apartment, which a UI Automation client may use; the
            // instance and the walker are used only on this thread.
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED)
                    .ok()
                    .map_err(|e| format!("COM could not start: {e}"))?;
                let uia: IUIAutomation =
                    CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                        .map_err(|e| format!("UI Automation is not available: {e}"))?;
                let walker = uia
                    .ControlViewWalker()
                    .map_err(|e| format!("UI Automation has no tree walker: {e}"))?;
                Ok(Self { uia, walker })
            }
        }

        fn at(&self, x: i32, y: i32) -> Option<IUIAutomationElement> {
            // SAFETY: a read-only query of the element at a screen point.
            unsafe { self.uia.ElementFromPoint(POINT { x, y }).ok() }
        }

        /// The Code pane under (x, y): its rectangle (left, top, right, bottom) and the
        /// title on its header row. `None` when the point is not in a Code pane, or its
        /// header names no session.
        pub fn pane_at(&self, x: i32, y: i32) -> Option<((i32, i32, i32, i32), String)> {
            let mut el = self.at(x, y)?;
            let mut rect = None;
            for _ in 0..MAX_DEPTH {
                // SAFETY: read-only properties of an element this thread holds.
                let class = unsafe { el.CurrentClassName() }.ok()?.to_string();
                if is_pane_class(&class) {
                    let r = unsafe { el.CurrentBoundingRectangle() }.ok()?;
                    rect = Some((r.left, r.top, r.right, r.bottom));
                    break;
                }
                el = unsafe { self.walker.GetParentElement(&el) }.ok()?;
                if el.as_raw().is_null() {
                    return None;
                }
            }
            let (l, t, r, b) = rect?;
            if r <= l || b <= t {
                return None;
            }
            for (px, py) in header_points(l, t, r, b) {
                let Some(e) = self.at(px, py) else {
                    continue;
                };
                // SAFETY: a read-only property of an element this thread holds.
                let Ok(name) = (unsafe { e.CurrentName() }) else {
                    continue;
                };
                if let Some(title) = title_from_name(&name.to_string()) {
                    return Some(((l, t, r, b), title.to_string()));
                }
            }
            None
        }
    }

    impl Reader {
        /// Diagnostics: the chain of (control type id, class, name) from the element at
        /// (x, y) upwards, truncated. For the ignored live test only.
        #[cfg(test)]
        pub fn chain(&self, x: i32, y: i32, depth: usize) -> Vec<String> {
            let mut out = Vec::new();
            let Some(mut el) = self.at(x, y) else {
                return out;
            };
            for _ in 0..depth {
                // SAFETY: read-only properties of an element this thread holds.
                let (ct, class, name) = unsafe {
                    (
                        el.CurrentControlType().map(|c| c.0).unwrap_or(0),
                        el.CurrentClassName()
                            .map(|b| b.to_string())
                            .unwrap_or_default(),
                        el.CurrentName().map(|b| b.to_string()).unwrap_or_default(),
                    )
                };
                let cut = |s: &str, n: usize| s.chars().take(n).collect::<String>();
                out.push(format!("{ct}<{}>'{}'", cut(&class, 40), cut(&name, 40)));
                match unsafe { self.walker.GetParentElement(&el) } {
                    Ok(p) if !p.as_raw().is_null() => el = p,
                    _ => break,
                }
            }
            out
        }
    }

    /// The top-level window under (x, y) and whether it is one of ReviewGlass's own.
    pub fn root_at(x: i32, y: i32) -> (isize, bool) {
        // SAFETY: plain window queries with a point and a handle they returned.
        unsafe {
            let w = WindowFromPoint(POINT { x, y });
            let root = GetAncestor(w, GA_ROOT);
            let mut pid = 0u32;
            GetWindowThreadProcessId(root, Some(&mut pid));
            (root.0 as isize, pid == std::process::id())
        }
    }
}

/// Start the loop. Called once from setup.
pub fn spawn(app: AppHandle) {
    thread::Builder::new()
        .name("reviewglass-follow-session".into())
        .spawn(move || run(app))
        .expect("follow-session thread");
}

#[cfg(not(windows))]
fn run(_app: AppHandle) {}

#[cfg(windows)]
fn run(app: AppHandle) {
    let mut reader: Option<uia::Reader> = None;
    let mut known: Option<Known> = None;
    let mut missed: Option<Miss> = None;
    let mut told: Option<FollowSaw> = None;
    loop {
        thread::sleep(POLL);
        let on = app.state::<Store>().get().glass.follow_session;
        let engine = app.state::<Engine>();
        let active =
            on && engine.is_enabled() && engine.mode() == Mode::Follow && !engine.is_held();
        let state = app.state::<FollowSessionState>();
        {
            let mut s = state.inner.lock();
            s.on = on;
            s.active = active;
        }
        if !active {
            // Follow stopped: the next start chooses afresh, from the pane under it.
            known = None;
            missed = None;
            told = None;
            continue;
        }
        let (x, y) = cursor_pos();
        let (root, own) = uia::root_at(x, y);
        if own {
            // Over the glass or the dock: Follow holds, and so does the choice.
            continue;
        }
        let now = Instant::now();
        if known.as_ref().is_some_and(|k| k.holds((x, y), root, now))
            || missed.is_some_and(|m| m.holds((x, y), root, now))
        {
            continue;
        }
        if reader.is_none() {
            match uia::Reader::new() {
                Ok(r) => reader = Some(r),
                Err(e) => {
                    state.inner.lock().unavailable = Some(e);
                    thread::sleep(Duration::from_secs(10));
                    continue;
                }
            }
        }
        let Some(r) = reader.as_ref() else { continue };
        let t0 = Instant::now();
        let found = r.pane_at(x, y);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        {
            let mut s = state.inner.lock();
            s.reads += 1;
            s.last_read_ms = Some(ms);
        }
        crate::measure::log(|| format!("follow-session read {ms:.1} ms {}", found.is_some() as u8));
        let Some((rect, title)) = found else {
            // Not a Code pane, or a header with no session (or Chromium's accessibility
            // tree still waking: the first reads after it is asked for find nothing): the
            // choice stays as it is.
            known = None;
            missed = Some(Miss {
                at: (x, y),
                root,
                when: now,
            });
            continue;
        };
        missed = None;
        known = Some(Known {
            rect,
            root,
            at: now,
        });
        let sessions = app.state::<PanelState>().session_names();
        let matched = match_title(&title, &sessions);
        let saw = FollowSaw {
            title,
            session_id: matched.as_ref().map(|(id, _)| id.clone()),
            name: matched.map(|(_, n)| n),
        };
        if told.as_ref() != Some(&saw) {
            state.inner.lock().saw = Some(saw.clone());
            let _ = app.emit_to(crate::dock::DOCK_LABEL, EVENT, saw.clone());
            told = Some(saw);
        }
    }
}

// ---- commands -------------------------------------------------------------

/// The switch, and what Follow last saw: for the Sessions tab on mount.
#[tauri::command]
pub fn follow_session_state(state: State<FollowSessionState>, store: State<Store>) -> Status {
    let mut s = state.inner.lock().clone();
    s.on = store.get().glass.follow_session;
    s
}

/// Turn "Follow chooses the session" on or off.
#[tauri::command]
pub fn follow_session_set(
    store: State<Store>,
    state: State<FollowSessionState>,
    on: bool,
) -> Status {
    let _ = store.update(|c| c.glass.follow_session = on);
    let mut s = state.inner.lock();
    s.on = on;
    if !on {
        s.saw = None;
    }
    s.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_is_what_precedes_the_rename_suffix() {
        assert_eq!(
            title_from_name("Projektin tilan tarkistus, rename session"),
            Some("Projektin tilan tarkistus")
        );
        // A title with commas of its own keeps them.
        assert_eq!(
            title_from_name("Fix a, b and c, rename session"),
            Some("Fix a, b and c")
        );
        assert_eq!(title_from_name("reviewglass, atlas"), None);
        assert_eq!(title_from_name(", rename session"), None);
        assert_eq!(title_from_name("Rename session"), None);
    }

    #[test]
    fn the_pane_class_is_a_whole_token() {
        assert!(is_pane_class(
            "dframe-pane dframe-pane-primary min-w-0 relative flex flex-col"
        ));
        assert!(is_pane_class("dframe-pane-extra dframe-pane"));
        assert!(!is_pane_class("dframe-pane-extra min-w-0"));
        assert!(!is_pane_class("dframe-content"));
        assert!(!is_pane_class(""));
    }

    #[test]
    fn header_points_stay_on_the_header_row_inside_the_pane() {
        // The pane measured on 20.9.2026.
        let pts = header_points(1409, 128, 1957, 1363);
        assert_eq!(pts[0], (1409 + 164, 152));
        assert_eq!(pts.len(), 20);
        assert!(pts
            .iter()
            .all(|&(x, y)| (1409..1957).contains(&x) && (144..=168).contains(&y)));
        // A pane shorter than the header row asks nothing below its bottom.
        assert!(header_points(0, 0, 100, 20).iter().all(|&(_, y)| y < 20));
    }

    #[test]
    fn titles_match_exactly_then_by_case_and_unnamed_sessions_never() {
        let sessions = vec![
            (
                "a".to_string(),
                Some("Projektin tilan tarkistus".to_string()),
            ),
            ("b".to_string(), None),
            ("c".to_string(), Some("Suno v6 korjaukset".to_string())),
        ];
        assert_eq!(
            match_title("Projektin tilan tarkistus", &sessions),
            Some(("a".into(), "Projektin tilan tarkistus".into()))
        );
        assert_eq!(
            match_title("suno V6 korjaukset", &sessions),
            Some(("c".into(), "Suno v6 korjaukset".into()))
        );
        assert_eq!(match_title("Something else", &sessions), None);
        assert_eq!(match_title("", &sessions), None);
    }

    /// Diagnostic against the real desktop app, not part of the suite: reads the Code
    /// pane at points across the screen (or at `RG_PANE_AT="x,y;x,y"`) and prints the
    /// titles found and how long each read took. Read-only; the cursor is not moved.
    ///
    ///   cargo test live_pane_titles -- --ignored --nocapture
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn live_pane_titles() {
        let r = uia::Reader::new().expect("UI Automation");
        let points: Vec<(i32, i32)> = match std::env::var("RG_PANE_AT") {
            Ok(s) => s
                .split(';')
                .filter_map(|p| {
                    let (x, y) = p.split_once(',')?;
                    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
                })
                .collect(),
            Err(_) => (0..9)
                .map(|k| (200 + k * 280, 700))
                .chain(std::iter::once(cursor_pos()))
                .collect(),
        };
        for (x, y) in points {
            let t0 = Instant::now();
            let found = r.pane_at(x, y);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let (root, own) = uia::root_at(x, y);
            println!("({x},{y}) {ms:7.1} ms root={root:#x} own={own} -> {found:?}");
            if found.is_none() && std::env::var("RG_PANE_CHAIN").is_ok() {
                for c in r.chain(x, y, 14) {
                    println!("      {c}");
                }
            }
        }
    }

    #[test]
    fn a_miss_holds_near_its_point_for_a_while_only() {
        let now = Instant::now();
        let m = Miss {
            at: (500, 500),
            root: 3,
            when: now,
        };
        assert!(m.holds((520, 480), 3, now));
        assert!(!m.holds((560, 500), 3, now), "moved away");
        assert!(!m.holds((500, 500), 4, now), "another window");
        assert!(!m.holds((500, 500), 3, now + MISS_HOLD), "hold over");
    }

    #[test]
    fn a_known_pane_holds_until_the_cursor_leaves_it_or_the_recheck_is_due() {
        let now = Instant::now();
        let k = Known {
            rect: (100, 100, 500, 900),
            root: 7,
            at: now,
        };
        assert!(k.holds((300, 400), 7, now));
        assert!(!k.holds((600, 400), 7, now), "left the pane");
        assert!(!k.holds((300, 400), 8, now), "another window on top");
        assert!(!k.holds((300, 400), 7, now + RECHECK), "re-check due");
    }
}
