//! Usage model (rg.usage-model).
//!
//! Two quantities that are never blended (adr.rg.007):
//!
//! * **`AccountQuota`** — one shared gauge. `rate_limits` is account-wide and identical
//!   in every concurrent session, so it is read once for the whole account, not per
//!   session. It arrives only through statusLine, which runs only in the CLI, so with no
//!   CLI session live the gauge has no source at all and is absent with its reason named
//!   (adr.rg.009).
//! * **`SessionAttribution`** — N relative shares. Derived from what each session has
//!   spent, in a unit the quota does not use. It answers "which session is consuming
//!   most" in relative terms and nothing more.
//!
//! Burn rate is a linear projection over consecutive `(used_percentage, observed_at)`
//! pairs and is labelled an estimate wherever it is shown. Samples from before a
//! `resets_at` boundary are discarded rather than averaged across it: a window that
//! reset from 94 % to 3 % has not "fallen at 91 points per hour".

use std::collections::HashMap;

use serde::Serialize;

use crate::session::{payload::RateLimits, Origin, Reading, SessionSnapshot, Surface};

/// At least two samples are needed for a rate, and they must be far enough apart that
/// the interval is not mostly noise.
const MIN_SAMPLE_GAP_MS: u64 = 20_000;
/// History older than this is dropped: a five-hour window's recent slope is what matters.
const HISTORY_MS: u64 = 90 * 60 * 1000;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Sample {
    pub used_percentage: f64,
    pub observed_at_ms: u64,
}

