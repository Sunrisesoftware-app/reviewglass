//! The diff window (adr.rg.028, placement by adr.rg.034): the Diff tab's view of one
//! session in a frameless window of its own, opened and closed from the glass's bar or
//! Ctrl+Alt+D.
//!
//! It shows the session of the pane locked by a click (adr.rg.026) and follows the lock;
//! with nothing locked, the drawer's choice. The window is created when opened and
//! destroyed when closed, so it costs a webview only while it is there.
//!
//! Where it goes (adr.rg.034): attached to one side of the glass, or free. Attached, the
//! shared edge is the glass's (beside it: its top and height; above or below: its left
//! and width) and the other dimension, the thickness, is the user's. It follows every
//! move and resize of the glass, driven by the glass window's own events. One click
//! opens it on the side the user last chose, or before any choice on the first side
//! with room. A drag that drops it beside the glass attaches it to that side; dropped
//! further away it is free, and stays where it was put. Only the user's own moves are
//! stored: the page reports a geometry after a drag or a resize it started.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::capture::{Engine, Mode};
use crate::config::Store;

pub const LABEL: &str = "diffwin";
/// Sent to every window when the diff window opens or closes: the glass lights its
/// button.
pub const OPEN_EVENT: &str = "diffwin:open";
/// Sent to the diff window when the side it is attached to changes (`null`: free).
pub const ATTACHED_EVENT: &str = "diffwin:attached";
/// The smallest free diff window: a file list and a hunk still read side by side.
const MIN_W: u32 = 560;
const MIN_H: u32 = 400;
/// With no neighbour and nothing remembered.
const DEFAULT_W: u32 = 900;
const DEFAULT_H: u32 = 700;
/// Attached: the thinnest a diff still reads beside the glass, and above or below it.
const MIN_BESIDE: u32 = 420;
const MIN_STACKED: u32 = 240;
/// Attached: the shortest shared edge; a glass shorter than this does not shrink the
/// diff with it.
const MIN_LENGTH: u32 = 240;
/// The thickness before the user has set one.
const DEFAULT_BESIDE: u32 = 640;
const DEFAULT_STACKED: u32 = 360;
/// Between the glass and the attached window, so the two read as two.
const GAP: i32 = 6;
/// A drop this close to a side of the glass (or over it) attaches the window.
const SNAP: i32 = 64;

/// Where the user last put the diff window, in physical pixels; none until they moved
/// or resized it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The side of the glass the diff window is attached to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
    Above,
    Below,
}

