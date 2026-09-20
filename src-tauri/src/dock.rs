//! The dock (rg.dock-window, adr.rg.018): the control panel and the fixed point. A
//! strip snapped to a screen corner, present at every start while the glass starts
//! hidden, from which the glass is switched on in a mode and off again. The Rust side
//! owns the corner, the snapping and the activation; the strip itself is the page.
//!
//! The panel is the dock's drawer, in this same window (adr.rg.020): closed, the
//! window is the strip; open, it grows to the drawer's remembered size, below the
//! strip in a top corner and above it in a bottom one, snapped to the same corner.
//! One window that grows, so nothing has to follow anything. The size is the owner's
//! to set, by dragging the drawer's free corner (adr.rg.021).

use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State};

use crate::capture::{Engine, Mode};
use crate::config::Store;
use crate::glass::{self, GLASS_LABEL};

pub const DOCK_LABEL: &str = "dock";
/// Menu id: the glass on or off, as the RG mark and the hotkey do.
pub const M_TOGGLE: &str = "dock-toggle";
/// Gap between the dock and the screen's edges, physical pixels.
const DOCK_MARGIN: i32 = 8;
/// The strip: the dock closed.
pub const STRIP_WIDTH: u32 = 470;
pub const STRIP_HEIGHT: u32 = 44;
/// The drawer's least size: wide enough for a diff's file list and hunk side by side,
/// tall enough for a table. The size it is dragged to is remembered (adr.rg.021).
pub const DRAWER_MIN_WIDTH: u32 = 640;
pub const DRAWER_MIN_HEIGHT: u32 = 300;
/// Event to the dock page when the drawer's state changed from outside it.
pub const STATE_EVENT: &str = "dock:state";

/// The screen corner the dock sits in. Dragging it anywhere snaps it to the nearest
/// corner, so a corner is the only position it can have.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DockConfig {
    pub corner: Corner,
    /// The drawer's size, physical pixels: the open window's width, and its height
    /// below (or above) the strip. Set by dragging the drawer's free corner
    /// (adr.rg.021) and remembered here; never smaller than the minimums.
    pub drawer_width: u32,
    pub drawer_height: u32,
    /// The tab the drawer opens on: the last one used.
    pub drawer_tab: String,
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            corner: Corner::TopLeft,
            drawer_width: 640,
            drawer_height: 620,
            drawer_tab: "sessions".into(),
        }
    }
}

/// The drawer's open state. Not a setting: every start begins with the strip alone.
#[derive(Default)]
pub struct DockState {
    open: AtomicBool,
}

impl DockState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Relaxed)
    }
}

/// What the dock page renders: the drawer's state and the corner it hangs from,
/// which decides whether the strip is the drawer's top row or its bottom one.
#[derive(Clone, Debug, Serialize)]
pub struct DockView {
    pub open: bool,
    pub corner: Corner,
    pub drawer_width: u32,
    pub drawer_height: u32,
    pub drawer_tab: String,
}

fn view_of(app: &AppHandle) -> DockView {
    let cfg = app.state::<Store>().get().dock;
    DockView {
        open: app.state::<DockState>().is_open(),
        corner: cfg.corner,
        drawer_width: cfg.drawer_width,
        drawer_height: cfg.drawer_height,
        drawer_tab: cfg.drawer_tab,
    }
}

/// The window's outer size for a drawer state: the strip, or the remembered drawer
/// size, never under the minimums.
fn size_for(open: bool, cfg: &DockConfig) -> PhysicalSize<u32> {
    if open {
        PhysicalSize::new(
            cfg.drawer_width.max(DRAWER_MIN_WIDTH),
            STRIP_HEIGHT + cfg.drawer_height.max(DRAWER_MIN_HEIGHT),
        )
    } else {
        PhysicalSize::new(STRIP_WIDTH, STRIP_HEIGHT)
    }
}

/// The largest the open window may be: the work area of the monitor it is on, less
/// the margins.
fn max_size(app: &AppHandle) -> Option<PhysicalSize<u32>> {
    let w = app.get_webview_window(DOCK_LABEL)?;
    let m = w
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten())?;
    let area = m.work_area();
    Some(PhysicalSize::new(
        (area.size.width as i32 - 2 * DOCK_MARGIN).max(DRAWER_MIN_WIDTH as i32) as u32,
        (area.size.height as i32 - 2 * DOCK_MARGIN).max((STRIP_HEIGHT + DRAWER_MIN_HEIGHT) as i32)
            as u32,
    ))
}

