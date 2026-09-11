//! The shape Claude Code puts on a statusLine command's stdin, and the record the
//! collector writes into the spool.
//!
//! Every field is optional, always. Claude Code adds and removes fields between
//! versions, `rate_limits` appears only on a Pro/Max account and only after the first
//! API response of a session, `prompt_cache` needs 2.1.251+, `pr` needs a git repo, and
//! `context_window.current_usage` is null before the first call and again after
//! `/compact`. Nothing here may be defaulted to zero: a zero is a claim, and absence is
//! the information (spec section 5.1).
//!
//! **Optional is not enough on its own.** A field that disappears deserialises to `None`,
//! but a field that changes *type* would fail the whole parse and take every other field
//! with it — which is exactly what happened on 2.1.268, where `current_usage` turned out
//! to be an object of token counts rather than the number the spec describes. So every
//! field here goes through [`lenient`]: anything that does not fit becomes `None` and the
//! rest of the payload survives. One field changing shape may cost that field, never the
//! panel.

use serde::{Deserialize, Deserializer, Serialize};

/// Deserialize a field into `Some(T)`, or into `None` if it is absent, null, or of a
/// shape `T` cannot accept. The type mismatch is swallowed on purpose: a status line is
/// a display, and no display is worth failing a parse over.
fn lenient<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).ok())
}

