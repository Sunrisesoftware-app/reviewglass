//! ReviewGlass core. Two windows (glass, panel) are declared in tauri.conf.json;
//! the Rust side owns capture, configuration, and later the spool watcher and git.
//! See docs/REVIEWGLASS-SPEC.md section 6 for the module contracts.

mod capture;
mod config;
mod dock;
mod glass;
mod measure;
mod notifier;
mod panel;
pub mod session;
pub mod spool;
mod tray;
pub mod usage;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            let config_dir = app.path().app_config_dir()?;
            app.manage(config::Store::open(&config_dir));
            app.manage(capture::Engine::new());
            app.manage(panel::PanelState::new());

            if let Some(w) = app.get_webview_window(glass::GLASS_LABEL) {
                if let Err(e) = glass::exclude_from_capture(&w) {
                    eprintln!("reviewglass: {e}");
                }
            }
            // Closing the panel hides it instead of destroying it: it is a companion
            // window, and a user who closes it to get it out of the way should be able
            // to bring it back from the glass rather than restarting the app.
            if let Some(w) = app.get_webview_window(panel::PANEL_LABEL) {
                let handle = w.clone();
                w.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = handle.hide();
                    }
                });
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
            dock::panel_save_size,
            glass::hotkey_set_toggle,
            glass::follow_log_set,
            glass::glass_log,
            panel::panel_usage,
            panel::panel_show,
            panel::alerts_get,
            panel::alerts_set,
            panel::alerts_test,
        ])
        .build(tauri::generate_context!())
        .expect("error while building ReviewGlass")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<capture::Engine>().stop();
            }
        });
}
