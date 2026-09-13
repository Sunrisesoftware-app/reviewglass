//! The dock (rg.dock-window, adr.rg.018): the control panel and the fixed point. A
//! strip snapped to a screen corner, present at every start while the glass starts
//! hidden, from which the glass is switched on in a mode and off again. The Rust side
//! owns the corner, the snapping and the activation; the strip itself is the page.

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager, PhysicalPosition, State};

use crate::capture::{Engine, Mode};
use crate::config::Store;
use crate::glass::{self, GLASS_LABEL};

pub const DOCK_LABEL: &str = "dock";
/// Gap between the dock and the screen's edges, physical pixels.
const DOCK_MARGIN: i32 = 8;

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
    /// The panel's size when it opens under the dock. Stored when the user resizes
    /// it; the position is always the dock's, never stored.
    pub panel_width: u32,
    pub panel_height: u32,
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            corner: Corner::TopLeft,
            panel_width: 500,
            panel_height: 620,
        }
    }
}

/// Gap between the dock and the panel that opens beside it, physical pixels.
const PANEL_GAP: i32 = 6;

/// Put the panel next to the dock, on the side away from the screen's edge — under a
/// dock at the top, above one at the bottom — flush with the dock's outer edge and at
/// its remembered size. The panel is a drawer of the dock, not a window that lands
/// wherever Windows puts it.
pub fn place_panel(app: &AppHandle) {
    let (Some(dock), Some(panel)) = (
        app.get_webview_window(DOCK_LABEL),
        app.get_webview_window(crate::panel::PANEL_LABEL),
    ) else {
        return;
    };
    let cfg = app.state::<Store>().get().dock;
    let _ = panel.set_size(tauri::PhysicalSize::new(cfg.panel_width, cfg.panel_height));
    let (Ok(dpos), Ok(dsize), Ok(psize)) =
        (dock.outer_position(), dock.outer_size(), panel.outer_size())
    else {
        return;
    };
    let x = match cfg.corner {
        Corner::TopLeft | Corner::BottomLeft => dpos.x,
        Corner::TopRight | Corner::BottomRight => dpos.x + dsize.width as i32 - psize.width as i32,
    };
    let y = match cfg.corner {
        Corner::TopLeft | Corner::TopRight => dpos.y + dsize.height as i32 + PANEL_GAP,
        Corner::BottomLeft | Corner::BottomRight => dpos.y - PANEL_GAP - psize.height as i32,
    };
    let _ = panel.set_position(PhysicalPosition::new(x, y));
}

/// The panel's outer size, reported by the panel when the user resizes it.
#[tauri::command]
pub fn panel_save_size(store: State<Store>, width: u32, height: u32) {
    let _ = store.update(|c| {
        c.dock.panel_width = width;
        c.dock.panel_height = height;
    });
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
    let corner = app.state::<Store>().get().dock.corner;
    place(app, corner);
}

/// Move the dock to `corner` of the monitor it is on (the primary one if none).
fn place(app: &AppHandle, corner: Corner) {
    let Some(w) = app.get_webview_window(DOCK_LABEL) else {
        return;
    };
    let monitor = w
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten());
    let (Some(m), Ok(size)) = (monitor, w.outer_size()) else {
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
    corner
}

/// Switch the glass on in a mode ("follow", "lens", "still") or off ("off"). The
/// glass belongs to the dock: this is the one place it is switched on from, and the
/// same button switches it off.
#[tauri::command]
pub fn dock_activate(app: AppHandle, engine: State<Engine>, store: State<Store>, mode: String) {
    match mode.as_str() {
        "off" => {
            glass::hide_glass(&app);
            return;
        }
        "follow" | "lens" | "still" => {}
        _ => return,
    }
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        let _ = w.show();
    }
    engine.set_enabled(true);
    let _ = store.update(|c| c.glass.visible = true);
    match mode.as_str() {
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
pub fn dock_menu(app: AppHandle) -> Result<(), String> {
    let menu = Menu::with_items(
        &app,
        &[
            &MenuItem::with_id(
                &app,
                glass::M_PANEL,
                "Show sessions panel",
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
        use tauri::menu::ContextMenu;
        menu.popup(w.as_ref().window()).map_err(|e| e.to_string())?;
    }
    Ok(())
}
