//! System tray icon: the one place the app is always visible from.
//!
//! The glass hides on Esc or the hotkey and the dock's drawer closes into the strip,
//! and without a tray a hidden app leaves a process running with no trace of itself,
//! which is how a user ends up "closing" the app and starting a second copy. The tray
//! is the fixed point: the glass, the drawer, or quit for real.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::capture::Engine;
use crate::config::Store;
use crate::glass::GLASS_LABEL;

const ID_GLASS: &str = "show-glass";
const ID_PANEL: &str = "show-panel";
const ID_LENS: &str = "toggle-lens";
const ID_QUIT: &str = "quit";

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let show_glass =
        MenuItem::with_id(app, ID_GLASS, "Show glass\tCtrl+Alt+G", true, None::<&str>)?;
    let show_panel = MenuItem::with_id(
        app,
        ID_PANEL,
        "Sessions, diff and settings",
        true,
        None::<&str>,
    )?;
    let lens = MenuItem::with_id(
        app,
        ID_LENS,
        "Lens on / off\tCtrl+Alt+L",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit ReviewGlass", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_glass,
            &show_panel,
            &lens,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("reviewglass")
        .icon(app.default_window_icon().cloned().expect("bundled icon"))
        .tooltip("ReviewGlass")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            ID_GLASS => show_glass_window(app),
            ID_PANEL => crate::dock::open_drawer(app),
            ID_LENS => {
                let engine = app.state::<Engine>();
                let on = engine.mode() != crate::capture::Mode::Lens;
                show_glass_window(app);
                crate::glass::set_lens_inner(app, &engine, on);
            }
            ID_QUIT => quit_app(app),
            _ => {}
        })
        // A left click opens the drawer: it is the part with something to read.
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::dock::open_drawer(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn show_glass_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        let _ = w.show();
    }
    let engine = app.state::<Engine>();
    engine.set_enabled(true);
    let store = app.state::<Store>();
    let _ = store.update(|c| c.glass.visible = true);
    crate::glass::broadcast_state(app, &engine, &store);
}

/// A second launch, or the shortcut clicked while the app runs: bring the dock
/// forward. Nothing else appears until the dock is asked (adr.rg.018).
pub fn show_dock_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(crate::dock::DOCK_LABEL) {
        let _ = w.show();
    }
}

pub fn quit_app(app: &AppHandle) {
    app.state::<Engine>().stop();
    app.exit(0);
}
