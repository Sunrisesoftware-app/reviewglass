//! Glass window (rg.glass-window), Rust side: the commands the magnifier frontend
//! calls, the global hotkeys, self-exclusion from capture, and persistence of
//! geometry, zoom and freeze state through the config store.

use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Response;
use tauri::menu::{CheckMenuItem, ContextMenu, Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};

use crate::capture::{cursor_pos, Engine, Mode};
use crate::config::{LoadOutcome, Store};

pub const GLASS_LABEL: &str = "glass";
pub const HALO_LABEL: &str = "halo";
pub const FINDER_LABEL: &str = "finder";
/// The bar sizes offered.
const UI_SCALES: [f32; 5] = [1.0, 1.25, 1.5, 1.75, 2.0];
/// Event sent to the glass when freeze/zoom changes from outside the webview.
pub const STATE_EVENT: &str = "glass:state";
/// Event asking the glass to step its zoom (the webview owns the zoom value, since it
/// is tied to the canvas size it reports).
pub const ZOOM_EVENT: &str = "glass:zoom";
/// Event sent to the glass when the pane under the cursor changed (adr.rg.017).
pub const PANE_EVENT: &str = "glass:pane";
/// The last pane sequence the glass was told about; see `glass_frame`.
static PANE_TOLD: AtomicU64 = AtomicU64::new(0);
/// The last view the measurement log recorded (size, zoom bits, derived).
static VIEW_LOGGED: parking_lot::Mutex<Option<(u32, u32, u32, bool)>> =
    parking_lot::Mutex::new(None);

#[derive(Clone, Serialize)]
pub struct GlassState {
    /// Shown on screen. Since adr.rg.018 this is the dock's state, not a setting: the
    /// glass starts hidden at every start.
    pub visible: bool,
    pub zoom: f32,
    pub frozen: bool,
    pub lens: bool,
    pub halo: bool,
    pub ui_scale: f32,
    pub pane_lock: bool,
    pub pane_fit: bool,
    /// Width of the pane under the cursor in source pixels; absent when none is found
    /// or the lock is off.
    pub pane_width: Option<u32>,
    /// "0.1.0 c4c4521", with a "+" after the hash when the tree had uncommitted
    /// changes: which build is being looked at, so a test never assumes the wrong one.
    pub build: String,
    /// The global shortcut that switches the glass on and off, as configured — so the
    /// dock can say it. A shortcut nobody was told about is not a feature.
    pub hotkey_toggle: String,
    /// The Follow measurement log (temporary tooling, session 3) and where it writes.
    pub follow_log: bool,
    pub follow_log_path: String,
    pub config: LoadOutcome,
}

/// The version and the commit the binary was built from (see build.rs).
pub fn build_stamp() -> String {
    format!("{} {}", env!("CARGO_PKG_VERSION"), env!("RG_BUILD_COMMIT"))
}

/// Payload of `PANE_EVENT`.
#[derive(Clone, Serialize)]
pub struct PaneState {
    pub width: Option<u32>,
}

fn state_of(engine: &Engine, store: &Store) -> GlassState {
    let mode = engine.mode();
    let g = store.get().glass;
    GlassState {
        visible: g.visible,
        zoom: engine.zoom(),
        frozen: mode == Mode::Frozen,
        lens: mode == Mode::Lens,
        halo: g.halo,
        ui_scale: g.ui_scale,
        pane_lock: g.pane_lock,
        pane_fit: g.pane_fit,
        pane_width: engine.pane().map(|p| p.width()),
        build: build_stamp(),
        hotkey_toggle: store.get().hotkeys.toggle_glass.clone(),
        follow_log: g.follow_log,
        follow_log_path: crate::measure::path().display().to_string(),
        config: store.outcome(),
    }
}

