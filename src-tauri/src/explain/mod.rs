//! Explain service (rg.explain-service): novice mode, P6 (spec 6.3, adr.rg.002,
//! adr.rg.023). A plain-language explanation of ONE hunk of the live diff, on a button
//! press on that hunk, through the backend the user chose.
//!
//! The contract, in the order this module keeps it:
//! - off by default in a fresh installation, with no backend chosen;
//! - explicitly user-triggered, one hunk at a time; never on a timer, never in the
//!   background; a new request cancels the one in flight, and the UI can cancel;
//! - the secret-file denylist of the diff service applies: a denied file is never read
//!   or sent, and only a path the diff loop has listed can be explained;
//! - the prompt is built here and holds the hunk, the file's path and the surrounding
//!   declaration - never the whole file, the repository or a transcript;
//! - every answer names the backend it came through, so local and remote are never
//!   confused; the first remote request of an installation shows its exact payload and
//!   waits for a yes before anything leaves the machine;
//! - no provider-specific code here: a provider is named only under `backend/`.

pub mod backend;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Notify;

use crate::config::Store;
use crate::diff::{denied_by, DiffState, DiffStatus};
use backend::{Backend, Preview, Prompt, Reply};

/// The largest hunk explained: a hunk is a question about a change, not a file.
pub const MAX_HUNK_LINES: usize = 400;
pub const MAX_HUNK_CHARS: usize = 24_000;
/// The longest declaration line sent.
const MAX_DECLARATION: usize = 200;

/// Which backend explains. `Off` is the default and sends nothing, ever.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    #[default]
    Off,
    Local,
    Remote,
}

/// Novice mode's settings. The remote key is not here: it lives in Windows Credential
/// Manager (`backend::credential`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExplainConfig {
    pub backend: BackendKind,
    /// The local endpoint: loopback only (127.0.0.0/8, ::1, localhost).
    pub local_host: String,
    pub local_port: u16,
    /// The model the local server serves, by its name there.
    pub local_model: String,
    /// The remote model; `None` or empty uses the backend's default.
    pub remote_model: Option<String>,
    /// The first remote request's payload has been shown and accepted.
    pub remote_confirmed: bool,
    /// The explanation's language: "auto" (the system's), or a language tag.
    pub language: String,
}

impl Default for ExplainConfig {
    fn default() -> Self {
        Self {
            backend: BackendKind::Off,
            local_host: "127.0.0.1".into(),
            local_port: 11434,
            local_model: String::new(),
            remote_model: None,
            remote_confirmed: false,
            language: "auto".into(),
        }
    }
}

// ---- the hunk, its declaration, the prompt ---------------------------------------

/// One hunk of a unified diff: its header, its text (header included) and where its
/// first change sits on the new side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub header: String,
    pub text: String,
    pub first_change: u32,
}

/// The hunks of a unified diff, in order. File headers before the first `@@` are not
/// part of any hunk.
pub fn hunks(unified: &str) -> Vec<Hunk> {
    let mut out: Vec<Hunk> = Vec::new();
    let mut new_line = 0u32;
    let mut found_change = false;
    for line in unified.lines() {
        if line.starts_with("@@ ") {
            let start = line
                .split(' ')
                .find(|p| p.starts_with('+'))
                .and_then(|p| p[1..].split(',').next())
                .and_then(|n| n.parse::<u32>().ok())
                .unwrap_or(1);
            new_line = start;
            found_change = false;
            out.push(Hunk {
                header: line.to_string(),
                text: format!("{line}\n"),
                first_change: start,
            });
            continue;
        }
        if line.starts_with("diff ") {
            continue;
        }
        let Some(h) = out.last_mut() else { continue };
        h.text.push_str(line);
        h.text.push('\n');
        match line.as_bytes().first() {
            Some(b'+') | Some(b'-') if !found_change => {
                h.first_change = new_line.max(1);
                found_change = true;
                if line.starts_with('+') {
                    new_line += 1;
                }
            }
            Some(b'+') | Some(b' ') => new_line += 1,
            _ => {}
        }
    }
    out
}

