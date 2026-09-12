//! Notifier (rg.notifier): the warning that arrives before the limit, not after.
//!
//! Configurable thresholds per quota window (default 75 % and 90 %). Each fires once
//! per threshold per window and re-arms when the window resets, so the user hears about
//! a crossing exactly once — never on every poll, never twice in one window, and never
//! for a window that has since reset.
//!
//! The decision of *what* crossed is made here, from the `AccountQuota` the usage model
//! already produces; delivery (a Windows toast) happens in the usage loop, so this
//! logic is testable without a desktop.

use serde::{Deserialize, Serialize};

use crate::usage::{AccountQuota, QuotaWindow};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertConfig {
    /// Enabled at all. Off means the model still tracks thresholds so that turning it on
    /// later does not replay every crossing already passed.
    pub enabled: bool,
    /// Percentages, ascending. Each is a separate alarm.
    pub thresholds: Vec<f64>,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            thresholds: vec![75.0, 90.0],
        }
    }
}

/// One alert the notifier decided to raise.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Alert {
    pub window: WindowKind,
    pub threshold: f64,
    pub used_percentage: f64,
    /// Hours until the window resets, when known.
    pub resets_in_hours: Option<f64>,
    /// Hours until 100 % at the current rate, when the model could estimate one.
    pub hours_to_limit: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowKind {
    FiveHour,
    SevenDay,
}

impl WindowKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::FiveHour => "5-hour",
            Self::SevenDay => "7-day",
        }
    }
}

/// Per-window memory of which thresholds have fired in the current reset period.
#[derive(Debug, Default)]
struct WindowState {
    resets_at: Option<i64>,
    fired: Vec<f64>,
}

impl WindowState {
    /// Thresholds that crossed since last time, in ascending order.
    fn crossings(&mut self, w: &QuotaWindow, thresholds: &[f64]) -> Vec<f64> {
        // A new reset period re-arms everything. A window with no resets_at at all is
        // treated as one long period: better to say it once than never.
        if w.resets_at.is_some() && w.resets_at != self.resets_at {
            self.resets_at = w.resets_at;
            self.fired.clear();
        }
        // A percentage that fell inside the same period is a reset we did not see
        // through resets_at; re-arm rather than stay silent until the next period.
        let max_fired = self.fired.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if max_fired.is_finite() && w.used_percentage < max_fired - 1.0 {
            self.fired.clear();
        }
        let mut out = Vec::new();
        for &t in thresholds {
            if w.used_percentage >= t && !self.fired.iter().any(|f| (*f - t).abs() < 1e-9) {
                self.fired.push(t);
                out.push(t);
            }
        }
        out
    }
}

#[derive(Debug, Default)]
pub struct Notifier {
    five_hour: WindowState,
    seven_day: WindowState,
}

impl Notifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compare the quota against the thresholds and return what newly crossed. The
    /// state advances whether or not alerts are enabled, so enabling later does not
    /// replay history; only the *returned* list is gated.
    pub fn observe(&mut self, quota: &AccountQuota, cfg: &AlertConfig, now_s: i64) -> Vec<Alert> {
        let mut thresholds = cfg.thresholds.clone();
        thresholds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        thresholds.dedup();

        let mut alerts = Vec::new();
        for (kind, window, state) in [
            (WindowKind::FiveHour, &quota.five_hour, &mut self.five_hour),
            (WindowKind::SevenDay, &quota.seven_day, &mut self.seven_day),
        ] {
            let Some(w) = window else { continue };
            if w.stale {
                continue;
            }
            for t in state.crossings(w, &thresholds) {
                alerts.push(Alert {
                    window: kind,
                    threshold: t,
                    used_percentage: w.used_percentage,
                    resets_in_hours: w
                        .resets_at
                        .map(|r| (r - now_s) as f64 / 3600.0)
                        .filter(|h| *h > 0.0),
                    hours_to_limit: w.hours_to_limit,
                });
            }
        }
        if cfg.enabled {
            alerts
        } else {
            Vec::new()
        }
    }
}

impl Alert {
    pub fn title(&self) -> String {
        format!(
            "Claude {} window at {}%",
            self.window.label(),
            self.used_percentage.round() as i64
        )
    }

    /// One line the user can act on: how much is left, and how long that lasts.
    pub fn body(&self) -> String {
        let mut parts = Vec::with_capacity(2);
        if let Some(h) = self.resets_in_hours {
            parts.push(format!("resets in {}", hours(h)));
        }
        if let Some(h) = self.hours_to_limit {
            parts.push(format!(
                "~{} to the limit at the current rate (estimate)",
                hours(h)
            ));
        }
        if parts.is_empty() {
            format!("Crossed your {}% alert.", self.threshold.round() as i64)
        } else {
            parts.join(" · ")
        }
    }
}