/// The header line of the measurement log: the build and the settings that shape Fit.
fn log_header(cfg: &crate::config::GlassConfig) -> String {
    format!(
        "build {}; zoom={} pane_lock={} pane_fit={} glass={}x{} lens={}x{}",
        build_stamp(),
        cfg.zoom,
        cfg.pane_lock as u8,
        cfg.pane_fit as u8,
        cfg.width,
        cfg.height,
        cfg.lens_width,
        cfg.lens_height
    )
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

/// Restore geometry and mode from the config store at startup. The glass itself
/// starts hidden: the dock is what shows it (adr.rg.018).
pub fn restore(app: &AppHandle) {
    let store = app.state::<Store>();
    let engine = app.state::<Engine>();
    let cfg = store.get().glass;
    if cfg.follow_log {
        // Left on at the last quit: a new file for this run.
        if let Err(e) = crate::measure::set_on(true, &log_header(&cfg)) {
            eprintln!("reviewglass: follow log: {e}");
        }
    }
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        let _ = w.set_size(PhysicalSize::new(cfg.width, cfg.height));
        if let (Some(x), Some(y)) = (cfg.x, cfg.y) {
            let _ = w.set_position(PhysicalPosition::new(x, y));
        }
        let _ = w.hide();
    }
    engine.set_enabled(false);
    let _ = store.update(|c| c.glass.visible = false);
    engine.set_view(cfg.width, cfg.height, cfg.zoom);
    engine.set_pane_lock(cfg.pane_lock);
    if cfg.lens {
        set_lens_inner(app, &engine, true);
    } else if cfg.frozen {
        engine.freeze_at(cfg.frozen_x, cfg.frozen_y);
    }
    // A restored still has no pixels in hand until one frame arrives; the engine lets
    // exactly one through and then holds (see Engine::freeze_at).
    let _ = engine.is_still();
}

/// Change the glass on/off shortcut at runtime: parse it, drop every registered
/// shortcut, register the new set, and only then store it. A combination that cannot
/// be parsed or is taken by another application is reported and nothing changes.
#[tauri::command]
pub fn hotkey_set_toggle(
    app: AppHandle,
    store: State<Store>,
    shortcut: String,
) -> Result<String, String> {
    let wanted = shortcut.trim().to_string();
    let parsed: Shortcut = wanted.parse().map_err(|e| format!("not a shortcut: {e}"))?;
    let previous = store.get().hotkeys.toggle_glass;
    let _ = store.update(|c| c.hotkeys.toggle_glass = wanted.clone());
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    if let Err(e) = register_hotkeys(&app) {
        // Back to the previous set, which did register.
        let _ = store.update(|c| c.hotkeys.toggle_glass = previous);
        let _ = gs.unregister_all();
        let _ = register_hotkeys(&app);
        return Err(format!("could not register {}: {e}", parsed.into_string()));
    }
    let engine = app.state::<Engine>();
    broadcast_state(&app, &engine, &store);
    Ok(wanted)
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
    let engine = app.state::<Engine>();
    engine.set_enabled(!visible);
    let store = app.state::<Store>();
    let _ = store.update(|c| c.glass.visible = !visible);
    broadcast_state(app, &engine, &store);
}

/// Hide the glass and stop its capture. The dock, the hotkey and the tray bring it
/// back.
pub fn hide_glass(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        let _ = w.hide();
    }
    let engine = app.state::<Engine>();
    engine.set_enabled(false);
    let store = app.state::<Store>();
    let _ = store.update(|c| c.glass.visible = false);
    broadcast_state(app, &engine, &store);
}

/// Tell every window the glass's state: the glass redraws its bar, the dock lights
/// the right button.
pub fn broadcast_state(app: &AppHandle, engine: &Engine, store: &Store) {
    let _ = app.emit(STATE_EVENT, state_of(engine, store));
}