impl Side {
    fn beside(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }
    fn code(self) -> u8 {
        match self {
            Side::Left => 1,
            Side::Right => 2,
            Side::Above => 3,
            Side::Below => 4,
        }
    }
    fn from_code(c: u8) -> Option<Side> {
        match c {
            1 => Some(Side::Left),
            2 => Some(Side::Right),
            3 => Some(Side::Above),
            4 => Some(Side::Below),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiffWindowConfig {
    /// The free window's place: where the user left it when they last freed it.
    pub place: Option<Place>,
    /// The side the user chose, by a button or a drop; none: the first side with room.
    pub side: Option<Side>,
    /// The user dropped it away from the glass: it opens free, at `place`.
    pub free: bool,
    /// The thickness beside the glass (width) and above or below it (height), as the
    /// user resized it; none: the default.
    pub beside: Option<u32>,
    pub stacked: Option<u32>,
}

/// A rectangle by its edges, physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Edges {
    l: i32,
    t: i32,
    r: i32,
    b: i32,
}

impl Edges {
    fn of(p: Place) -> Edges {
        Edges {
            l: p.x,
            t: p.y,
            r: p.x + p.width as i32,
            b: p.y + p.height as i32,
        }
    }
    fn w(self) -> i32 {
        self.r - self.l
    }
    fn h(self) -> i32 {
        self.b - self.t
    }
}

/// The glass as the attached window sees it: its body (without the tab's strip, which
/// beside the glass would make the diff overhang it) and the bottom of the whole window
/// (the tab included, which a window below must clear).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GlassBox {
    body: Edges,
    foot: i32,
}

/// The side the window is attached to while open; 0 = free. The window's own events
/// read it on the main thread; the commands set it.
static ATTACHED: AtomicU8 = AtomicU8::new(0);

fn attached() -> Option<Side> {
    Side::from_code(ATTACHED.load(Ordering::Relaxed))
}

fn set_attached(app: &AppHandle, side: Option<Side>) {
    ATTACHED.store(side.map_or(0, Side::code), Ordering::Relaxed);
    let _ = app.emit_to(LABEL, ATTACHED_EVENT, side);
}

/// Room for the diff window on `side` of the glass, inside the work area.
fn room(g: GlassBox, work: Edges, side: Side) -> i32 {
    match side {
        Side::Right => work.r - g.body.r - GAP,
        Side::Left => g.body.l - work.l - GAP,
        Side::Below => work.b - g.foot - GAP,
        Side::Above => g.body.t - work.t - GAP,
    }
}

fn min_thickness(side: Side) -> u32 {
    if side.beside() {
        MIN_BESIDE
    } else {
        MIN_STACKED
    }
}

/// Before the user has chosen: the first of right, left, below and above where a diff
/// reads; with none, the side with the most room.
fn auto_side(g: GlassBox, work: Edges) -> Side {
    const ORDER: [Side; 4] = [Side::Right, Side::Left, Side::Below, Side::Above];
    ORDER
        .into_iter()
        .find(|&s| room(g, work, s) >= min_thickness(s) as i32)
        .unwrap_or_else(|| {
            ORDER
                .into_iter()
                .max_by_key(|&s| room(g, work, s))
                .unwrap_or(Side::Right)
        })
}

/// The attached window's place on `side`: the glass's edge shared, `thickness` the
/// other way, narrowed to the room there (never under the minimum), then moved onto the
/// work area — over the glass if it must — never cut.
fn attached_place(g: GlassBox, work: Edges, side: Side, thickness: u32) -> Place {
    let min = min_thickness(side);
    let room = room(g, work, side).max(0) as u32;
    let limit = if side.beside() { work.w() } else { work.h() }.max(min as i32) as u32;
    let thick = thickness.min(room).max(min).min(limit);
    let (x, y, width, height) = if side.beside() {
        let length = (g.body.h().max(0) as u32).max(MIN_LENGTH);
        let x = match side {
            Side::Right => g.body.r + GAP,
            _ => g.body.l - GAP - thick as i32,
        };
        (x, g.body.t, thick, length)
    } else {
        let length = (g.body.w().max(0) as u32).max(MIN_LENGTH);
        let y = match side {
            Side::Below => g.foot + GAP,
            _ => g.body.t - GAP - thick as i32,
        };
        (g.body.l, y, length, thick)
    };
    let width = width.min(work.w().max(1) as u32);
    let height = height.min(work.h().max(1) as u32);
    Place {
        x: x.clamp(work.l, (work.r - width as i32).max(work.l)),
        y: y.clamp(work.t, (work.b - height as i32).max(work.t)),
        width,
        height,
    }
}

/// Which side a window dropped at `d` attaches to: the side its centre lies towards,
/// if its near edge is within the snap distance of the glass's (or over it); none when
/// it was dropped away from the glass.
fn side_of_drop(g: GlassBox, d: Edges) -> Option<Side> {
    let (gcx, gcy) = ((g.body.l + g.body.r) / 2, (g.body.t + g.foot) / 2);
    let (dcx, dcy) = ((d.l + d.r) / 2, (d.t + d.b) / 2);
    // Normalised by the two half-sizes, so a wide glass does not favour above/below.
    let nx = (dcx - gcx) as f64 / ((g.body.w() + d.w()).max(2) as f64 / 2.0);
    let ny = (dcy - gcy) as f64 / ((g.foot - g.body.t + d.h()).max(2) as f64 / 2.0);
    let (side, gap) = if nx.abs() >= ny.abs() {
        if nx >= 0.0 {
            (Side::Right, d.l - g.body.r)
        } else {
            (Side::Left, g.body.l - d.r)
        }
    } else if ny >= 0.0 {
        (Side::Below, d.t - g.foot)
    } else {
        (Side::Above, g.body.t - d.b)
    };
    // Apart along the other axis too: a window far above-right is not beside the glass.
    let apart = if side.beside() {
        d.b < g.body.t - SNAP || d.t > g.foot + SNAP
    } else {
        d.r < g.body.l - SNAP || d.l > g.body.r + SNAP
    };
    (gap <= SNAP && !apart).then_some(side)
}

/// Where a free window opens: the user's place; else over the neighbour at its size
/// (never under the minimum); else centred in `screen` (x, y, width, height) at the
/// default size.
fn opening_place(
    remembered: Option<Place>,
    neighbour: Option<(i32, i32, i32, i32)>,
    screen: (i32, i32, u32, u32),
) -> Place {
    if let Some(p) = remembered {
        return Place {
            width: p.width.max(MIN_W),
            height: p.height.max(MIN_H),
            ..p
        };
    }
    if let Some((l, t, r, b)) = neighbour {
        return Place {
            x: l,
            y: t,
            width: ((r - l).max(0) as u32).max(MIN_W),
            height: ((b - t).max(0) as u32).max(MIN_H),
        };
    }
    let (sx, sy, sw, sh) = screen;
    let (w, h) = (DEFAULT_W.min(sw), DEFAULT_H.min(sh));
    Place {
        x: sx + (sw.saturating_sub(w) / 2) as i32,
        y: sy + (sh.saturating_sub(h) / 2) as i32,
        width: w,
        height: h,
    }
}

/// The glass's box and its monitor's work area, when the diff window can attach to it:
/// the glass is shown and not the Lens (which rides the cursor).
fn glass_box(app: &AppHandle) -> Option<(GlassBox, Edges)> {
    if app.state::<Engine>().mode() == Mode::Lens {
        return None;
    }
    let g = app.get_webview_window(crate::glass::GLASS_LABEL)?;
    if !g.is_visible().unwrap_or(false) {
        return None;
    }
    let p = g.outer_position().ok()?;
    let s = g.outer_size().ok()?;
    let foot = p.y + s.height as i32;
    let tab = crate::glass::tab_height() as i32;
    let body_b = if tab > 0 && tab < s.height as i32 {
        foot - tab
    } else {
        foot
    };
    let m = g.current_monitor().ok().flatten()?;
    let a = m.work_area();
    let work = Edges {
        l: a.position.x,
        t: a.position.y,
        r: a.position.x + a.size.width as i32,
        b: a.position.y + a.size.height as i32,
    };
    Some((
        GlassBox {
            body: Edges {
                l: p.x,
                t: p.y,
                r: p.x + s.width as i32,
                b: body_b,
            },
            foot,
        },
        work,
    ))
}

fn thickness(app: &AppHandle, side: Side) -> u32 {
    let cfg = app.state::<Store>().get().diff_window;
    if side.beside() {
        cfg.beside.unwrap_or(DEFAULT_BESIDE)
    } else {
        cfg.stacked.unwrap_or(DEFAULT_STACKED)
    }
}

fn put(w: &WebviewWindow, p: Place) {
    let pos = PhysicalPosition::new(p.x, p.y);
    let size = PhysicalSize::new(p.width, p.height);
    if w.outer_size().ok() != Some(size) {
        let _ = w.set_size(size);
    }
    if w.outer_position().ok() != Some(pos) {
        let _ = w.set_position(pos);
    }
}

/// Place the open, attached diff window against the glass. Called on every move and
/// resize of the glass (on the main thread, from its window events) and after a choice.
pub fn follow_glass(app: &AppHandle) {
    let Some(side) = attached() else {
        return;
    };
    let Some(w) = app.get_webview_window(LABEL) else {
        return;
    };
    let Some((g, work)) = glass_box(app) else {
        return;
    };
    put(&w, attached_place(g, work, side, thickness(app, side)));
}

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).is_some()
}

