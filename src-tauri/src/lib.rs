//! ReviewGlass core. The windows (glass, dock with its drawer, halo, finder) are
//! declared in tauri.conf.json; the Rust side owns capture, configuration, the spool
//! and git.
//! See docs/REVIEWGLASS-SPEC.md section 6 for the module contracts.

mod capture;
mod config;
mod diff;
mod dock;
mod follow_session;
mod glass;
mod measure;
mod notifier;
mod panel;
pub mod session;
pub mod spool;
mod tray;
pub mod usage;

use tauri::Manager;

/// A panic leaves a trace. The release build aborts on panic (`panic = "abort"`) and its
/// stderr reaches nowhere, so without this a crash is a Windows Error Reporting line
/// with an offset and nothing else (23.9.2026). One line per panic, appended to
/// `~/.reviewglass/panic.log` — the profile root, never AppData (adr.rg.019) — with the
/// build, the thread, the place and the message; the file is rolled over at 64 KB.
fn install_panic_log() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let place = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = format!(
            "{secs} {} thread '{thread}' panicked at {place}: {msg}\n",
            glass::build_stamp()
        );
        if let Some(dir) = dirs::home_dir().map(|h| h.join(".reviewglass")) {
            let path = dir.join("panic.log");
            let _ = std::fs::create_dir_all(&dir);
            if std::fs::metadata(&path).is_ok_and(|m| m.len() > 64 * 1024) {
                let _ = std::fs::rename(&path, dir.join("panic.log.old"));
            }
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                use std::io::Write;
                let _ = f.write_all(line.as_bytes());
            }
        }
        previous(info);
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_log();
    let context = tauri::generate_context!();
    // The same directory Tauri's app_config_dir resolves to (the config directory and
    // the bundle identifier), computed here because the state is managed before the
    // app exists: see below.
    let config_dir = dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(&context.config().identifier);
    tauri::Builder::default()
        // Every piece of state is managed on the builder, before any window exists.
        // Tauri creates the windows declared in tauri.conf.json before `setup` runs,
        // and creating a WebView2 pumps messages while it waits: a page that is
        // already loaded can have its commands served in that gap. State managed in
        // `setup` was then missing, and a command reaching it through `app.state()`
        // panicked the app on start (23.9.2026: 5 of 5 warm launches, "state()
        // called before manage() for config::Store").
        .manage(config::Store::open(&config_dir))
        .manage(capture::Engine::new())
        .manage(panel::PanelState::new())
        .manage(diff::DiffState::new())
        .manage(follow_session::FollowSessionState::new())
        .manage(dock::DockState::new())
        // A second launch brings the running copy's dock forward instead of starting
        // another that would fight it for the config file and the capture. The dock is
        // the fixed point (adr.rg.018): nothing else appears until it is asked — the
        // first dock build showed the glass and the panel here, and the owner saw a
        // panel land in the middle of the screen at every click on the shortcut.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tray::show_dock_window(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            if let Some(w) = app.get_webview_window(glass::GLASS_LABEL) {
                if let Err(e) = glass::exclude_from_capture(&w) {
                    eprintln!("reviewglass: {e}");
                }
            }
            tray::install(app.handle())?;
            glass::prepare_overlays(app.handle());
            glass::restore(app.handle());
            dock::prepare(app.handle());
            if let Err(e) = glass::register_hotkeys(app.handle()) {
                eprintln!("reviewglass: hotkeys not registered: {e}");
            }
            panel::spawn_usage_loop(app.handle().clone());
            glass::spawn_lens_rider(app.handle().clone());
            diff::spawn_diff_loop(app.handle().clone());
            follow_session::spawn(app.handle().clone());
            Ok(())
        })
        .on_menu_event(|app, event| glass::on_menu(app, event.id().as_ref()))
        .invoke_handler(tauri::generate_handler![
            glass::glass_state,
            glass::glass_set_view,
            glass::glass_save_size,
            glass::glass_set_frozen,
            glass::glass_set_lens,
            glass::glass_set_halo,
            glass::glass_set_pane_lock,
            glass::glass_set_pane_fit,
            glass::glass_ui_scale_menu,
            glass::glass_menu,
            glass::glass_set_hovered,
            glass::glass_scroll,
            glass::glass_save_position,
            glass::glass_hide,
            glass::app_quit,
            glass::glass_frame,
            dock::dock_snap,
            dock::dock_activate,
            dock::dock_menu,
            dock::dock_state,
            dock::dock_drawer,
            dock::dock_set_tab,
            dock::dock_drawer_resized,
            follow_session::follow_session_state,
            follow_session::follow_session_set,
            glass::hotkey_set_toggle,
            glass::follow_log_set,
            glass::glass_log,
            panel::panel_usage,
            panel::panel_show,
            panel::alerts_get,
            panel::alerts_set,
            panel::alerts_test,
            diff::panel_diffs,
            diff::file::panel_file_view,
        ])
        .build(context)
        .expect("error while building ReviewGlass")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<capture::Engine>().stop();
            }
        });
}