/// Why the gauge has nothing to show. The three are not interchangeable: only the first
/// is something the user can act on, so they must never share a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuotaAbsence {
    /// No CLI session is live. statusLine runs only there, so nothing is reporting.
    NoCliSession,
    /// A CLI session is live but has not reported rate_limits: either it has made no API
    /// call yet, or the account is not Pro/Max.
    NotReportedYet,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuotaWindow {
    pub used_percentage: f64,
    pub resets_at: Option<i64>,
    /// Percentage points per hour, from consecutive samples. `None` until there are two
    /// far enough apart — a rate from one point is not an estimate, it is a fabrication.
    pub burn_per_hour: Option<f64>,
    /// Hours until 100 % at the current rate. An estimate, and labelled as one wherever
    /// it is shown.
    pub hours_to_limit: Option<f64>,
    /// True when `resets_at` has passed with no fresher sample: the figure is the last
    /// one seen, not the current one.
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountQuota {
    pub five_hour: Option<QuotaWindow>,
    pub seven_day: Option<QuotaWindow>,
    /// Set when both windows are absent; says which absence this is.
    pub absence: Option<QuotaAbsence>,
    /// The session the figures came from, so the panel can say the gauge is borrowed.
    pub source_session: Option<String>,
}

/// Where a session's share was derived from. CLI shares come from statusLine cost, which
/// weights by model; Desktop shares come from transcript activity, which does not. They
/// are not comparable unit for unit, so each carries its basis and the panel says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttributionBasis {
    /// Cumulative `cost.total_cost_usd` from statusLine.
    Cost,
    /// Token counts read from the session's own statusLine record.
    Tokens,
    /// Nothing to derive a share from — a Desktop session with no statusLine and no
    /// token counts of its own. Shown as a session, not as a share.
    None,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionAttribution {
    pub session_id: String,
    pub session_name: Option<String>,
    pub surface: Surface,
    pub origin: Origin,
    pub cwd: Option<String>,
    pub model_name: Option<String>,
    pub context_used_pct: Option<f64>,
    pub cost_usd: Option<f64>,
    pub lines_added: Option<u64>,
    pub lines_removed: Option<u64>,
    pub basis: AttributionBasis,
    /// Share of the measurable total, 0-100. Relative only, never a share of the quota.
    pub share_pct: Option<f64>,
    pub observed_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageView {
    pub quota: AccountQuota,
    pub sessions: Vec<SessionAttribution>,
    pub collector_installed: bool,
    pub unreadable: usize,
    /// True when shares come from more than one basis, so the panel can say the ranking
    /// is only meaningful within a basis.
    pub mixed_basis: bool,
}

/// Keeps the sample history the gauge's slope is computed from.
#[derive(Debug, Default)]
pub struct UsageModel {
    five_hour: Vec<Sample>,
    seven_day: Vec<Sample>,
    five_hour_resets_at: Option<i64>,
    seven_day_resets_at: Option<i64>,
}

impl UsageModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one reading of both channels into the view the panel renders.
    pub fn observe(&mut self, reading: &Reading) -> UsageView {
        let quota = self.observe_quota(&reading.sessions);
        let sessions = attribute(&reading.sessions);
        let mut bases: Vec<AttributionBasis> = sessions
            .iter()
            .map(|s| s.basis)
            .filter(|b| *b != AttributionBasis::None)
            .collect();
        bases.dedup();
        let mixed_basis = bases.windows(2).any(|w| w[0] != w[1]);

        UsageView {
            quota,
            sessions,
            collector_installed: reading.collector_installed,
            unreadable: reading.unreadable,
            mixed_basis,
        }
    }

    fn observe_quota(&mut self, sessions: &[SessionSnapshot]) -> AccountQuota {
        // rate_limits is account-wide, so any session carrying it speaks for the account.
        // The freshest wins; sessions are already newest-first.
        let source = sessions
            .iter()
            .find(|s| has_any_window(s.rate_limits.as_ref()));

        let Some(source) = source else {
            let any_cli = sessions
                .iter()
                .any(|s| s.surface == Surface::Cli || s.origin != Origin::Transcript);
            return AccountQuota {
                five_hour: None,
                seven_day: None,
                absence: Some(if any_cli {
                    QuotaAbsence::NotReportedYet
                } else {
                    QuotaAbsence::NoCliSession
                }),
                source_session: None,
            };
        };

        let rl = source.rate_limits.as_ref().expect("checked above");
        let at = source.observed_at_ms;
        let five_hour = rl.five_hour.and_then(|w| {
            window(
                &mut self.five_hour,
                &mut self.five_hour_resets_at,
                w.used_percentage,
                w.resets_at,
                at,
            )
        });
        let seven_day = rl.seven_day.and_then(|w| {
            window(
                &mut self.seven_day,
                &mut self.seven_day_resets_at,
                w.used_percentage,
                w.resets_at,
                at,
            )
        });

        AccountQuota {
            absence: if five_hour.is_none() && seven_day.is_none() {
                Some(QuotaAbsence::NotReportedYet)
            } else {
                None
            },
            five_hour,
            seven_day,
            source_session: Some(source.session_id.clone()),
        }
    }
}

fn has_any_window(rl: Option<&RateLimits>) -> bool {
    rl.is_some_and(|r| {
        r.five_hour.is_some_and(|w| w.used_percentage.is_some())
            || r.seven_day.is_some_and(|w| w.used_percentage.is_some())
    })
}

/// Fold one observation into a window's history and derive its rate.
fn window(
    history: &mut Vec<Sample>,
    known_reset: &mut Option<i64>,
    used: Option<f64>,
    resets_at: Option<i64>,
    observed_at_ms: u64,
) -> Option<QuotaWindow> {
    let used = used?;

    // Crossing resets_at starts a new window: the old samples describe a quota that no
    // longer exists, and averaging across the boundary invents a fall that never
    // happened.
    if let (Some(new), Some(old)) = (resets_at, *known_reset) {
        if new != old {
            history.clear();
        }
    }
    if resets_at.is_some() {
        *known_reset = resets_at;
    }

    let is_new = history
        .last()
        .is_none_or(|last| last.observed_at_ms != observed_at_ms);
    if is_new {
        history.push(Sample {
            used_percentage: used,
            observed_at_ms,
        });
    }
    let cutoff = observed_at_ms.saturating_sub(HISTORY_MS);
    history.retain(|s| s.observed_at_ms >= cutoff);

    let burn_per_hour = burn_rate(history);
    let hours_to_limit = burn_per_hour.and_then(|rate| {
        if rate > 0.0 && used < 100.0 {
            Some((100.0 - used) / rate)
        } else {
            None
        }
    });

    let stale = resets_at.is_some_and(|r| (r as u64) * 1000 < observed_at_ms);

    Some(QuotaWindow {
        used_percentage: used,
        resets_at,
        burn_per_hour,
        hours_to_limit,
        stale,
    })
}

/// Percentage points per hour across the retained history. `None` with fewer than two
/// samples, or with the samples too close together to say anything.
fn burn_rate(history: &[Sample]) -> Option<f64> {
    let first = history.first()?;
    let last = history.last()?;
    let span_ms = last.observed_at_ms.checked_sub(first.observed_at_ms)?;
    if span_ms < MIN_SAMPLE_GAP_MS {
        return None;
    }
    let delta = last.used_percentage - first.used_percentage;
    if delta <= 0.0 {
        // Flat or falling. A falling percentage inside one window means a reset we did
        // not see; either way there is no rate toward a limit to report.
        return None;
    }
    Some(delta / (span_ms as f64 / 3_600_000.0))
}

/// Relative shares over the sessions that have something to measure.
///
/// Never a share of the quota: the total here is the sum of what the sessions themselves
/// report, in their own unit. A session with nothing measurable keeps its row and gets no
/// share, because dropping it would hide a running session.
fn attribute(sessions: &[SessionSnapshot]) -> Vec<SessionAttribution> {
    let mut basis_of: HashMap<&str, AttributionBasis> = HashMap::new();
    let mut cost_total = 0.0_f64;
    let mut token_total = 0.0_f64;

    for s in sessions {
        let basis = if s.cost_usd.is_some_and(|c| c > 0.0) {
            cost_total += s.cost_usd.unwrap_or(0.0);
            AttributionBasis::Cost
        } else if let Some(t) = total_tokens(s) {
            token_total += t;
            AttributionBasis::Tokens
        } else {
            AttributionBasis::None
        };
        basis_of.insert(s.session_id.as_str(), basis);
    }

    sessions
        .iter()
        .map(|s| {
            let basis = basis_of
                .get(s.session_id.as_str())
                .copied()
                .unwrap_or(AttributionBasis::None);
            let share_pct = match basis {
                AttributionBasis::Cost if cost_total > 0.0 => {
                    s.cost_usd.map(|c| c / cost_total * 100.0)
                }
                AttributionBasis::Tokens if token_total > 0.0 => {
                    total_tokens(s).map(|t| t / token_total * 100.0)
                }
                _ => None,
            };
            SessionAttribution {
                session_id: s.session_id.clone(),
                session_name: s.session_name.clone(),
                surface: s.surface,
                origin: s.origin,
                cwd: s.cwd.clone(),
                model_name: s.model_name.clone(),
                context_used_pct: s.context_used_pct,
                cost_usd: s.cost_usd,
                lines_added: s.lines_added,
                lines_removed: s.lines_removed,
                basis,
                share_pct,
                observed_at_ms: s.observed_at_ms,
            }
        })
        .collect()
}

fn total_tokens(s: &SessionSnapshot) -> Option<f64> {
    let i = s.input_tokens.unwrap_or(0);
    let o = s.output_tokens.unwrap_or(0);
    if i == 0 && o == 0 {
        None
    } else {
        Some((i + o) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::snapshot::{Origin, Surface};

    fn snap(id: &str, at: u64) -> SessionSnapshot {
        SessionSnapshot {
            session_id: id.into(),
            surface: Surface::Cli,
            origin: Origin::StatusLine,
            observed_at_ms: at,
            session_name: None,
            transcript_path: None,
            cwd: None,
            project_dir: None,
            model_name: None,
            model_id: None,
            version: None,
            effort: None,
            fast_mode: None,
            cost_usd: None,
            duration_ms: None,
            lines_added: None,
            lines_removed: None,
            context_used_pct: None,
            context_window_size: None,
            input_tokens: None,
            output_tokens: None,
            rate_limits: None,
            prompt_cache: None,
            pr: None,
        }
    }

    fn with_quota(mut s: SessionSnapshot, five: f64, resets_at: i64) -> SessionSnapshot {
        s.rate_limits = Some(
            serde_json::from_value(serde_json::json!({
                "five_hour": {"used_percentage": five, "resets_at": resets_at}
            }))
            .unwrap(),
        );
        s
    }

    fn reading(sessions: Vec<SessionSnapshot>) -> Reading {
        Reading {
            sessions,
            collector_installed: true,
            unreadable: 0,
        }
    }

    #[test]
    fn the_shared_gauge_is_never_a_per_session_figure() {
        // Three sessions all report the same account-wide number; the model must read it
        // once, not three times, and must not attach it to a session as that session's.
        let mut m = UsageModel::new();
        let v = m.observe(&reading(vec![
            with_quota(snap("a", 1000), 7.0, 9_999),
            with_quota(snap("b", 900), 7.0, 9_999),
            snap("c", 800),
        ]));
        assert_eq!(v.quota.five_hour.as_ref().unwrap().used_percentage, 7.0);
        assert_eq!(v.quota.source_session.as_deref(), Some("a"));
        assert_eq!(v.sessions.len(), 3);
        // No attribution row carries a quota percentage at all.
        assert!(v.sessions.iter().all(|s| s.share_pct.is_none()));
    }

    #[test]
    fn no_cli_session_is_a_different_absence_from_no_report() {
        let mut m = UsageModel::new();
        let mut desktop = snap("d", 1000);
        desktop.surface = Surface::Desktop;
        desktop.origin = Origin::Transcript;
        let v = m.observe(&reading(vec![desktop]));
        assert!(v.quota.five_hour.is_none());
        assert_eq!(v.quota.absence, Some(QuotaAbsence::NoCliSession));

        let mut m2 = UsageModel::new();
        let v2 = m2.observe(&reading(vec![snap("c", 1000)]));
        assert_eq!(v2.quota.absence, Some(QuotaAbsence::NotReportedYet));
    }

    #[test]
    fn one_sample_yields_no_rate() {
        let mut m = UsageModel::new();
        let v = m.observe(&reading(vec![with_quota(
            snap("a", 1_000_000),
            10.0,
            9_999,
        )]));
        let w = v.quota.five_hour.unwrap();
        assert_eq!(w.used_percentage, 10.0);
        assert!(
            w.burn_per_hour.is_none(),
            "a rate from one point is a fabrication"
        );
        assert!(w.hours_to_limit.is_none());
    }

    #[test]
    fn two_samples_an_hour_apart_give_a_rate_and_a_projection() {
        let mut m = UsageModel::new();
        m.observe(&reading(vec![with_quota(snap("a", 0), 10.0, 9_999)]));
        let v = m.observe(&reading(vec![with_quota(
            snap("a", 3_600_000),
            40.0,
            9_999,
        )]));
        let w = v.quota.five_hour.unwrap();
        assert!((w.burn_per_hour.unwrap() - 30.0).abs() < 0.001);
        // 60 points left at 30/h.
        assert!((w.hours_to_limit.unwrap() - 2.0).abs() < 0.001);
    }

    #[test]
    fn a_reset_discards_the_samples_from_before_it() {
        let mut m = UsageModel::new();
        m.observe(&reading(vec![with_quota(snap("a", 0), 94.0, 1_000)]));
        // New window: resets_at changed and the percentage fell.
        let v = m.observe(&reading(vec![with_quota(snap("a", 3_600_000), 3.0, 2_000)]));
        let w = v.quota.five_hour.unwrap();
        assert_eq!(w.used_percentage, 3.0);
        assert!(
            w.burn_per_hour.is_none(),
            "94 to 3 is a reset, not a fall of 91 points per hour"
        );
    }

    #[test]
    fn shares_are_relative_and_sum_to_a_hundred() {
        let mut m = UsageModel::new();
        let mut a = snap("a", 1000);
        a.cost_usd = Some(3.0);
        let mut b = snap("b", 900);
        b.cost_usd = Some(1.0);
        let v = m.observe(&reading(vec![a, b]));
        let shares: Vec<f64> = v.sessions.iter().filter_map(|s| s.share_pct).collect();
        assert_eq!(shares.len(), 2);
        assert!((shares.iter().sum::<f64>() - 100.0).abs() < 0.001);
        assert!((shares[0] - 75.0).abs() < 0.001);
        assert!(v.sessions.iter().all(|s| s.basis == AttributionBasis::Cost));
    }

    #[test]
    fn a_session_with_nothing_to_measure_keeps_its_row() {
        let mut m = UsageModel::new();
        let mut a = snap("a", 1000);
        a.cost_usd = Some(2.0);
        let mut d = snap("d", 900);
        d.surface = Surface::Desktop;
        d.origin = Origin::Transcript;
        let v = m.observe(&reading(vec![a, d]));
        assert_eq!(v.sessions.len(), 2, "a running session is never dropped");
        let desktop = v.sessions.iter().find(|s| s.session_id == "d").unwrap();
        assert_eq!(desktop.basis, AttributionBasis::None);
        assert!(desktop.share_pct.is_none());
    }

    #[test]
    fn mixed_bases_are_flagged_rather_than_blended() {
        let mut m = UsageModel::new();
        let mut a = snap("a", 1000);
        a.cost_usd = Some(2.0);
        let mut b = snap("b", 900);
        b.input_tokens = Some(1000);
        b.output_tokens = Some(500);
        let v = m.observe(&reading(vec![a, b]));
        assert!(v.mixed_basis, "cost and tokens are not the same unit");
        let bases: Vec<_> = v.sessions.iter().map(|s| s.basis).collect();
        assert!(bases.contains(&AttributionBasis::Cost));
        assert!(bases.contains(&AttributionBasis::Tokens));
    }

    #[test]
    fn an_expired_window_is_marked_stale() {
        let mut m = UsageModel::new();
        // resets_at in the past relative to the observation.
        let v = m.observe(&reading(vec![with_quota(
            snap("a", 10_000_000),
            50.0,
            1_000,
        )]));
        assert!(v.quota.five_hour.unwrap().stale);
    }
}