/// The declaration git names after a hunk header's second `@@` (its function context).
pub fn declaration_from_header(header: &str) -> Option<String> {
    let rest = header.strip_prefix("@@ ")?;
    let (_, after) = rest.split_once(" @@")?;
    let t = after.trim();
    (!t.is_empty()).then(|| cut(t, MAX_DECLARATION))
}

fn indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn looks_like_declaration(t: &str) -> bool {
    const STARTS: &[&str] = &[
        "fn ",
        "pub fn ",
        "pub(crate) fn ",
        "async fn ",
        "pub async fn ",
        "impl ",
        "impl<",
        "struct ",
        "pub struct ",
        "enum ",
        "pub enum ",
        "trait ",
        "pub trait ",
        "mod ",
        "pub mod ",
        "def ",
        "async def ",
        "class ",
        "function ",
        "async function ",
        "export function ",
        "export async function ",
        "export default function ",
        "export class ",
        "export default class ",
        "interface ",
        "export interface ",
        "func ",
        "public ",
        "private ",
        "protected ",
        "internal ",
        "static ",
    ];
    if STARTS.iter().any(|s| t.starts_with(s)) {
        return true;
    }
    // An arrow function or a function expression bound to a name.
    (t.starts_with("const ") || t.starts_with("let ") || t.starts_with("export const "))
        && (t.contains("=>") || t.contains("function"))
}

/// The nearest declaration above `line` (1-based) that is less indented than it: where
/// git gave no function context (a file outside a repository, an untracked file).
pub fn declaration_above(text: &str, line: u32) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let at = (line as usize).saturating_sub(1);
    let own = indent(lines.get(at)?);
    if own == 0 {
        return None;
    }
    lines[..at]
        .iter()
        .rev()
        .take(2000)
        .filter(|l| !l.trim().is_empty())
        .find(|l| indent(l) < own && looks_like_declaration(l.trim()))
        .map(|l| cut(l.trim(), MAX_DECLARATION))
}

fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

/// The language the explanation is written in: the setting, or with "auto" the system's
/// (the page passes `navigator.language`). An unknown tag means English.
pub fn language_name(setting: &str, locale: &str) -> &'static str {
    let tag = if setting.trim().is_empty() || setting == "auto" {
        locale
    } else {
        setting
    };
    match tag
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "fi" => "Finnish",
        "sv" => "Swedish",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "et" => "Estonian",
        _ => "English",
    }
}

/// The prompt: the hunk, the path, the declaration around it - nothing else.
pub fn build_prompt(path: &str, declaration: Option<&str>, hunk: &str, language: &str) -> Prompt {
    let system = format!(
        "You explain code changes to someone who is new to programming, while an AI coding \
         agent is making them. You are given one hunk of a unified diff: lines starting with \
         + were added, lines starting with - were removed, the rest is unchanged context. \
         Say in plain words what the change does and why it was probably made, in two to \
         four short paragraphs. If you see a risk - a likely bug, a missing check, a secret, \
         a destructive command - name it plainly in a sentence of its own. Do not rewrite \
         the code and do not repeat it line by line. Write in {language}."
    );
    let mut user = format!("File: {path}\n");
    if let Some(d) = declaration {
        user.push_str(&format!("Inside: {d}\n"));
    }
    user.push_str("\nThe change:\n```diff\n");
    user.push_str(hunk.trim_end());
    user.push_str("\n```\n");
    Prompt { system, user }
}

// ---- running one request -----------------------------------------------------------

/// The request in flight, if any: one at a time.
#[derive(Default)]
pub struct ExplainState {
    running: Mutex<Option<(u64, Arc<Notify>)>>,
    next: AtomicU64,
}

impl ExplainState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new request, cancelling the one in flight.
    fn begin(&self) -> (u64, Arc<Notify>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let n = Arc::new(Notify::new());
        if let Some((_, old)) = self.running.lock().replace((id, n.clone())) {
            old.notify_one();
        }
        (id, n)
    }

    fn end(&self, id: u64) {
        let mut r = self.running.lock();
        if r.as_ref().is_some_and(|(i, _)| *i == id) {
            *r = None;
        }
    }

    /// Cancel the request in flight; `false` when there was none.
    pub fn cancel(&self) -> bool {
        match self.running.lock().take() {
            Some((_, n)) => {
                n.notify_one();
                true
            }
            None => false,
        }
    }
}