pub fn set_frozen_inner(app: &AppHandle, engine: &Engine, frozen: bool) {
    // Freezing from the lens keeps the lens's size and position: the still is what the
    // lens was showing, and the user is about to drag it somewhere to keep it.
    let was_lens = engine.mode() == Mode::Lens;
    if was_lens {
        let store = app.state::<Store>();
        let lens_size = (store.get().glass.lens_width, store.get().glass.lens_height);
        let pos = app
            .get_webview_window(GLASS_LABEL)
            .and_then(|w| w.outer_position().ok());
        let _ = store.update(|c| {
            c.glass.width = lens_size.0;
            c.glass.height = lens_size.1;
            if let Some(p) = pos {
                c.glass.x = Some(p.x);
                c.glass.y = Some(p.y);
            }
        });
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
    broadcast_state(app, engine, &store);
}

/// Enter or leave lens mode. In the lens the window rides on the cursor at its own,
/// smaller size. It is NOT click-through: the cursor is always over it, so a
/// right-click opens the glass menu and a double-click freezes what is under it —
/// which is the reading-then-parking workflow the lens exists for. Leaving restores the
/// parked size where the lens last was.
pub fn set_lens_inner(app: &AppHandle, engine: &Engine, lens: bool) {
    let store = app.state::<Store>();
    let cfg = store.get().glass;
    let Some(w) = app.get_webview_window(GLASS_LABEL) else {
        return;
    };
    // Mode first, size second: the resize event the glass reports lands in the slot of
    // the mode that is current when it arrives.
    if lens {
        engine.set_mode(Mode::Lens);
        let _ = w.set_size(PhysicalSize::new(cfg.lens_width, cfg.lens_height));
    } else {
        engine.set_mode(Mode::Follow);
        let _ = w.set_size(PhysicalSize::new(cfg.width, cfg.height));
        if let Ok(p) = w.outer_position() {
            let _ = store.update(|c| {
                c.glass.x = Some(p.x);
                c.glass.y = Some(p.y);
            });
        }
    }
    let _ = store.update(|c| {
        c.glass.lens = lens;
        if lens {
            c.glass.frozen = false;
        }
    });
    broadcast_state(app, engine, &store);
}

/// Ride the cursor on a thread of its own, at a rate the webview's frame poll cannot
/// match. Moving a window from the poll made it step at 30 Hz with setTimeout jitter on
/// top; this runs at ~120 Hz and only touches a window when its target changed.
///
/// Two riders share the thread: the lens (the glass itself, in Lens mode) and the halo
/// (the ring around the pointer, in Follow mode). At most one is riding at a time. The
/// finder (adr.rg.017) sits on the engine's source rectangle rather than the cursor,
/// and moves only when that rectangle does.
pub fn spawn_lens_rider(app: AppHandle) {
    thread::Builder::new()
        .name("reviewglass-rider".into())
        .spawn(move || {
            let mut halo_shown = false;
            let mut finder_shown = false;
            let mut finder_rect: Option<crate::capture::SourceRect> = None;
            loop {
                let engine = app.state::<Engine>();
                let mode = engine.mode();
                let enabled = engine.is_enabled();
                let cfg = app.state::<Store>().get().glass;
                let following = enabled && mode == Mode::Follow;
                let halo_wanted = following && cfg.halo;
                let finder_wanted = following && cfg.pane_lock && engine.pane().is_some();

                let riding = if mode == Mode::Lens && enabled {
                    Some(GLASS_LABEL)
                } else if halo_wanted {
                    Some(HALO_LABEL)
                } else {
                    None
                };

                if let Some(halo) = app.get_webview_window(HALO_LABEL) {
                    if halo_wanted != halo_shown {
                        let _ = if halo_wanted {
                            halo.show()
                        } else {
                            halo.hide()
                        };
                        halo_shown = halo_wanted;
                    }
                }

                if let Some(finder) = app.get_webview_window(FINDER_LABEL) {
                    if finder_wanted {
                        let src = engine.source();
                        if finder_rect != Some(src) {
                            let _ = finder.set_position(PhysicalPosition::new(src.x, src.y));
                            let _ = finder.set_size(PhysicalSize::new(src.w, src.h));
                            finder_rect = Some(src);
                        }
                    }
                    if finder_wanted != finder_shown {
                        let _ = if finder_wanted {
                            finder.show()
                        } else {
                            finder.hide()
                        };
                        finder_shown = finder_wanted;
                    }
                }

                // A menu open over the glass holds the rider too: the menu pops at the
                // cursor and a lens that kept riding would carry the picture out from
                // under it while the user reaches for an item.
                let paused = riding == Some(GLASS_LABEL) && engine.is_held();
                if let Some(label) = riding.filter(|_| !paused) {
                    if let Some(w) = app.get_webview_window(label) {
                        let (cx, cy) = cursor_pos();
                        if let Ok(size) = w.outer_size() {
                            let target = PhysicalPosition::new(
                                cx - (size.width / 2) as i32,
                                cy - (size.height / 2) as i32,
                            );
                            if w.outer_position().ok() != Some(target) {
                                let _ = w.set_position(target);
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(8));
                } else if paused {
                    // Riding again the moment the menu closes.
                    thread::sleep(Duration::from_millis(8));
                } else if finder_wanted {
                    // The source rectangle changes at the frame poll's rate at most.
                    thread::sleep(Duration::from_millis(33));
                } else {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        })
        .expect("rider thread");
}

/// The halo and the finder must never land in the picture, and must never take a
/// click.
pub fn prepare_overlays(app: &AppHandle) {
    for label in [HALO_LABEL, FINDER_LABEL] {
        if let Some(w) = app.get_webview_window(label) {
            if let Err(e) = exclude_from_capture(&w) {
                eprintln!("reviewglass: {label} {e}");
            }
            let _ = w.set_ignore_cursor_events(true);
        }
    }
}

// ---- context menu ---------------------------------------------------------

const M_FREEZE: &str = "glass-freeze";
const M_LENS: &str = "glass-lens";
const M_ZOOM_IN: &str = "glass-zoom-in";
const M_ZOOM_OUT: &str = "glass-zoom-out";
const M_HIDE: &str = "glass-hide";
pub const M_PANEL: &str = "glass-panel";
pub const M_QUIT: &str = "glass-quit";
/// Bar-size menu items carry their scale after this prefix ("glass-ui-1.25").
const M_UI_PREFIX: &str = "glass-ui-";

/// Pop a menu over the glass and hold the engine while it is open: the picture and
/// the source rectangle stay as they were at the right-click, the lens does not ride
/// (see `Engine::hold`). `popup` blocks until the menu closes on Windows — a pick or a
/// dismissal alike — and a pick is delivered to `on_menu` through the event loop
/// afterwards, inside the grace the release grants.
pub fn popup_held(
    engine: &Engine,
    menu: &Menu<tauri::Wry>,
    window: &WebviewWindow,
) -> tauri::Result<()> {
    engine.hold();
    let shown = menu.popup(window.as_ref().window());
    engine.release();
    shown
}

/// Build and pop the glass's right-click menu at the cursor. Every entry mirrors a
/// control on the bar or a hotkey; the menu exists so the same actions are one click
/// away when the bar is dim, the lens is riding, or the user simply reaches for the
/// right button first. While it is open the picture holds, so "Freeze this picture"
/// keeps the picture that was right-clicked on — never the menu itself.
pub fn popup_menu(app: &AppHandle, engine: &Engine) -> tauri::Result<()> {
    let mode = engine.mode();
    let freeze_label = if mode == Mode::Frozen {
        "Resume live view\tF"
    } else {
        "Freeze this picture\tF"
    };
    let lens_label = if mode == Mode::Lens {
        "Leave lens\tL"
    } else {
        "Lens: ride on the cursor\tL"
    };
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, M_FREEZE, freeze_label, true, None::<&str>)?,
            &MenuItem::with_id(app, M_LENS, lens_label, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                M_ZOOM_IN,
                "Zoom in\t+",
                mode != Mode::Frozen,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app,
                M_ZOOM_OUT,
                "Zoom out\t−",
                mode != Mode::Frozen,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, M_PANEL, "Show sessions panel", true, None::<&str>)?,
            &MenuItem::with_id(app, M_HIDE, "Hide glass\tEsc", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, M_QUIT, "Quit ReviewGlass", true, None::<&str>)?,
        ],
    )?;
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        popup_held(engine, &menu, &w)?;
    }
    Ok(())
}

/// Pop a menu of bar sizes at the cursor, the current one checked. A blind cycle
/// through five steps meant four more clicks to come back from the largest; a menu
/// shows every size at once and takes one.
pub fn popup_ui_scale_menu(app: &AppHandle) -> tauri::Result<()> {
    let current = app.state::<Store>().get().glass.ui_scale;
    let mut items: Vec<CheckMenuItem<tauri::Wry>> = Vec::new();
    for s in UI_SCALES {
        items.push(CheckMenuItem::with_id(
            app,
            format!("{M_UI_PREFIX}{s}"),
            format!("Bar size {} %", (s * 100.0).round()),
            true,
            (s - current).abs() < 0.01,
            None::<&str>,
        )?);
    }
    let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        items.iter().map(|i| i as _).collect();
    let menu = Menu::with_items(app, &refs)?;
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        popup_held(&app.state::<Engine>(), &menu, &w)?;
    }
    Ok(())
}

