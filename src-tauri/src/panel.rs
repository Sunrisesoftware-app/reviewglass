//! Panel window (rg.panel-window), Rust side: one command that reads both session
//! channels and folds them into the view the Sessions tab renders.
//!
//! The panel polls rather than being pushed to. The data changes at the pace of
//! assistant messages, a second of latency is invisible, and a poll keeps the whole
//! path — spool, transcripts, usage model — in one place that is easy to reason about.

use parking_lot::Mutex;
use tauri::{AppHandle, Manager, State};

use crate::session;
use crate::usage::{UsageModel, UsageView};

/// Holds the sample history the burn rate is computed from, so it survives between polls.
#[derive(Default)]
pub struct PanelState {
    model: Mutex<UsageModel>,
}

impl PanelState {
    pub fn new() -> Self {
        Self {
            model: Mutex::new(UsageModel::new()),
        }
    }
}

pub const PANEL_LABEL: &str = "panel";

/// Bring the panel up. Closing it hides it rather than destroying it (see lib.rs), so
/// this is always able to bring the same window back.
#[tauri::command]
pub fn panel_show(app: AppHandle) {
    if let Some(w) = app.get_webview_window(PANEL_LABEL) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[tauri::command]
pub fn panel_usage(state: State<PanelState>) -> UsageView {
    let reading = session::read_all();
    state.model.lock().observe(&reading)
}