/// Open the diff window, or close it when it is open. Creating a window waits on the
/// main thread, so this runs on the async runtime (a command or a spawned task).
pub async fn toggle(app: AppHandle) -> Result<bool, String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        w.destroy().map_err(|e| e.to_string())?;
        ATTACHED.store(0, Ordering::Relaxed);
        let _ = app.emit(OPEN_EVENT, false);
        return Ok(false);
    }
    let cfg = app.state::<Store>().get().diff_window;
    // Attached unless the user freed it, or there is no glass to attach to.
    let attach = if cfg.free {
        None
    } else {
        glass_box(&app).map(|(g, work)| {
            let side = cfg.side.unwrap_or_else(|| auto_side(g, work));
            (side, attached_place(g, work, side, thickness(&app, side)))
        })
    };
    let place = match attach {
        Some((_, p)) => p,
        None => {
            let screen = app
                .primary_monitor()
                .ok()
                .flatten()
                .map(|m| {
                    let a = m.work_area();
                    (a.position.x, a.position.y, a.size.width, a.size.height)
                })
                .unwrap_or((0, 0, 1920, 1080));
            opening_place(cfg.place, crate::frame::neighbour(&app), screen)
        }
    };
    let w = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("diffwin".into()))
        .title("ReviewGlass Diff")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        .shadow(true)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    let _ = w.set_size(PhysicalSize::new(place.width, place.height));
    let _ = w.set_position(PhysicalPosition::new(place.x, place.y));
    ATTACHED.store(attach.map_or(0, |(s, _)| s.code()), Ordering::Relaxed);
    let _ = w.show();
    let _ = w.set_focus();
    crate::measure::log(|| format!("diffwin open {place:?} attached={:?}", attached()));
    let _ = app.emit(OPEN_EVENT, true);
    Ok(true)
}

