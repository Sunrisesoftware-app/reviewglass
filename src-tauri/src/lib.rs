//! ReviewGlass core. Two windows (glass, panel) are declared in tauri.conf.json;
//! the Rust side owns capture, the spool watcher, git and configuration.
//! See docs/REVIEWGLASS-SPEC-v0.1.md section 6 for the module contracts.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running ReviewGlass");
}