/// Handle a pick from the glass menu. Wired in lib.rs; ids that are not ours fall
/// through untouched so the tray's own handler still sees its events.
pub fn on_menu(app: &AppHandle, id: &str) {
    let engine = app.state::<Engine>();
    if let Some(scale) = id.strip_prefix(M_UI_PREFIX) {
        if let Ok(s) = scale.parse::<f32>() {
            let store = app.state::<Store>();
            let _ = store.update(|c| c.glass.ui_scale = s);
            broadcast_state(app, &engine, &store);
        }
        return;
    }
    match id {
        crate::dock::M_TOGGLE => toggle_visible(app),
        M_FREEZE => {
            let frozen = engine.mode() != Mode::Frozen;
            set_frozen_inner(app, &engine, frozen);
        }
        M_LENS => {
            let lens = engine.mode() != Mode::Lens;
            set_lens_inner(app, &engine, lens);
        }
        M_ZOOM_IN | M_ZOOM_OUT => {
            let step = if id == M_ZOOM_IN { 0.25 } else { -0.25 };
            let _ = app.emit_to(GLASS_LABEL, ZOOM_EVENT, step);
        }
        M_HIDE => glass_hide(app.clone()),
        M_PANEL => crate::tray::show_panel_window(app),
        M_QUIT => crate::tray::quit_app(app),
        _ => {}
    }
}

