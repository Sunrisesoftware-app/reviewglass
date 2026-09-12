//! Glass window (rg.glass-window), Rust side: the commands the magnifier frontend
//! calls, the global hotkeys, self-exclusion from capture, and persistence of
//! geometry, zoom and freeze state through the config store.

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};

use crate::capture::{cursor_pos, Engine, Mode};
use crate::config::{LoadOutcome, Store};

pub const GLASS_LABEL: &str = "glass";
/// Event sent to the glass when freeze/zoom changes from outside the webview.
pub const STATE_EVENT: &str = "glass:state";

#[derive(Clone, Serialize)]
pub struct GlassState {
    pub zoom: f32,
    pub frozen: bool,
    pub lens: bool,
    pub config: LoadOutcome,
}

fn state_of(engine: &Engine, store: &Store) -> GlassState {
    let mode = engine.mode();
    GlassState {
        zoom: engine.zoom(),
        frozen: mode == Mode::Frozen,
        lens: mode == Mode::Lens,
        config: store.outcome(),
    }
}

/// Exclude the glass from every screen-capture path so it never captures itself.
/// Without this the magnifier shows itself inside itself, recursively.
pub fn exclude_from_capture(window: &WebviewWindow) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    // SAFETY: hwnd is a live window handle owned by this process.
    unsafe {
        SetWindowDisplayAffinity(HWND(hwnd.0), WDA_EXCLUDEFROMCAPTURE)
            .map_err(|e| format!("SetWindowDisplayAffinity failed: {e}"))
    }
}

/// Restore geometry and mode from the config store at startup.
pub fn restore(app: &AppHandle) {
    let store = app.state::<Store>();
    let engine = app.state::<Engine>();
    let cfg = store.get().glass;
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        let _ = w.set_size(PhysicalSize::new(cfg.width, cfg.height));
        if let (Some(x), Some(y)) = (cfg.x, cfg.y) {
            let _ = w.set_position(PhysicalPosition::new(x, y));
        }
        if !cfg.visible {
            let _ = w.hide();
        }
    }
    engine.set_enabled(cfg.visible);
    engine.set_view(cfg.width, cfg.height, cfg.zoom);
    if cfg.lens {
        set_lens_inner(app, &engine, true);
    } else if cfg.frozen {
        engine.freeze_at(cfg.frozen_x, cfg.frozen_y);
    }
}

pub fn register_hotkeys(app: &AppHandle) -> Result<(), String> {
    let hk = app.state::<Store>().get().hotkeys;
    let toggle_glass: Shortcut = hk.toggle_glass.parse().map_err(|e| format!("{e}"))?;
    let toggle_freeze: Shortcut = hk.toggle_freeze.parse().map_err(|e| format!("{e}"))?;
    let toggle_lens: Shortcut = hk.toggle_lens.parse().map_err(|e| format!("{e}"))?;
    let gs = app.global_shortcut();
    gs.on_shortcut(toggle_glass, |app, _, ev| {
        if ev.state == ShortcutState::Pressed {
            toggle_visible(app);
        }
    })
    .map_err(|e| e.to_string())?;
    gs.on_shortcut(toggle_freeze, |app, _, ev| {
        if ev.state == ShortcutState::Pressed {
            let engine = app.state::<Engine>();
            let frozen = engine.mode() != Mode::Frozen;
            set_frozen_inner(app, &engine, frozen);
        }
    })
    .map_err(|e| e.to_string())?;
    // The lens is click-through, so its own controls cannot switch it off: the hotkey
    // (and the tray) are the way out, and the glass says so while it is on.
    gs.on_shortcut(toggle_lens, |app, _, ev| {
        if ev.state == ShortcutState::Pressed {
            let engine = app.state::<Engine>();
            let lens = engine.mode() != Mode::Lens;
            set_lens_inner(app, &engine, lens);
        }
    })
    .map_err(|e| e.to_string())
}

fn toggle_visible(app: &AppHandle) {
    let Some(w) = app.get_webview_window(GLASS_LABEL) else {
        return;
    };
    let visible = w.is_visible().unwrap_or(true);
    let _ = if visible { w.hide() } else { w.show() };
    app.state::<Engine>().set_enabled(!visible);
    let store = app.state::<Store>();
    let _ = store.update(|c| c.glass.visible = !visible);
}

fn set_frozen_inner(app: &AppHandle, engine: &Engine, frozen: bool) {
    if engine.mode() == Mode::Lens {
        leave_lens(app);
    }
    engine.set_mode(if frozen { Mode::Frozen } else { Mode::Follow });
    let src = engine.source();
    let store = app.state::<Store>();
    let _ = store.update(|c| {
        c.glass.frozen = frozen;
        c.glass.lens = false;
        c.glass.frozen_x = src.x;
        c.glass.frozen_y = src.y;
    });
    let _ = app.emit_to(GLASS_LABEL, STATE_EVENT, state_of(engine, &store));
}

