//! The diff window (adr.rg.028): the Diff tab's view of one session in a frameless
//! window of its own, opened and closed from the glass's bar or Ctrl+Alt+D.
//!
//! It shows the session of the pane locked by a click (adr.rg.026) and follows the lock;
//! with nothing locked, the drawer's choice. The window is created when opened and
//! destroyed when closed, so it costs a webview only while it is there. It opens the
//! first time over the locked pane's neighbouring pane at that pane's size (the frame
//! loop keeps the neighbour, read as the pane's sibling in the UI Automation tree),
//! never smaller than a diff reads; with no neighbour, in the middle of the screen; and
//! once the user has moved or resized it, where they left it. Only the user's own moves
//! are stored: the page reports a geometry after a drag or a resize it started.

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::config::Store;

pub const LABEL: &str = "diffwin";
/// Sent to every window when the diff window opens or closes: the glass lights its
/// button.
pub const OPEN_EVENT: &str = "diffwin:open";
/// The smallest diff window: a file list and a hunk still read side by side.
const MIN_W: u32 = 560;
const MIN_H: u32 = 400;
/// With no neighbour and nothing remembered.
const DEFAULT_W: u32 = 900;
const DEFAULT_H: u32 = 700;

/// Where the user last put the diff window, in physical pixels; none until they moved
/// or resized it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiffWindowConfig {
    pub place: Option<Place>,
}

/// Where the window opens: the user's place; else over the neighbour at its size (never
/// under the minimum); else centred in `screen` (x, y, width, height) at the default size.
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

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).is_some()
}

/// Open the diff window, or close it when it is open. Creating a window waits on the
/// main thread, so this runs on the async runtime (a command or a spawned task).
pub async fn toggle(app: AppHandle) -> Result<bool, String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        w.destroy().map_err(|e| e.to_string())?;
        let _ = app.emit(OPEN_EVENT, false);
        return Ok(false);
    }
    let remembered = app.state::<Store>().get().diff_window.place;
    let neighbour = crate::frame::neighbour(&app);
    let screen = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let a = m.work_area();
            (a.position.x, a.position.y, a.size.width, a.size.height)
        })
        .unwrap_or((0, 0, 1920, 1080));
    let place = opening_place(remembered, neighbour, screen);
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
    let _ = w.show();
    let _ = w.set_focus();
    crate::measure::log(|| format!("diffwin open {place:?}"));
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

/// The diff window after the user moved or resized it: remembered as their place.
#[tauri::command]
pub fn diffwin_save(store: State<Store>, x: i32, y: i32, width: u32, height: u32) {
    let _ = store.update(|c| {
        c.diff_window.place = Some(Place {
            x,
            y,
            width,
            height,
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (i32, i32, u32, u32) = (0, 0, 2560, 1392);

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
}
