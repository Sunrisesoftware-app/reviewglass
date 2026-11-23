// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The uninstaller's call (adr.rg.038), handled before Tauri starts: neither a
    // running instance nor the single-instance rule stands in its way.
    if std::env::args().any(|a| a == "--remove-integration") {
        std::process::exit(reviewglass_lib::integration::remove_from_command_line());
    }
    reviewglass_lib::run()
}
