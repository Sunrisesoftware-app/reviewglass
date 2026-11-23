// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The uninstaller's call (adr.rg.038), handled before Tauri starts: neither a
    // running instance nor the single-instance rule stands in its way.
    if std::env::args().any(|a| a == "--remove-integration") {
        std::process::exit(reviewglass_lib::integration::remove_from_command_line());
    }
    // The installer's last step: an upgrade that uninstalled first gets its connection
    // back; a first install is left for the user's Connect.
    if std::env::args().any(|a| a == "--restore-integration") {
        std::process::exit(reviewglass_lib::integration::restore_from_command_line());
    }
    reviewglass_lib::run()
}