/// Toggle from a place that is not async (a hotkey).
pub fn toggle_later(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = toggle(app).await {
            eprintln!("reviewglass: diff window: {e}");
        }
    });
}

// ---- commands -------------------------------------------------------------

/// The glass's button: open or close; the answer is whether it is open now.
#[tauri::command]
pub async fn diffwin_toggle(app: AppHandle) -> Result<bool, String> {
    toggle(app).await
}

#[tauri::command]
pub fn diffwin_is_open(app: AppHandle) -> bool {
    is_open(&app)
}

/// The side the window is attached to now (none: free), for the page's side buttons.
#[tauri::command]
pub fn diffwin_attached() -> Option<Side> {
    attached()
}

/// A side button: attach to `side` at once. Async: it moves a window and asks the glass
/// for its place, which waits on the main thread.
#[tauri::command]
pub async fn diffwin_attach(app: AppHandle, side: Side) {
    if glass_box(&app).is_none() {
        return; // no glass to attach to: the page says so
    }
    let _ = app.state::<Store>().update(|c| {
        c.diff_window.side = Some(side);
        c.diff_window.free = false;
    });
    set_attached(&app, Some(side));
    follow_glass(&app);
}

/// The diff window after the user moved or resized it: beside the glass it attaches
/// to that side (a resize of its free edge sets the thickness); away from the glass it
/// is free and its place is remembered.
#[tauri::command]
pub async fn diffwin_dropped(app: AppHandle, x: i32, y: i32, width: u32, height: u32) {
    let dropped = Place {
        x,
        y,
        width,
        height,
    };
    let side = glass_box(&app).and_then(|(g, _)| side_of_drop(g, Edges::of(dropped)));
    let store = app.state::<Store>();
    match side {
        Some(s) => {
            // Resized in place, the free edge is the user's thickness; a drop from
            // elsewhere keeps the thickness this side already had.
            let was = attached() == Some(s);
            let _ = store.update(|c| {
                let d = &mut c.diff_window;
                d.side = Some(s);
                d.free = false;
                if was {
                    if s.beside() {
                        d.beside = Some(width.max(MIN_BESIDE));
                    } else {
                        d.stacked = Some(height.max(MIN_STACKED));
                    }
                }
            });
            set_attached(&app, Some(s));
            follow_glass(&app);
        }
        None => {
            let _ = store.update(|c| {
                c.diff_window.free = true;
                c.diff_window.place = Some(dropped);
            });
            set_attached(&app, None);
        }
    }
    crate::measure::log(|| format!("diffwin dropped {dropped:?} -> {side:?}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (i32, i32, u32, u32) = (0, 0, 2560, 1392);
    const WORK: Edges = Edges {
        l: 0,
        t: 0,
        r: 2560,
        b: 1392,
    };

    fn glass(l: i32, t: i32, r: i32, b: i32) -> GlassBox {
        GlassBox {
            body: Edges { l, t, r, b },
            foot: b + 30, // the tab's strip below the body
        }
    }

    #[test]
    fn the_first_opening_lies_over_the_neighbour_at_its_size() {
        let p = opening_place(None, Some((1112, 175, 1817, 1305)), SCREEN);
        assert_eq!(
            p,
            Place {
                x: 1112,
                y: 175,
                width: 705,
                height: 1130
            }
        );
        // A narrow neighbour is widened to where a diff reads.
        let p = opening_place(None, Some((0, 0, 300, 200)), SCREEN);
        assert_eq!((p.width, p.height), (MIN_W, MIN_H));
    }

    #[test]
    fn the_users_place_wins_and_no_neighbour_means_the_middle() {
        let mine = Place {
            x: 40,
            y: 50,
            width: 800,
            height: 600,
        };
        assert_eq!(
            opening_place(Some(mine), Some((1112, 175, 1817, 1305)), SCREEN),
            mine
        );
        let p = opening_place(None, None, SCREEN);
        assert_eq!(
            p,
            Place {
                x: (2560 - 900) / 2,
                y: (1392 - 700) / 2,
                width: 900,
                height: 700
            }
        );
    }

    #[test]
    fn the_first_side_with_room_is_right_then_left_then_below_then_above() {
        // A glass in the left half: room on the right.
        assert_eq!(auto_side(glass(100, 100, 1000, 1000), WORK), Side::Right);
        // Against the right edge: the left.
        assert_eq!(auto_side(glass(1700, 100, 2560, 1000), WORK), Side::Left);
        // The whole width, high up: below.
        assert_eq!(auto_side(glass(0, 0, 2560, 700), WORK), Side::Below);
        // The whole width, low down: above.
        assert_eq!(auto_side(glass(0, 700, 2560, 1360), WORK), Side::Above);
        // Nowhere a diff reads (240 px needed above or below; this glass leaves 200
        // above and 156 below): the side with the most room.
        assert_eq!(auto_side(glass(0, 206, 2560, 1200), WORK), Side::Above);
    }

    #[test]
    fn attached_beside_it_shares_the_glass_top_and_height() {
        let g = glass(100, 120, 1000, 1020);
        let p = attached_place(g, WORK, Side::Right, 640);
        assert_eq!(
            p,
            Place {
                x: 1000 + GAP,
                y: 120,
                width: 640,
                height: 900
            }
        );
        let p = attached_place(glass(1400, 120, 2300, 1020), WORK, Side::Left, 640);
        assert_eq!(
            (p.x, p.y, p.width, p.height),
            (1400 - GAP - 640, 120, 640, 900)
        );
    }

    #[test]
    fn attached_below_it_clears_the_tab_and_shares_the_width() {
        let g = glass(300, 100, 1500, 700);
        let p = attached_place(g, WORK, Side::Below, 360);
        assert_eq!(
            p,
            Place {
                x: 300,
                y: 730 + GAP,
                width: 1200,
                height: 360
            }
        );
        let p = attached_place(glass(300, 600, 1500, 1200), WORK, Side::Above, 360);
        assert_eq!(
            (p.x, p.y, p.width, p.height),
            (300, 600 - GAP - 360, 1200, 360)
        );
    }

    #[test]
    fn short_of_room_it_narrows_to_the_minimum_then_moves_onto_the_screen() {
        // 600 px of room on the right: the 640 asked narrows to the room.
        let p = attached_place(glass(100, 0, 1954, 900), WORK, Side::Right, 640);
        assert_eq!(p.width, 600);
        assert_eq!(p.x, 1954 + GAP);
        // 100 px of room: the minimum, moved left over the glass to stay on screen.
        let p = attached_place(glass(100, 0, 2454, 900), WORK, Side::Right, 640);
        assert_eq!(p.width, MIN_BESIDE);
        assert_eq!(p.x, 2560 - MIN_BESIDE as i32);
        // A short glass does not make a short diff.
        let p = attached_place(glass(100, 100, 900, 200), WORK, Side::Right, 640);
        assert_eq!(p.height, MIN_LENGTH);
    }

    #[test]
    fn a_drop_beside_the_glass_attaches_and_a_drop_away_frees() {
        let g = glass(500, 200, 1500, 1000);
        // Just right of it, a little apart: right.
        let d = Edges {
            l: 1540,
            t: 300,
            r: 2100,
            b: 900,
        };
        assert_eq!(side_of_drop(g, d), Some(Side::Right));
        // Overlapping its left edge: left.
        let d = Edges {
            l: 100,
            t: 250,
            r: 560,
            b: 900,
        };
        assert_eq!(side_of_drop(g, d), Some(Side::Left));
        // Under it: below (measured from the tab's foot).
        let d = Edges {
            l: 600,
            t: 1060,
            r: 1400,
            b: 1380,
        };
        assert_eq!(side_of_drop(g, d), Some(Side::Below));
        // Far to the right: free.
        let d = Edges {
            l: 1800,
            t: 300,
            r: 2400,
            b: 900,
        };
        assert_eq!(side_of_drop(g, d), None);
        // Right of it but entirely above its top: free.
        let d = Edges {
            l: 1510,
            t: 0,
            r: 2100,
            b: 100,
        };
        assert_eq!(side_of_drop(g, d), None);
    }

    #[test]
    fn a_config_from_before_the_sides_opens_attached() {
        let old: DiffWindowConfig =
            serde_json::from_str(r#"{"place":{"x":1,"y":2,"width":800,"height":600}}"#).unwrap();
        assert!(!old.free);
        assert_eq!(old.side, None);
        assert!(old.place.is_some());
    }
}