/// Open or close the drawer: resize the one window and put it back in its corner, so
/// the strip stays where it was and the drawer unfolds away from the screen's edge.
/// Reached from the strip's button, the tray, the glass's menu and its bar. Open, the
/// window can be sized by hand from its free corner (adr.rg.021), between the
/// minimums and the work area; closed, the strip cannot be resized at all.
pub fn set_drawer(app: &AppHandle, open: bool) {
    let Some(w) = app.get_webview_window(DOCK_LABEL) else {
        return;
    };
    let cfg = app.state::<Store>().get().dock;
    app.state::<DockState>().open.store(open, Ordering::Relaxed);
    let mut size = size_for(open, &cfg);
    if open {
        if let Some(max) = max_size(app) {
            size = PhysicalSize::new(size.width.min(max.width), size.height.min(max.height));
        }
    } else {
        // The limits come off before the window shrinks to the strip.
        let _ = w.set_resizable(false);
        let _ = w.set_min_size(None::<PhysicalSize<u32>>);
        let _ = w.set_max_size(None::<PhysicalSize<u32>>);
    }
    // Position first, from the size the window is about to have, so a bottom-corner
    // drawer never spends a frame hanging below the screen; then again after the
    // resize, for the corner the window is actually on.
    place_with(app, cfg.corner, size);
    let _ = w.set_size(size);
    place_with(app, cfg.corner, size);
    if open {
        let _ = w.set_resizable(true);
        let _ = w.set_min_size(Some(PhysicalSize::new(
            DRAWER_MIN_WIDTH,
            STRIP_HEIGHT + DRAWER_MIN_HEIGHT,
        )));
        if let Some(max) = max_size(app) {
            let _ = w.set_max_size(Some(max));
        }
    }
    let _ = w.show();
    if open {
        let _ = w.set_focus();
    }
    let _ = app.emit_to(DOCK_LABEL, STATE_EVENT, view_of(app));
}

/// After a resize by hand (adr.rg.021): remember the size the window ended with and
/// put it back in its corner, so the snapped corner is where it was. Nothing to do
/// while the drawer is closed (the strip's own resize on close arrives here too).
#[tauri::command]
pub fn dock_drawer_resized(app: AppHandle, store: State<Store>) -> DockView {
    if app.state::<DockState>().is_open() {
        if let Some(size) = app
            .get_webview_window(DOCK_LABEL)
            .and_then(|w| w.outer_size().ok())
        {
            let width = size.width.max(DRAWER_MIN_WIDTH);
            let height = size
                .height
                .saturating_sub(STRIP_HEIGHT)
                .max(DRAWER_MIN_HEIGHT);
            let _ = store.update(|c| {
                c.dock.drawer_width = width;
                c.dock.drawer_height = height;
            });
            place(&app, store.get().dock.corner);
        }
    }
    view_of(&app)
}

/// Bring the dock forward with its drawer open. The one entry for every "show the
/// panel" control outside the dock.
pub fn open_drawer(app: &AppHandle) {
    set_drawer(app, true);
}

#[tauri::command]
pub fn dock_state(app: AppHandle) -> DockView {
    view_of(&app)
}

#[tauri::command]
pub fn dock_drawer(app: AppHandle, open: bool) -> DockView {
    set_drawer(&app, open);
    view_of(&app)
}

/// Remember the tab the drawer is on, so it opens there next time.
#[tauri::command]
pub fn dock_set_tab(store: State<Store>, tab: String) {
    let _ = store.update(|c| c.dock.drawer_tab = tab);
}

/// Exclude the dock from capture and put it in its corner. The glass never shows the
/// dock, and the dock is where the user left it.
pub fn prepare(app: &AppHandle) {
    let Some(w) = app.get_webview_window(DOCK_LABEL) else {
        return;
    };
    if let Err(e) = glass::exclude_from_capture(&w) {
        eprintln!("reviewglass: dock {e}");
    }
    let cfg = app.state::<Store>().get().dock;
    // Every start begins with the strip alone, whatever size the window was declared
    // with.
    let _ = w.set_size(size_for(false, &cfg));
    place(app, cfg.corner);
}

/// Move the dock to `corner` of the monitor it is on (the primary one if none), at
/// the size it has now.
fn place(app: &AppHandle, corner: Corner) {
    let Some(w) = app.get_webview_window(DOCK_LABEL) else {
        return;
    };
    if let Ok(size) = w.outer_size() {
        place_with(app, corner, size);
    }
}