// ---- commands -------------------------------------------------------------

#[tauri::command]
pub fn glass_state(engine: State<Engine>, store: State<Store>) -> GlassState {
    state_of(&engine, &store)
}

/// The glass reports its inner size (physical px) and wanted zoom; the clamped zoom
/// comes back and is persisted — unless it is `derived`, a zoom the fit lowered so a
/// wide column fits the screen. The user's own zoom stays the setting; the derived one
/// is in effect only while that column is.
#[tauri::command]
pub fn glass_set_view(
    engine: State<Engine>,
    store: State<Store>,
    width_px: u32,
    height_px: u32,
    zoom: f32,
    derived: bool,
) -> f32 {
    let zoom = engine.set_view(width_px, height_px, zoom);
    if crate::measure::is_on() {
        // Reported on every relayout; only a change is worth a line.
        let now = (width_px, height_px, zoom.to_bits(), derived);
        if VIEW_LOGGED.lock().replace(now) != Some(now) {
            crate::measure::log(|| {
                format!(
                    "view {width_px}x{height_px} zoom={zoom} derived={}",
                    derived as u8
                )
            });
        }
    }
    if !derived {
        let _ = store.update(|c| c.glass.zoom = zoom);
    }
    zoom
}

/// The window's inner size, reported by the glass when it changes. Stored under the
/// current mode's slot: the parked glass and the lens keep separate sizes. While Fit
/// is on in Follow, the width is the pane's choice, not the user's, and is not stored
/// (a derived value must never be written back as the setting it came from).
#[tauri::command]
pub fn glass_save_size(engine: State<Engine>, store: State<Store>, width: u32, height: u32) {
    let mode = engine.mode();
    let _ = store.update(|c| {
        if mode == Mode::Lens {
            c.glass.lens_width = width;
            c.glass.lens_height = height;
        } else {
            if !(mode == Mode::Follow && c.glass.pane_fit && c.glass.pane_lock) {
                c.glass.width = width;
            }
            c.glass.height = height;
        }
    });
}