/// What a request came to. Every variant but `Cancelled` names the backend.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Outcome {
    Done {
        label: String,
        hunk: String,
        text: String,
        truncated: bool,
    },
    /// The model declined: an answer, shown as one.
    Refused {
        label: String,
        hunk: String,
        reason: String,
    },
    Failed {
        label: String,
        hunk: String,
        message: String,
    },
    /// The first remote request: nothing was sent; this is exactly what would be.
    Confirm {
        label: String,
        hunk: String,
        preview: Preview,
    },
    Cancelled,
}

/// Everything a request needs, checked: the backend, the prompt, the hunk's header.
struct Prepared {
    backend: Backend,
    prompt: Prompt,
    hunk_header: String,
}

fn prepare(
    store: &Store,
    diff: &DiffState,
    path: &str,
    hunk: usize,
    locale: &str,
) -> Result<Prepared, String> {
    let cfg = store.get();
    let backend = Backend::from_config(&cfg.explain)?.ok_or_else(|| {
        "Explanations are off. Choose a backend in Settings > Explain.".to_string()
    })?;
    let view = diff
        .views()
        .into_iter()
        .find(|v| v.path == path)
        .ok_or_else(|| "Not a file the agent has edited since ReviewGlass started.".to_string())?;
    let file_name = std::path::Path::new(&view.path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if view.status == DiffStatus::Denied || denied_by(&file_name, &cfg.diff.denylist) {
        return Err(
            "This file matches the secret-file denylist; it is never read or explained.".into(),
        );
    }
    let unified = view.unified.as_deref().ok_or_else(|| {
        view.reason
            .clone()
            .unwrap_or_else(|| "No diff to explain.".into())
    })?;
    let h = hunks(unified)
        .into_iter()
        .nth(hunk)
        .ok_or_else(|| "That hunk is no longer in the diff.".to_string())?;
    if h.text.lines().count() > MAX_HUNK_LINES || h.text.len() > MAX_HUNK_CHARS {
        return Err(format!(
            "This hunk is too large to explain: one hunk of up to {MAX_HUNK_LINES} lines is explained at a time."
        ));
    }
    let declaration = declaration_from_header(&h.header).or_else(|| {
        crate::diff::read_text(std::path::Path::new(&view.path))
            .ok()
            .and_then(|t| declaration_above(&t, h.first_change))
    });
    let language = language_name(&cfg.explain.language, locale);
    let prompt = build_prompt(
        &view.display_path,
        declaration.as_deref(),
        &h.text,
        language,
    );
    Ok(Prepared {
        backend,
        prompt,
        hunk_header: h.header,
    })
}

// ---- commands -------------------------------------------------------------------------

/// The settings as the Settings tab shows them.
#[derive(Clone, Debug, Serialize)]
pub struct ExplainView {
    pub config: ExplainConfig,
    /// A remote key is stored in Windows Credential Manager. The key itself never is
    /// handed to the page.
    pub key_stored: bool,
    /// The remote model used when the setting is empty.
    pub remote_default_model: String,
    /// The remote service and the known local servers, named by the backend module (a
    /// provider is named nowhere else).
    pub remote_service: String,
    pub local_servers: String,
    /// The active backend's name, when one is chosen and complete.
    pub label: Option<String>,
    /// Why the chosen backend cannot run yet, when it cannot.
    pub problem: Option<String>,
}

fn view_of(store: &Store) -> ExplainView {
    let config = store.get().explain;
    let (label, problem) = match Backend::from_config(&config) {
        Ok(Some(b)) => (Some(b.label()), None),
        Ok(None) => (None, None),
        Err(e) => (None, Some(e)),
    };
    ExplainView {
        config,
        key_stored: backend::credential::stored(),
        remote_default_model: backend::remote_default_model().into(),
        remote_service: backend::remote_service().into(),
        local_servers: backend::local_servers().into(),
        label,
        problem,
    }
}

#[tauri::command]
pub fn explain_settings(store: State<Store>) -> ExplainView {
    view_of(&store)
}

/// Save the settings. The remote confirmation is not settable from here: it is given by
/// accepting the disclosure, and cleared by `explain_reset_confirmation`.
#[tauri::command]
pub fn explain_set(store: State<Store>, config: ExplainConfig) -> Result<ExplainView, String> {
    if config.backend == BackendKind::Local && !backend::is_loopback(&config.local_host) {
        return Err(format!(
            "The local backend must be on this machine (127.0.0.1, ::1 or localhost), not {}.",
            config.local_host
        ));
    }
    store
        .update(|c| {
            let confirmed = c.explain.remote_confirmed;
            c.explain = ExplainConfig {
                remote_confirmed: confirmed,
                ..config
            };
        })
        .map_err(|e| e.to_string())?;
    Ok(view_of(&store))
}

#[tauri::command]
pub fn explain_key_set(store: State<Store>, key: String) -> Result<ExplainView, String> {
    backend::credential::write(&key)?;
    Ok(view_of(&store))
}

#[tauri::command]
pub fn explain_key_clear(store: State<Store>) -> Result<ExplainView, String> {
    backend::credential::delete()?;
    Ok(view_of(&store))
}

/// Show the payload again before the next remote request.
#[tauri::command]
pub fn explain_reset_confirmation(store: State<Store>) -> Result<ExplainView, String> {
    store
        .update(|c| c.explain.remote_confirmed = false)
        .map_err(|e| e.to_string())?;
    Ok(view_of(&store))
}

/// Whether the local endpoint answers, and its models. Sends no prompt.
#[tauri::command]
pub async fn explain_check_local(store: State<'_, Store>) -> Result<Vec<String>, String> {
    let cfg = store.get().explain;
    backend::check_local(&cfg).await.map_err(|f| f.to_string())
}

/// Explain one hunk of a listed file. With a remote backend not yet confirmed, sends
/// nothing and returns the exact payload for the disclosure; `confirmed` is the user's
/// yes to it, remembered for the installation.
#[tauri::command]
pub async fn explain_run(
    state: State<'_, ExplainState>,
    store: State<'_, Store>,
    diff: State<'_, DiffState>,
    path: String,
    hunk: usize,
    locale: String,
    confirmed: bool,
) -> Result<Outcome, String> {
    let p = prepare(&store, &diff, &path, hunk, &locale)?;
    let label = p.backend.label();
    if p.backend.is_remote() && !store.get().explain.remote_confirmed {
        if !confirmed {
            return Ok(Outcome::Confirm {
                label,
                hunk: p.hunk_header,
                preview: p.backend.preview(&p.prompt),
            });
        }
        store
            .update(|c| c.explain.remote_confirmed = true)
            .map_err(|e| e.to_string())?;
    }
    let (id, cancel) = state.begin();
    let result = tokio::select! {
        r = p.backend.explain(&p.prompt) => Some(r),
        _ = cancel.notified() => None,
    };
    state.end(id);
    let hunk_header = p.hunk_header;
    Ok(match result {
        None => Outcome::Cancelled,
        Some(Ok(Reply::Text { text, truncated })) => Outcome::Done {
            label,
            hunk: hunk_header,
            text,
            truncated,
        },
        Some(Ok(Reply::Refused(reason))) => Outcome::Refused {
            label,
            hunk: hunk_header,
            reason,
        },
        Some(Err(f)) => Outcome::Failed {
            label,
            hunk: hunk_header,
            message: f.to_string(),
        },
    })
}

#[tauri::command]
pub fn explain_cancel(state: State<ExplainState>) -> bool {
    state.cancel()
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNIFIED: &str = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n\
        @@ -1,4 +1,5 @@ fn main() {\n     let a = 1;\n-    let b = 2;\n+    let b = 3;\n+    let c = 4;\n     println!(\"{a}\");\n\
        @@ -20,3 +21,3 @@\n-x\n+y\n z\n";

    #[test]
    fn hunks_split_at_headers_and_know_their_first_change() {
        let h = hunks(UNIFIED);
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].header, "@@ -1,4 +1,5 @@ fn main() {");
        assert_eq!(h[0].first_change, 2, "the removal before new line 2");
        assert!(
            h[0].text.starts_with("@@ -1,4 +1,5 @@") && h[0].text.contains("+    let c = 4;\n")
        );
        assert!(!h[0].text.contains("diff --git") && !h[0].text.contains("+++ b/"));
        assert_eq!(h[1].first_change, 21);
        assert_eq!(h[1].text, "@@ -20,3 +21,3 @@\n-x\n+y\n z\n");
        assert!(hunks("--- a\n+++ b\n").is_empty());
    }

    #[test]
    fn the_declaration_comes_from_gits_context_or_the_nearest_enclosing_line() {
        assert_eq!(
            declaration_from_header("@@ -1,4 +1,5 @@ fn main() {").as_deref(),
            Some("fn main() {")
        );
        assert_eq!(declaration_from_header("@@ -20,3 +21,3 @@"), None);
        let py = "import os\n\nclass Cleaner:\n    def run(self, folder):\n        files = collect(folder)\n        return files\n";
        assert_eq!(
            declaration_above(py, 5).as_deref(),
            Some("def run(self, folder):")
        );
        assert_eq!(declaration_above(py, 4).as_deref(), Some("class Cleaner:"));
        assert_eq!(
            declaration_above(py, 1),
            None,
            "top level has no enclosing line"
        );
        let ts = "export const load = async () => {\n  const x = 1;\n  return x;\n};\n";
        assert_eq!(
            declaration_above(ts, 3).as_deref(),
            Some("export const load = async () => {")
        );
    }

    #[test]
    fn the_language_follows_the_setting_or_the_system() {
        assert_eq!(language_name("auto", "fi-FI"), "Finnish");
        assert_eq!(language_name("auto", "en-GB"), "English");
        assert_eq!(language_name("fi", "en-US"), "Finnish");
        assert_eq!(language_name("", "sv_SE"), "Swedish");
        assert_eq!(language_name("auto", "zz"), "English");
    }

    #[test]
    fn the_prompt_holds_the_hunk_the_path_and_the_declaration_only() {
        let p = build_prompt(
            "src/a.rs",
            Some("fn main() {"),
            "@@ -1 +1 @@\n-a\n+b\n",
            "Finnish",
        );
        assert!(p.system.contains("Write in Finnish."));
        assert!(p.system.contains("new to programming"));
        assert_eq!(
            p.user,
            "File: src/a.rs\nInside: fn main() {\n\nThe change:\n```diff\n@@ -1 +1 @@\n-a\n+b\n```\n"
        );
        let p = build_prompt("x.py", None, "@@ -0,0 +1 @@\n+print(1)\n", "English");
        assert!(!p.user.contains("Inside:"));
    }

    #[test]
    fn explanations_are_off_by_default() {
        let c = ExplainConfig::default();
        assert_eq!(c.backend, BackendKind::Off);
        assert!(!c.remote_confirmed);
        assert!(c.remote_model.is_none());
        assert!(Backend::from_config(&c).unwrap().is_none());
        // A config file from before P6 has no explain section: still off.
        let old: crate::config::Config = serde_json::from_str(r#"{"glass":{},"dock":{}}"#).unwrap();
        assert_eq!(old.explain.backend, BackendKind::Off);
    }

    #[test]
    fn a_new_request_cancels_the_one_in_flight() {
        let s = ExplainState::new();
        assert!(!s.cancel(), "nothing in flight");
        let (a, na) = s.begin();
        let (b, _nb) = s.begin();
        assert_ne!(a, b);
        // The first was notified: its wait completes at once.
        tauri::async_runtime::block_on(async {
            tokio::time::timeout(std::time::Duration::from_millis(100), na.notified())
                .await
                .expect("the first request was cancelled");
        });
        s.end(a);
        assert!(s.cancel(), "the second is still in flight");
        s.end(b);
        assert!(!s.cancel());
    }
}