fn hours(h: f64) -> String {
    if h >= 1.0 {
        let whole = h.floor() as i64;
        let mins = ((h - whole as f64) * 60.0).round() as i64;
        if mins == 0 {
            format!("{whole} h")
        } else {
            format!("{whole} h {mins} min")
        }
    } else {
        format!("{} min", (h * 60.0).round().max(1.0) as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quota(five: Option<(f64, i64)>, seven: Option<(f64, i64)>) -> AccountQuota {
        let mk = |(p, r): (f64, i64)| QuotaWindow {
            used_percentage: p,
            resets_at: Some(r),
            burn_per_hour: None,
            hours_to_limit: None,
            stale: false,
        };
        AccountQuota {
            five_hour: five.map(mk),
            seven_day: seven.map(mk),
            absence: None,
            source_session: None,
        }
    }

    #[test]
    fn a_crossing_fires_once_and_not_again_on_the_next_poll() {
        let mut n = Notifier::new();
        let cfg = AlertConfig::default();
        let first = n.observe(&quota(Some((76.0, 1000)), None), &cfg, 0);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].threshold, 75.0);
        assert_eq!(first[0].window, WindowKind::FiveHour);
        // Same window, a little higher, same threshold: silence.
        assert!(n
            .observe(&quota(Some((80.0, 1000)), None), &cfg, 0)
            .is_empty());
    }

    #[test]
    fn jumping_past_two_thresholds_fires_both_in_order() {
        let mut n = Notifier::new();
        let alerts = n.observe(&quota(Some((95.0, 1000)), None), &AlertConfig::default(), 0);
        let ts: Vec<f64> = alerts.iter().map(|a| a.threshold).collect();
        assert_eq!(ts, vec![75.0, 90.0]);
    }

    #[test]
    fn a_reset_re_arms_the_window() {
        let mut n = Notifier::new();
        let cfg = AlertConfig::default();
        assert_eq!(
            n.observe(&quota(Some((80.0, 1000)), None), &cfg, 0).len(),
            1
        );
        // New resets_at: a new period. Crossing 75 again is news again.
        assert_eq!(
            n.observe(&quota(Some((80.0, 2000)), None), &cfg, 0).len(),
            1
        );
    }

    #[test]
    fn a_fall_inside_one_period_also_re_arms() {
        // resets_at can lag; the percentage dropping is the reset we can see.
        let mut n = Notifier::new();
        let cfg = AlertConfig::default();
        assert_eq!(
            n.observe(&quota(Some((80.0, 1000)), None), &cfg, 0).len(),
            1
        );
        assert!(n
            .observe(&quota(Some((5.0, 1000)), None), &cfg, 0)
            .is_empty());
        assert_eq!(
            n.observe(&quota(Some((77.0, 1000)), None), &cfg, 0).len(),
            1
        );
    }

    #[test]
    fn the_two_windows_are_independent() {
        let mut n = Notifier::new();
        let cfg = AlertConfig::default();
        let a = n.observe(&quota(Some((80.0, 1000)), Some((80.0, 5000))), &cfg, 0);
        assert_eq!(a.len(), 2);
        assert_ne!(a[0].window, a[1].window);
    }

    #[test]
    fn a_stale_window_never_alerts() {
        let mut n = Notifier::new();
        let mut q = quota(Some((95.0, 1000)), None);
        q.five_hour.as_mut().unwrap().stale = true;
        assert!(n.observe(&q, &AlertConfig::default(), 0).is_empty());
    }

    #[test]
    fn disabling_gates_delivery_but_not_memory() {
        let mut n = Notifier::new();
        let off = AlertConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(n
            .observe(&quota(Some((80.0, 1000)), None), &off, 0)
            .is_empty());
        // Turned on later at the same level: the crossing already happened, no replay.
        assert!(n
            .observe(&quota(Some((81.0, 1000)), None), &AlertConfig::default(), 0)
            .is_empty());
    }

    #[test]
    fn custom_thresholds_are_sorted_and_deduplicated() {
        let mut n = Notifier::new();
        let cfg = AlertConfig {
            enabled: true,
            thresholds: vec![90.0, 50.0, 50.0],
        };
        let ts: Vec<f64> = n
            .observe(&quota(Some((91.0, 1000)), None), &cfg, 0)
            .iter()
            .map(|a| a.threshold)
            .collect();
        assert_eq!(ts, vec![50.0, 90.0]);
    }

    #[test]
    fn the_body_says_what_the_user_can_act_on() {
        let a = Alert {
            window: WindowKind::FiveHour,
            threshold: 75.0,
            used_percentage: 76.4,
            resets_in_hours: Some(2.25),
            hours_to_limit: Some(0.5),
        };
        assert_eq!(a.title(), "Claude 5-hour window at 76%");
        assert_eq!(
            a.body(),
            "resets in 2 h 15 min · ~30 min to the limit at the current rate (estimate)"
        );
        let bare = Alert {
            resets_in_hours: None,
            hours_to_limit: None,
            ..a
        };
        assert_eq!(bare.body(), "Crossed your 75% alert.");
    }
}
