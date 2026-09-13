//! System tray icon: the one place the app is always visible from.
//!
//! Both windows can be hidden — the panel hides on close, the glass hides on Esc or the
//! hotkey — and without a tray that leaves a process running with no trace of itself,
//! which is how a user ends up "closing" the app and starting a second copy. The tray
//! is the fixed point: show either window, or quit for real.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::capture::Engine;
use crate::config::Store;
use crate::glass::GLASS_LABEL;
use crate::panel::PANEL_LABEL;

const ID_GLASS: &str = "show-glass";
const ID_PANEL: &str = "show-panel";
const ID_LENS: &str = "toggle-lens";
const ID_QUIT: &str = "quit";

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let show_glass =
        MenuItem::with_id(app, ID_GLASS, "Show glass\tCtrl+Alt+G", true, None::<&str>)?;
    let show_panel = MenuItem::with_id(app, ID_PANEL, "Show sessions panel", true, None::<&str>)?;
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
            ID_PANEL => show_panel_window(app),
            ID_LENS => {
                let engine = app.state::<Engine>();
                let on = engine.mode() != crate::capture::Mode::Lens;
                show_glass_window(app);
                crate::glass::set_lens_inner(app, &engine, on);
            }
            ID_QUIT => quit_app(app),
            _ => {}
        })
        // A left click brings the panel up: it is the window with something to read.
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_panel_window(tray.app_handle());
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

pub fn show_panel_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(PANEL_LABEL) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn quit_app(app: &AppHandle) {
    app.state::<Engine>().stop();
    app.exit(0);
}
