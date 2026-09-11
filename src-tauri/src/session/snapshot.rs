//! The normalized session record every consumer sees, and the surface it lives on.
//!
//! One shape for both channels, so `usage-model` and the panel never learn that a
//! Desktop session arrived a different way from a CLI one. Every field but the id is
//! optional, and absence travels: a consumer hides the element rather than rendering a
//! zero (spec section 10).

use serde::{Deserialize, Serialize};

use super::payload::{Pr, PromptCache, RateLimits, StatusLine};

/// Which Claude Code surface a session lives on.
///
/// Taken from the transcript's `entrypoint` field, never guessed: the statusLine payload
/// carries no marker, and on 2.1.268 both surfaces write their transcripts under
/// `~/.claude/projects/` and both carry a `scratchpad_dir`, so neither separates them
/// (adr.rg.003). `Unknown` is a real answer — the panel shows the word rather than
/// sending the user to the wrong window on a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    Desktop,
    Cli,
    Unknown,
}

impl Surface {
    /// Map a transcript `entrypoint` value. An unrecognised entrypoint is `Unknown`
    /// rather than a default, because a new surface is exactly the thing a guess would
    /// get wrong.
    pub fn from_entrypoint(entrypoint: &str) -> Self {
        match entrypoint {
            "claude-desktop" => Self::Desktop,
            "cli" => Self::Cli,
            _ => Self::Unknown,
        }
    }
}

/// How a snapshot reached us. The panel shows this only to explain an absence — a
/// Desktop session has no quota of its own to show, and saying why beats a blank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// Through the spool, from the statusLine collector. CLI sessions only.
    StatusLine,
    /// Through the JSONL transcript. The only channel that reaches Desktop.
    Transcript,
    /// Seen on both; the statusLine fields win, being fresher and richer.
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub surface: Surface,
    pub origin: Origin,
    /// Unix epoch milliseconds when this was observed, not when the file was touched.
    pub observed_at_ms: u64,

    pub session_name: Option<String>,
    /// Where Claude Code writes this session's transcript. A direct pointer to the file
    /// that knows the surface, which beats scanning for it.
    #[serde(default)]
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub project_dir: Option<String>,
    pub model_name: Option<String>,
    pub model_id: Option<String>,
    pub version: Option<String>,
    pub effort: Option<String>,
    pub fast_mode: Option<bool>,

    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    pub lines_added: Option<u64>,
    pub lines_removed: Option<u64>,

    pub context_used_pct: Option<f64>,
    pub context_window_size: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,

    /// Account-wide, never per-session (adr.rg.007). Present on CLI sessions only.
    pub rate_limits: Option<RateLimits>,
    pub prompt_cache: Option<PromptCache>,
    pub pr: Option<Pr>,
}

impl SessionSnapshot {
    /// Build from a statusLine payload. `surface` is filled in later from the
    /// transcript, so it starts `Unknown` rather than `Cli`: statusLine running is
    /// strong evidence of a CLI session on 2.1.268, but it is evidence about today's
    /// Claude Code, not a fact about the session, and the transcript states the fact.
    pub fn from_status_line(id: String, observed_at_ms: u64, s: &StatusLine) -> Self {
        let cw = s.context_window.as_ref();
        Self {
            session_id: id,
            surface: Surface::Unknown,
            origin: Origin::StatusLine,
            observed_at_ms,
            session_name: s.session_name.clone(),
            transcript_path: s.transcript_path.clone(),
            cwd: s
                .workspace
                .as_ref()
                .and_then(|w| w.current_dir.clone())
                .or_else(|| s.cwd.clone()),
            project_dir: s.workspace.as_ref().and_then(|w| w.project_dir.clone()),
            model_name: s.model.as_ref().and_then(|m| m.display_name.clone()),
            model_id: s.model.as_ref().and_then(|m| m.id.clone()),
            version: s.version.clone(),
            effort: s.effort.as_ref().and_then(|e| e.level.clone()),
            fast_mode: s.fast_mode,
            cost_usd: s.cost.as_ref().and_then(|c| c.total_cost_usd),
            duration_ms: s.cost.as_ref().and_then(|c| c.total_duration_ms),
            lines_added: s.cost.as_ref().and_then(|c| c.total_lines_added),
            lines_removed: s.cost.as_ref().and_then(|c| c.total_lines_removed),
            context_used_pct: cw.and_then(|c| c.used_percentage),
            context_window_size: cw.and_then(|c| c.context_window_size),
            input_tokens: cw.and_then(|c| c.total_input_tokens),
            output_tokens: cw.and_then(|c| c.total_output_tokens),
            rate_limits: s.rate_limits.clone(),
            prompt_cache: s.prompt_cache.clone(),
            pr: s.pr.clone(),
        }
    }