/// Pane lock on or off (adr.rg.017). Off forgets the pane at once, so the picture
/// returns to following the cursor in both axes on the next tick.
#[tauri::command]
pub fn glass_set_pane_lock(app: AppHandle, engine: State<Engine>, store: State<Store>, lock: bool) {
    engine.set_pane_lock(lock);
    let _ = store.update(|c| c.glass.pane_lock = lock);
    if !lock {
        restore_own_width(&app, &store);
    }
    broadcast_state(&app, &engine, &store);
}

/// Fit on or off. Off restores the user's own width; on lets the glass react to the
/// next pane event.
#[tauri::command]
pub fn glass_set_pane_fit(app: AppHandle, engine: State<Engine>, store: State<Store>, fit: bool) {
    crate::measure::log(|| format!("fit-toggle {}", fit as u8));
    let _ = store.update(|c| c.glass.pane_fit = fit);
    if !fit {
        restore_own_width(&app, &store);
    }
    broadcast_state(&app, &engine, &store);
}

/// Put the parked glass back at its remembered width, keeping the current height.
fn restore_own_width(app: &AppHandle, store: &Store) {
    if let Some(w) = app.get_webview_window(GLASS_LABEL) {
        if let Ok(size) = w.inner_size() {
            let own = store.get().glass.width;
            if size.width != own {
                let _ = w.set_size(PhysicalSize::new(own, size.height));
            }
        }
    }
}

/// Switch the Follow measurement log on or off (Settings tab). On starts the file
/// over; the answer is the state, with the path the log is written to.
#[tauri::command]
pub fn follow_log_set(
    app: AppHandle,
    engine: State<Engine>,
    store: State<Store>,
    on: bool,
) -> Result<GlassState, String> {
    let cfg = store.get().glass;
    crate::measure::set_on(on, &log_header(&cfg))?;
    let _ = store.update(|c| c.glass.follow_log = on);
    broadcast_state(&app, &engine, &store);
    Ok(state_of(&engine, &store))
}

/// A line from the glass page for the measurement log: Fit's decisions and resizes,
/// stamped with the same clock as the engine's lines.
#[tauri::command]
pub fn glass_log(line: String) {
    crate::measure::log(|| line);
}

#[tauri::command]
pub fn glass_set_frozen(app: AppHandle, engine: State<Engine>, frozen: bool) {
    set_frozen_inner(&app, &engine, frozen);
}

#[tauri::command]
pub fn glass_set_lens(app: AppHandle, engine: State<Engine>, lens: bool) {
    set_lens_inner(&app, &engine, lens);
}

#[tauri::command]
pub fn glass_set_halo(store: State<Store>, halo: bool) {
    let _ = store.update(|c| c.glass.halo = halo);
}

/// Open the bar-size menu (the Aa button).
#[tauri::command]
pub fn glass_ui_scale_menu(app: AppHandle) -> Result<(), String> {
    popup_ui_scale_menu(&app).map_err(|e| e.to_string())
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

/// Hide the glass. The dock, the hotkey and the tray bring it back; the capture stops
/// meanwhile.
#[tauri::command]
pub fn glass_hide(app: AppHandle) {
    hide_glass(&app);
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
pub fn glass_menu(app: AppHandle, engine: State<Engine>) -> Result<(), String> {
    popup_menu(&app, &engine).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn glass_frame(app: AppHandle, engine: State<Engine>, since: u64) -> Result<Response, String> {
    engine.tick().map_err(|e| e.to_string())?;
    // The pane is read on the frame poll, which is where the glass already listens;
    // one event per change, not one per frame.
    let pane_seq = engine.pane_seq();
    if PANE_TOLD.swap(pane_seq, Ordering::Relaxed) != pane_seq {
        crate::measure::log(|| {
            let width = engine
                .pane()
                .map(|p| p.width().to_string())
                .unwrap_or_else(|| "none".into());
            format!("pane-event width={width}")
        });
        let _ = app.emit_to(
            GLASS_LABEL,
            PANE_EVENT,
            PaneState {
                width: engine.pane().map(|p| p.width()),
            },
        );
    }
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
