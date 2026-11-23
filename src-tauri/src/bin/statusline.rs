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
//!
//! A status line of the user's own, kept at Connect and chosen to be chained
//! (adr.rg.038), is run here with the same input, through the shell Claude Code would
//! have used, and its output is the line printed. If it fails, prints nothing or takes
//! longer than `CHAIN_TIMEOUT`, ReviewGlass's own line is printed instead.

use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reviewglass_lib::session::payload::{SpoolRecord, StatusLine};
use reviewglass_lib::{integration, spool};

/// How long a chained status line of the user's own may take before ReviewGlass's own
/// line stands in for it.
const CHAIN_TIMEOUT: Duration = Duration::from_millis(1500);

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
    let line = integration::read_record()
        .chained_command()
        .and_then(|cmd| run_chained(&cmd, &raw))
        .unwrap_or_else(|| render(&status));

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

/// The shell Claude Code runs a status line through on Windows: Git Bash when it is
/// installed (`CLAUDE_CODE_GIT_BASH_PATH`, or Git's usual place), else PowerShell.
fn shell_for(cmd: &str) -> Command {
    let bash = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| {
            [
                r"C:\Program Files\Git\bin\bash.exe",
                r"C:\Program Files (x86)\Git\bin\bash.exe",
            ]
            .iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.is_file())
        });
    let mut c = match bash {
        Some(b) => {
            let mut c = Command::new(b);
            c.arg("-c").arg(cmd);
            c
        }
        None => {
            let mut c = Command::new("powershell.exe");
            c.args(["-NoProfile", "-NonInteractive", "-Command", cmd]);
            c
        }
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up on every assistant message.
        c.creation_flags(0x0800_0000);
    }
    c
}

/// Run the user's own status line with the payload on stdin; its output, trimmed of
/// trailing line breaks, or `None` when it fails, prints nothing or runs too long.
fn run_chained(cmd: &str, payload: &str) -> Option<String> {
    let mut child = shell_for(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes());
    }
    // Read on a thread of its own so a full pipe never stalls the wait.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = reader.join().ok()?;
                let out = out.trim_end_matches(['\r', '\n']).to_owned();
                return (status.success() && !out.trim().is_empty()).then_some(out);
            }
            Ok(None) if start.elapsed() < CHAIN_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(5))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The chained line runs through a real shell (Git Bash where it is installed, as on
    // the owner's machine and windows-latest, else PowerShell); both understand these.
    #[test]
    fn a_chained_line_prints_the_users_own_output() {
        assert_eq!(
            run_chained("echo my-line", "{}").as_deref(),
            Some("my-line")
        );
    }

    #[test]
    fn a_chained_line_that_fails_or_prints_nothing_gives_way_to_ours() {
        assert_eq!(run_chained("exit 3", "{}"), None);
        assert_eq!(run_chained("exit 0", "{}"), None);
    }

    #[test]
    fn a_chained_line_that_runs_too_long_is_stopped() {
        let t = Instant::now();
        assert_eq!(run_chained("sleep 5", "{}"), None);
        assert!(t.elapsed() < CHAIN_TIMEOUT + Duration::from_secs(1));
    }

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