    /// Fold a transcript-derived snapshot into a statusLine one, or the other way round.
    ///
    /// The statusLine record wins field by field because it is fresher and carries more,
    /// but the transcript owns `surface` outright: it is the only channel that knows it.
    /// A field the winner does not have is taken from the loser rather than dropped.
    pub fn merge(status: Option<Self>, transcript: Option<Self>) -> Option<Self> {
        match (status, transcript) {
            (None, None) => None,
            (Some(s), None) => Some(s),
            (None, Some(t)) => Some(t),
            (Some(mut s), Some(t)) => {
                s.surface = t.surface;
                s.origin = Origin::Both;
                s.observed_at_ms = s.observed_at_ms.max(t.observed_at_ms);
                s.session_name = s.session_name.or(t.session_name);
                s.transcript_path = s.transcript_path.or(t.transcript_path);
                s.cwd = s.cwd.or(t.cwd);
                s.project_dir = s.project_dir.or(t.project_dir);
                s.model_name = s.model_name.or(t.model_name);
                s.model_id = s.model_id.or(t.model_id);
                s.version = s.version.or(t.version);
                s.input_tokens = s.input_tokens.or(t.input_tokens);
                s.output_tokens = s.output_tokens.or(t.output_tokens);
                s.context_used_pct = s.context_used_pct.or(t.context_used_pct);
                s.context_window_size = s.context_window_size.or(t.context_window_size);
                Some(s)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: &str, surface: Surface, origin: Origin, at: u64) -> SessionSnapshot {
        SessionSnapshot {
            session_id: id.into(),
            surface,
            origin,
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

    #[test]
    fn entrypoint_maps_to_surface_and_anything_new_is_unknown() {
        assert_eq!(Surface::from_entrypoint("claude-desktop"), Surface::Desktop);
        assert_eq!(Surface::from_entrypoint("cli"), Surface::Cli);
        assert_eq!(Surface::from_entrypoint("sdk"), Surface::Unknown);
        assert_eq!(Surface::from_entrypoint(""), Surface::Unknown);
    }

    #[test]
    fn a_status_line_snapshot_keeps_every_absence() {
        let s = SessionSnapshot::from_status_line(
            "a".into(),
            10,
            &serde_json::from_str(r#"{"cwd":"/p","model":{"display_name":"Opus 5"}}"#).unwrap(),
        );
        assert_eq!(s.cwd.as_deref(), Some("/p"));
        assert_eq!(s.model_name.as_deref(), Some("Opus 5"));
        assert!(
            s.cost_usd.is_none(),
            "an unstarted session has no cost, not a zero"
        );
        assert!(s.rate_limits.is_none());
        assert_eq!(s.surface, Surface::Unknown, "the transcript decides this");
    }

    #[test]
    fn merging_takes_the_surface_from_the_transcript_and_the_rest_from_the_status_line() {
        let mut status = snap("a", Surface::Unknown, Origin::StatusLine, 100);
        status.model_name = Some("Opus 5".into());
        let mut transcript = snap("a", Surface::Cli, Origin::Transcript, 90);
        transcript.model_name = Some("stale".into());
        transcript.cwd = Some("/p".into());

        let m = SessionSnapshot::merge(Some(status), Some(transcript)).unwrap();
        assert_eq!(m.surface, Surface::Cli, "only the transcript knows this");
        assert_eq!(
            m.model_name.as_deref(),
            Some("Opus 5"),
            "statusLine is fresher"
        );
        assert_eq!(
            m.cwd.as_deref(),
            Some("/p"),
            "and gaps are filled, not dropped"
        );
        assert_eq!(m.origin, Origin::Both);
        assert_eq!(m.observed_at_ms, 100);
    }

    #[test]
    fn a_session_on_one_channel_only_passes_through() {
        let only_transcript = snap("d", Surface::Desktop, Origin::Transcript, 5);
        let m = SessionSnapshot::merge(None, Some(only_transcript)).unwrap();
        assert_eq!(m.surface, Surface::Desktop);
        assert_eq!(m.origin, Origin::Transcript);
        assert!(SessionSnapshot::merge(None, None).is_none());
    }
}
