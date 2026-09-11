//! ReviewGlass core. Two windows (glass, panel) are declared in tauri.conf.json;
//! the Rust side owns capture, configuration, and later the spool watcher and git.
//! See docs/REVIEWGLASS-SPEC-v0.1.md section 6 for the module contracts.

mod capture;
mod config;
mod glass;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            app.manage(config::Store::open(&config_dir));
            app.manage(capture::Engine::new());

            if let Some(w) = app.get_webview_window(glass::GLASS_LABEL) {
                if let Err(e) = glass::exclude_from_capture(&w) {
                    eprintln!("reviewglass: {e}");
                }
            }
            glass::restore(app.handle());
            if let Err(e) = glass::register_hotkeys(app.handle()) {
                eprintln!("reviewglass: hotkeys not registered: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            glass::glass_state,
            glass::glass_set_view,
            glass::glass_set_frozen,
            glass::glass_scroll,
            glass::glass_save_position,
            glass::glass_hide,
            glass::app_quit,
            glass::glass_frame,
        ])
        .build(tauri::generate_context!())
        .expect("error while building ReviewGlass")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<capture::Engine>().stop();
            }
        });
}
