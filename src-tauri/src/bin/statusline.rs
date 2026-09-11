//! ReviewGlass statusLine collector (rg.statusline-collector).
//!
//! Claude Code runs this with its status-line JSON on stdin. It writes the payload into
//! the spool and prints a status line. Both halves matter: `statusLine` is a
//! single-value setting, so ReviewGlass takes over the user's status line entirely, and
//! a run that prints nothing leaves the line blank.
//!
//! Three rules, in order of importance:
//!   1. Always print a line. Even with no input, no spool, and no parse.
//!   2. Always exit 0. A non-zero exit is a visible fault in someone's session for the
//!      sake of a display-only feature.
//!   3. Be fast. It runs on every assistant message, inside a 300 ms debounce.
//!
//! It is a binary rather than a shell script on purpose. The spec describes it as a
//! script, but on Windows Claude Code runs status-line commands through Git Bash when
//! installed and PowerShell otherwise, and Git Bash eats unquoted backslashes in
//! Windows paths — a fragility the spec itself warns about. A native binary invoked
//! directly has no shell in the path at all, and starts in single-digit milliseconds.

use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use reviewglass_lib::session::payload::{SpoolRecord, StatusLine};
use reviewglass_lib::spool;

fn main() {
    let mut raw = String::new();
    // A read failure is not fatal: it means an empty payload and a minimal line.
    let _ = io::stdin().read_to_string(&mut raw);

    // A payload that will not parse is not a reason to fail a user's session, so the
    // error is swallowed — but it is reachable, because a silent collector that has
    // stopped understanding Claude Code is exactly the failure nobody would notice.
    let status: StatusLine = match serde_json::from_str(&raw) {
        Ok(s) => s,
        Err(e) => {
            if std::env::var_os("REVIEWGLASS_DEBUG").is_some() {
                eprintln!("reviewglass-statusline: could not parse stdin: {e}");
            }
            StatusLine::default()
        }
    };
    let line = render(&status);

    // Print before storing. If the write below somehow blocks or fails, the user still
    // has a status line.
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();

    store(&status);
}

/// The line the user reads. Deliberately plain: it stands in for whatever they had, so
/// it shows the things a status line is for and nothing ReviewGlass is proud of.
fn render(s: &StatusLine) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(4);

    if let Some(dir) = s
        .workspace
        .as_ref()
        .and_then(|w| w.current_dir.as_deref())
        .or(s.cwd.as_deref())
    {
        let leaf = dir
            .rsplit(['/', '\\'])
            .find(|p| !p.is_empty())
            .unwrap_or(dir);
        parts.push(leaf.to_string());
    }
    if let Some(model) = s.model.as_ref().and_then(|m| m.display_name.as_deref()) {
        parts.push(model.to_string());
    }
    // Context fill is null before the first API call and again after /compact. Absent
    // means absent: no "0%" is printed for a session that has simply not started.
    if let Some(pct) = s.context_window.as_ref().and_then(|c| c.used_percentage) {
        parts.push(format!("ctx {}%", pct.round() as i64));
    }
    // The account-wide five-hour window, when this session has seen one. It is the same
    // number in every session; it is shown here because the status line is where the
    // user already looks, not as a per-session figure.
    if let Some(pct) = s
        .rate_limits
        .as_ref()
        .and_then(|r| r.five_hour.as_ref())
        .and_then(|w| w.used_percentage)
    {
        parts.push(format!("5h {}%", pct.round() as i64));
    }

    if parts.is_empty() {
        "ReviewGlass".to_string()
    } else {
        parts.join(" · ")
    }
}

/// Write the record into the spool, atomically, under the session id. Any failure is
/// silent: the panel simply does not see this session, which is a smaller harm than a
/// broken status line.
fn store(status: &StatusLine) {
    let Some(id) = status.session_id.as_deref().and_then(spool::safe_file_stem) else {
        return;
    };
    let Some(dir) = spool::sessions_dir() else {
        return;
    };
    let observed_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let record = SpoolRecord {
        observed_at_ms,
        status: status.clone(),
    };
    if let Ok(bytes) = serde_json::to_vec(&record) {
        let _ = spool::write_atomic(&dir.join(format!("{id}.json")), &bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> StatusLine {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn renders_something_from_nothing() {
        assert_eq!(render(&StatusLine::default()), "ReviewGlass");
        assert_eq!(render(&parse("{}")), "ReviewGlass");
    }

    #[test]
    fn renders_only_what_is_present() {
        // No context fill and no rate limits yet: neither is invented as a zero.
        let s = parse(
            r#"{"workspace":{"current_dir":"E:\\projects\\sunrisesoftware\\reviewglass"},
                "model":{"display_name":"Opus 5"},
                "context_window":{"used_percentage":null}}"#,
        );
        assert_eq!(render(&s), "reviewglass · Opus 5");
    }

    #[test]
    fn renders_the_full_line_once_the_session_has_called() {
        let s = parse(
            r#"{"cwd":"/home/k/proj","model":{"display_name":"Opus 5"},
                "context_window":{"used_percentage":5.4},
                "rate_limits":{"five_hour":{"used_percentage":7.0}}}"#,
        );
        assert_eq!(render(&s), "proj · Opus 5 · ctx 5% · 5h 7%");
    }

    #[test]
    fn a_trailing_separator_never_appears() {
        let s = parse(r#"{"cwd":"/home/k/proj/"}"#);
        assert_eq!(render(&s), "proj");
    }

    #[test]
    fn storing_without_a_session_id_is_a_no_op() {
        // No panic, no write, no complaint.
        store(&StatusLine::default());
        store(&parse(r#"{"session_id":".."}"#));
    }
}