/// Move the dock to `corner` as if it were `size`: the drawer's unfold computes the
/// position from the size it is about to have.
fn place_with(app: &AppHandle, corner: Corner, size: PhysicalSize<u32>) {
    let Some(w) = app.get_webview_window(DOCK_LABEL) else {
        return;
    };
    let monitor = w
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten());
    let Some(m) = monitor else {
        return;
    };
    let area = m.work_area();
    let left = area.position.x + DOCK_MARGIN;
    let top = area.position.y + DOCK_MARGIN;
    let right = area.position.x + area.size.width as i32 - size.width as i32 - DOCK_MARGIN;
    let bottom = area.position.y + area.size.height as i32 - size.height as i32 - DOCK_MARGIN;
    let (x, y) = match corner {
        Corner::TopLeft => (left, top),
        Corner::TopRight => (right, top),
        Corner::BottomLeft => (left, bottom),
        Corner::BottomRight => (right, bottom),
    };
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

/// After a drag: snap to the nearest corner of the monitor the dock is on, and
/// remember it.
#[tauri::command]
pub fn dock_snap(app: AppHandle, store: State<Store>) -> Corner {
    let corner = app
        .get_webview_window(DOCK_LABEL)
        .and_then(|w| {
            let m = w.current_monitor().ok().flatten()?;
            let pos = w.outer_position().ok()?;
            let size = w.outer_size().ok()?;
            let area = m.work_area();
            let cx = pos.x + size.width as i32 / 2;
            let cy = pos.y + size.height as i32 / 2;
            let right = cx > area.position.x + area.size.width as i32 / 2;
            let bottom = cy > area.position.y + area.size.height as i32 / 2;
            Some(match (right, bottom) {
                (false, false) => Corner::TopLeft,
                (true, false) => Corner::TopRight,
                (false, true) => Corner::BottomLeft,
                (true, true) => Corner::BottomRight,
            })
        })
        .unwrap_or(store.get().dock.corner);
    let _ = store.update(|c| c.dock.corner = corner);
    place(&app, corner);
    // The corner decides which way the drawer unfolds: the page re-lays itself out.
    let _ = app.emit_to(DOCK_LABEL, STATE_EVENT, view_of(&app));
    corner
}

/// Switch the glass on in a mode ("follow", "lens", "still"), on as it last was
/// ("last" — the RG mark and the hotkey), or off ("off"). The glass belongs to the
/// dock: this is the one place it is switched on from, and the same control switches
/// it off.
#[tauri::command]
pub fn dock_activate(app: AppHandle, engine: State<Engine>, store: State<Store>, mode: String) {
    match mode.as_str() {
        "off" => {
            glass::hide_glass(&app);
            return;
        }
        "follow" | "lens" | "still" | "last" => {}
        _ => return,
    }
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        let _ = w.show();
    }
    engine.set_enabled(true);
    let _ = store.update(|c| c.glass.visible = true);
    match mode.as_str() {
        "last" => {}
        "lens" => glass::set_lens_inner(&app, &engine, true),
        "still" => {
            if engine.mode() == Mode::Lens {
                glass::set_lens_inner(&app, &engine, false);
            }
            glass::set_frozen_inner(&app, &engine, true);
        }
        _ => {
            if engine.mode() == Mode::Lens {
                glass::set_lens_inner(&app, &engine, false);
            } else {
                glass::set_frozen_inner(&app, &engine, false);
            }
        }
    }
    glass::broadcast_state(&app, &engine, &store);
}

/// The dock's right-click menu: the panel, and the real quit.
#[tauri::command]
pub fn dock_menu(app: AppHandle, store: State<Store>) -> Result<(), String> {
    let visible = store.get().glass.visible;
    let hotkey = store.get().hotkeys.toggle_glass;
    let toggle = if visible {
        format!("Glass off\t{hotkey}")
    } else {
        format!("Glass on\t{hotkey}")
    };
    let menu = Menu::with_items(
        &app,
        &[
            &MenuItem::with_id(&app, M_TOGGLE, toggle, true, None::<&str>)
                .map_err(|e| e.to_string())?,
            &MenuItem::with_id(
                &app,
                glass::M_PANEL,
                "Sessions, diff and settings",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?,
            &PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?,
            &MenuItem::with_id(&app, glass::M_QUIT, "Quit ReviewGlass", true, None::<&str>)
                .map_err(|e| e.to_string())?,
        ],
    )
    .map_err(|e| e.to_string())?;
    if let Some(w) = app.get_webview_window(DOCK_LABEL) {
        // The dock's menu holds the glass like the glass's own: a lens riding at the
        // cursor would otherwise ride over the menu.
        glass::popup_held(&app.state::<Engine>(), &menu, &w).map_err(|e| e.to_string())?;
    }
    Ok(())
}