/// Enter or leave lens mode. In the lens the window rides on the cursor and passes
/// clicks through, so the user can keep working underneath it. Leaving restores a
/// normal, clickable window where the lens last was.
pub fn set_lens_inner(app: &AppHandle, engine: &Engine, lens: bool) {
    if lens {
        engine.set_mode(Mode::Lens);
        if let Some(w) = app.get_webview_window(GLASS_LABEL) {
            let _ = w.set_ignore_cursor_events(true);
        }
    } else {
        leave_lens(app);
        engine.set_mode(Mode::Follow);
    }
    let store = app.state::<Store>();
    let _ = store.update(|c| {
        c.glass.lens = lens;
        if lens {
            c.glass.frozen = false;
        }
    });
    let _ = app.emit_to(GLASS_LABEL, STATE_EVENT, state_of(engine, &store));
}

fn leave_lens(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        let _ = w.set_ignore_cursor_events(false);
        // Persist where the lens ended up, so the window does not jump back to its
        // pre-lens position on the next start.
        if let Ok(p) = w.outer_position() {
            let _ = app.state::<Store>().update(|c| {
                c.glass.x = Some(p.x);
                c.glass.y = Some(p.y);
            });
        }
    }
}

/// In lens mode, keep the window centred on the cursor. Called on every frame poll.
fn ride_cursor(app: &AppHandle, engine: &Engine) {
    if engine.mode() != Mode::Lens {
        return;
    }
    let Some(w) = app.get_webview_window(GLASS_LABEL) else {
        return;
    };
    let (cx, cy) = cursor_pos();
    let Ok(size) = w.outer_size() else { return };
    let target = PhysicalPosition::new(cx - (size.width / 2) as i32, cy - (size.height / 2) as i32);
    if w.outer_position().ok() != Some(target) {
        let _ = w.set_position(target);
    }
}

// ---- commands -------------------------------------------------------------

#[tauri::command]
pub fn glass_state(engine: State<Engine>, store: State<Store>) -> GlassState {
    state_of(&engine, &store)
}

/// The glass reports its inner size (physical px) and wanted zoom; the clamped zoom
/// comes back and is persisted.
#[tauri::command]
pub fn glass_set_view(
    engine: State<Engine>,
    store: State<Store>,
    width_px: u32,
    height_px: u32,
    zoom: f32,
) -> f32 {
    let zoom = engine.set_view(width_px, height_px, zoom);
    let _ = store.update(|c| {
        c.glass.width = width_px;
        c.glass.height = height_px;
        c.glass.zoom = zoom;
    });
    zoom
}

#[tauri::command]
pub fn glass_set_frozen(app: AppHandle, engine: State<Engine>, frozen: bool) {
    set_frozen_inner(&app, &engine, frozen);
}

#[tauri::command]
pub fn glass_set_lens(app: AppHandle, engine: State<Engine>, lens: bool) {
    set_lens_inner(&app, &engine, lens);
}

/// The pointer entered or left the glass. While following, the source holds still so
/// reaching for a control does not swap the picture for "what is under the glass".
#[tauri::command]
pub fn glass_set_hovered(engine: State<Engine>, hovered: bool) {
    engine.set_hovered(hovered);
}

#[tauri::command]
pub fn glass_scroll(engine: State<Engine>, store: State<Store>, dx: i32, dy: i32) {
    engine.scroll(dx, dy);
    let src = engine.source();
    let _ = store.update(|c| {
        c.glass.frozen_x = src.x;
        c.glass.frozen_y = src.y;
    });
}

/// Hide the glass. The global hotkey brings it back; the capture stops meanwhile.
#[tauri::command]
pub fn glass_hide(app: AppHandle) {
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        let _ = w.hide();
    }
    app.state::<Engine>().set_enabled(false);
    let _ = app.state::<Store>().update(|c| c.glass.visible = false);
}

/// Quit ReviewGlass entirely. Distinct from hiding: the hotkey does not bring it back.
#[tauri::command]
pub fn app_quit(app: AppHandle) {
    crate::tray::quit_app(&app);
}

#[tauri::command]
pub fn glass_save_position(store: State<Store>, x: i32, y: i32) {
    let _ = store.update(|c| {
        c.glass.x = Some(x);
        c.glass.y = Some(y);
    });
}

/// Poll for a frame newer than `since`. Response layout, little-endian:
/// `[seq: u64][width: u32][height: u32][rgba bytes]`. When nothing is newer the
/// header carries the current seq with width = height = 0 and no pixels. A capture
/// error is reported as a string so the glass can show its error state.
#[tauri::command]
pub fn glass_frame(app: AppHandle, engine: State<Engine>, since: u64) -> Result<Response, String> {
    ride_cursor(&app, &engine);
    engine.tick().map_err(|e| e.to_string())?;
    if !engine.is_enabled() {
        let mut idle = Vec::with_capacity(16);
        idle.extend_from_slice(&since.to_le_bytes());
        idle.extend_from_slice(&0u32.to_le_bytes());
        idle.extend_from_slice(&0u32.to_le_bytes());
        return Ok(Response::new(idle));
    }
    let mut out = Vec::with_capacity(16);
    match engine.frame_since(since) {
        Some((seq, w, h, rgba)) => {
            out.reserve(rgba.len());
            out.extend_from_slice(&seq.to_le_bytes());
            out.extend_from_slice(&w.to_le_bytes());
            out.extend_from_slice(&h.to_le_bytes());
            out.extend_from_slice(&rgba);
            engine.recycle(rgba);
        }
        None => {
            out.extend_from_slice(&since.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
        }
    }
    Ok(Response::new(out))
}