/// What the collector writes to `spool/sessions/<session_id>.json`.
///
/// The payload is stored verbatim, uninterpreted. The collector runs on every assistant
/// message and must finish well inside the debounce window, so it does no work the app
/// can do later; and storing the raw object means a field the app does not read yet is
/// still there when it learns to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpoolRecord {
    /// Unix epoch milliseconds at which the collector saw this payload. The app cannot
    /// use the file mtime instead: burn rate is computed from the interval between
    /// observations, and mtime granularity is coarse on some filesystems.
    pub observed_at_ms: u64,
    pub status: StatusLine,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusLine {
    #[serde(default, deserialize_with = "lenient")]
    pub session_id: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub session_name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub transcript_path: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub cwd: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub version: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub model: Option<Model>,
    #[serde(default, deserialize_with = "lenient")]
    pub workspace: Option<Workspace>,
    #[serde(default, deserialize_with = "lenient")]
    pub cost: Option<Cost>,
    #[serde(default, deserialize_with = "lenient")]
    pub context_window: Option<ContextWindow>,
    #[serde(default, deserialize_with = "lenient")]
    pub rate_limits: Option<RateLimits>,
    #[serde(default, deserialize_with = "lenient")]
    pub prompt_cache: Option<PromptCache>,
    #[serde(default, deserialize_with = "lenient")]
    pub pr: Option<Pr>,
    #[serde(default, deserialize_with = "lenient")]
    pub effort: Option<Effort>,
    #[serde(default, deserialize_with = "lenient")]
    pub fast_mode: Option<bool>,
    #[serde(default, deserialize_with = "lenient")]
    pub exceeds_200k_tokens: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Model {
    #[serde(default, deserialize_with = "lenient")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(default, deserialize_with = "lenient")]
    pub current_dir: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub project_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cost {
    #[serde(default, deserialize_with = "lenient")]
    pub total_cost_usd: Option<f64>,
    #[serde(default, deserialize_with = "lenient")]
    pub total_duration_ms: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub total_api_duration_ms: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub total_lines_added: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub total_lines_removed: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextWindow {
    #[serde(default, deserialize_with = "lenient")]
    pub total_input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub total_output_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub context_window_size: Option<u64>,
    /// Measured on 2.1.268: an object of token counts, not the number spec section 5.1
    /// describes, and null before the first API call and again after `/compact`.
    #[serde(default, deserialize_with = "lenient")]
    pub current_usage: Option<CurrentUsage>,
    #[serde(default, deserialize_with = "lenient")]
    pub used_percentage: Option<f64>,
    #[serde(default, deserialize_with = "lenient")]
    pub remaining_percentage: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CurrentUsage {
    #[serde(default, deserialize_with = "lenient")]
    pub input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub output_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub cache_creation_input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub cache_read_input_tokens: Option<u64>,
}

/// Account-wide, identical in every concurrent session. Never a per-session figure
/// (adr.rg.007), and reachable only through CLI sessions (adr.rg.009).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RateLimits {
    #[serde(default, deserialize_with = "lenient")]
    pub five_hour: Option<Window>,
    #[serde(default, deserialize_with = "lenient")]
    pub seven_day: Option<Window>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Window {
    #[serde(default, deserialize_with = "lenient")]
    pub used_percentage: Option<f64>,
    /// Unix epoch seconds. Crossing it resets the window.
    #[serde(default, deserialize_with = "lenient")]
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptCache {
    #[serde(default, deserialize_with = "lenient")]
    pub warm: Option<bool>,
    #[serde(default, deserialize_with = "lenient")]
    pub caching_observed: Option<bool>,
    #[serde(default, deserialize_with = "lenient")]
    pub ttl: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub expires_at: Option<i64>,
    #[serde(default, deserialize_with = "lenient")]
    pub requests: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub misses: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub hit_ratio: Option<f64>,
    #[serde(default, deserialize_with = "lenient")]
    pub cache_write_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub last_miss_cause: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pr {
    #[serde(default, deserialize_with = "lenient")]
    pub number: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub url: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub review_state: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Effort {
    #[serde(default, deserialize_with = "lenient")]
    pub level: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from the P0 spike on Claude Code 2.1.268, trimmed but otherwise
    /// verbatim: a session that has made API calls, so it carries rate_limits,
    /// prompt_cache and the object-shaped current_usage.
    const REAL: &str = r#"{
      "session_id":"b99e7a1f-b52d-42f8-b3db-7858547ccca7",
      "transcript_path":"C:\\Users\\Koti\\.claude\\projects\\E--p\\b99e7a1f.jsonl",
      "cwd":"E:\\projects\\sunrisesoftware\\masterpromo",
      "scratchpad_dir":"C:\\Users\\Koti\\AppData\\Local\\Temp\\claude\\E--p\\b99e7a1f\\scratchpad",
      "prompt_id":"p_01","effort":{"level":"high"},
      "session_name":"masterpromo",
      "model":{"id":"claude-opus-5[1m]","display_name":"Opus 5 (1M context)"},
      "workspace":{"current_dir":"E:\\projects\\sunrisesoftware\\masterpromo",
                   "project_dir":"E:\\projects\\sunrisesoftware\\masterpromo","added_dirs":[]},
      "version":"2.1.268","output_style":{"name":"default"},
      "cost":{"total_cost_usd":0.1778,"total_duration_ms":90210,"total_api_duration_ms":28650,
              "total_lines_added":12,"total_lines_removed":3},
      "context_window":{"total_input_tokens":2,"total_output_tokens":1422,
                        "context_window_size":1000000,
                        "current_usage":{"input_tokens":2,"output_tokens":1422,
                                         "cache_creation_input_tokens":36101,
                                         "cache_read_input_tokens":0},
                        "used_percentage":5,"remaining_percentage":95},
      "exceeds_200k_tokens":false,
      "prompt_cache":{"warm":true,"caching_observed":true,"ttl":"1h","expires_at":1789146747,
                      "requests":7,"misses":0,"expected_rebuilds":0,
                      "hit_ratio":0.9186047297830285,"cache_write_tokens":36101,
                      "miss_recache_tokens":0,"last_miss_at":null,"last_miss_cause":null,
                      "miss_causes":{},"recache_tokens_if_cold":71634},
      "fast_mode":false,"thinking":{"enabled":true},
      "rate_limits":{"five_hour":{"used_percentage":7.0,"resets_at":1789147800},
                     "seven_day":{"used_percentage":66,"resets_at":1789347600}}
    }"#;

    /// The first call of a session: no rate_limits, no prompt_cache, no session_name,
    /// and a null current_usage.
    const FIRST_CALL: &str = r#"{
      "session_id":"02562ffb","cwd":"C:\\Users\\Koti","version":"2.1.268",
      "model":{"id":"claude-opus-5[1m]","display_name":"Opus 5 (1M context)"},
      "cost":{"total_cost_usd":0,"total_duration_ms":58463,"total_api_duration_ms":0},
      "context_window":{"total_input_tokens":0,"total_output_tokens":0,
                        "context_window_size":1000000,"current_usage":null,
                        "used_percentage":null,"remaining_percentage":null},
      "exceeds_200k_tokens":false,"fast_mode":false,"thinking":{"enabled":true}
    }"#;

    #[test]
    fn the_real_payload_parses_whole() {
        let s: StatusLine = serde_json::from_str(REAL).unwrap();
        assert_eq!(s.session_name.as_deref(), Some("masterpromo"));
        assert_eq!(s.version.as_deref(), Some("2.1.268"));
        let cw = s.context_window.unwrap();
        assert_eq!(cw.used_percentage, Some(5.0));
        // current_usage is an object, not a number (measured, against spec section 5.1).
        assert_eq!(
            cw.current_usage.unwrap().cache_creation_input_tokens,
            Some(36101)
        );
        let rl = s.rate_limits.unwrap();
        assert_eq!(rl.five_hour.unwrap().used_percentage, Some(7.0));
        assert_eq!(rl.seven_day.unwrap().resets_at, Some(1789347600));
        let hit = s.prompt_cache.unwrap().hit_ratio.unwrap();
        assert!(
            (hit - 0.918_604_729_783).abs() < 1e-9,
            "hit ratio was {hit}"
        );
    }

    #[test]
    fn first_call_parses_with_its_absences_intact() {
        let s: StatusLine = serde_json::from_str(FIRST_CALL).unwrap();
        // The absences are the point: none of these may become a zero.
        assert!(s.rate_limits.is_none());
        assert!(s.prompt_cache.is_none());
        assert!(s.session_name.is_none());
        assert!(s.pr.is_none());
        assert!(s.context_window.unwrap().current_usage.is_none());
    }

    #[test]
    fn a_field_of_the_wrong_type_costs_only_that_field() {
        // This is the regression that motivated `lenient`: on 2.1.268 current_usage
        // turned out to be an object where a number was expected, and a strict Option
        // failed the entire payload — so the panel lost the session, the model, the cost
        // and the quota over one field it did not even need.
        let s: StatusLine = serde_json::from_str(
            r#"{"session_id":"a","cwd":"/p",
                "context_window":{"current_usage":"surprise","used_percentage":5},
                "cost":{"total_cost_usd":"1.50"},
                "rate_limits":{"five_hour":{"used_percentage":7.0}}}"#,
        )
        .unwrap();
        assert_eq!(s.session_id.as_deref(), Some("a"));
        let cw = s.context_window.unwrap();
        assert!(cw.current_usage.is_none(), "the odd field drops out");
        assert_eq!(cw.used_percentage, Some(5.0), "its neighbour survives");
        assert!(s.cost.unwrap().total_cost_usd.is_none());
        assert_eq!(
            s.rate_limits.unwrap().five_hour.unwrap().used_percentage,
            Some(7.0),
            "and so does the rest of the payload"
        );
    }

    #[test]
    fn a_whole_subobject_of_the_wrong_type_drops_out_alone() {
        let s: StatusLine =
            serde_json::from_str(r#"{"session_id":"a","model":"opus","cwd":"/p"}"#).unwrap();
        assert!(s.model.is_none());
        assert_eq!(s.cwd.as_deref(), Some("/p"));
    }

    #[test]
    fn unknown_fields_and_an_empty_object_are_both_fine() {
        let s: StatusLine =
            serde_json::from_str(r#"{"session_id":"a","some_future_field":{"x":1}}"#).unwrap();
        assert_eq!(s.session_id.as_deref(), Some("a"));
        let empty: StatusLine = serde_json::from_str("{}").unwrap();
        assert!(empty.session_id.is_none());
    }

    #[test]
    fn a_spool_record_round_trips() {
        let rec = SpoolRecord {
            observed_at_ms: 1_789_148_000_000,
            status: serde_json::from_str(REAL).unwrap(),
        };
        let bytes = serde_json::to_vec(&rec).unwrap();
        let back: SpoolRecord = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.observed_at_ms, rec.observed_at_ms);
        assert_eq!(back.status.cwd, rec.status.cwd);
        assert_eq!(
            back.status
                .rate_limits
                .unwrap()
                .five_hour
                .unwrap()
                .used_percentage,
            Some(7.0)
        );
    }
}
