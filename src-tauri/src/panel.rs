//! Panel window (rg.panel-window), Rust side, and the usage loop behind it.
//!
//! The usage model runs on its own thread rather than on the panel's poll. Two reasons:
//! the burn-rate history must accumulate whether or not anyone is looking, and the
//! threshold alerts (rg.notifier) must fire from a closed panel — the whole point of an
//! alert is that the user is elsewhere. The panel reads the latest view; it does not
//! drive the computation.

use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_notification::NotificationExt;

use crate::config::{AlertConfig, Store};
use crate::notifier::{Alert, Notifier, WindowKind};
use crate::session;
use crate::usage::{UsageModel, UsageView};

pub const PANEL_LABEL: &str = "panel";

/// How often both channels are read. Session data changes at the pace of assistant
/// messages, so two seconds is invisible latency and negligible cost.
const TICK: Duration = Duration::from_secs(2);

pub struct PanelState {
    model: Mutex<UsageModel>,
    notifier: Mutex<Notifier>,
    latest: Mutex<Option<UsageView>>,
}

impl Default for PanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelState {
    pub fn new() -> Self {
        Self {
            model: Mutex::new(UsageModel::new()),
            notifier: Mutex::new(Notifier::new()),
            latest: Mutex::new(None),
        }
    }

    /// One pass: read both channels, update the model, decide alerts. Returns the
    /// alerts so the caller can deliver them; the state has already recorded them.
    pub fn tick(&self, alerts_cfg: &AlertConfig) -> Vec<Alert> {
        let reading = session::read_all();
        let view = self.model.lock().observe(&reading);
        let now_s = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let alerts = self.notifier.lock().observe(&view.quota, alerts_cfg, now_s);
        *self.latest.lock() = Some(view);
        alerts
    }
}

/// Start the usage loop. Called once from setup.
pub fn spawn_usage_loop(app: AppHandle) {
    thread::Builder::new()
        .name("reviewglass-usage".into())
        .spawn(move || loop {
            let cfg = app.state::<Store>().get().alerts;
            let alerts = app.state::<PanelState>().tick(&cfg);
            for alert in alerts {
                toast(&app, &alert.title(), &alert.body());
            }
            thread::sleep(TICK);
        })
        .expect("usage loop thread");
}

fn toast(app: &AppHandle, title: &str, body: &str) {
    // A failed toast is not worth more than a line on stderr: the gauge in the panel
    // carries the same state, and the notifier has already marked the threshold fired,
    // so it will not retry and will not nag.
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        eprintln!("reviewglass: toast not shown: {e}");
    }
}

// ---- commands -------------------------------------------------------------

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

/// The latest view the loop produced. `None` only in the first two seconds after start.
#[tauri::command]
pub fn panel_usage(state: State<PanelState>) -> Option<UsageView> {
    state.latest.lock().clone()
}

#[tauri::command]
pub fn alerts_get(store: State<Store>) -> AlertConfig {
    store.get().alerts
}

/// Replace the alert settings. Thresholds are clamped to 1-99 and deduplicated so the
/// settings tab cannot produce an alarm that can never fire or fires twice.
#[tauri::command]
pub fn alerts_set(store: State<Store>, enabled: bool, thresholds: Vec<f64>) -> AlertConfig {
    let mut ts: Vec<f64> = thresholds
        .into_iter()
        .filter(|t| t.is_finite())
        .map(|t| t.round().clamp(1.0, 99.0))
        .collect();
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ts.dedup();
    let cfg = AlertConfig {
        enabled,
        thresholds: ts,
    };
    let saved = cfg.clone();
    let _ = store.update(|c| c.alerts = saved);
    cfg
}

/// Show a sample toast now, so the user can see what an alert looks like and confirm
/// Windows lets it through — without waiting to actually burn 75 % of a window.
#[tauri::command]
pub fn alerts_test(app: AppHandle) -> Result<(), String> {
    let sample = Alert {
        window: WindowKind::FiveHour,
        threshold: 75.0,
        used_percentage: 76.0,
        resets_in_hours: Some(2.25),
        hours_to_limit: Some(1.1),
    };
    app.notification()
        .builder()
        .title(format!("{} (test)", sample.title()))
        .body(sample.body())
        .show()
        .map_err(|e| e.to_string())
}
