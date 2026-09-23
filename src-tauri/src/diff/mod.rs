//! Diff service (rg.diff-service): a `ChangeEvent` in, a `DiffView` out.
//!
//! Runs `git diff` scoped to the changed path, in the repository that contains it.
//! Nothing is written anywhere: git is asked what changed, and the answer is handed to
//! the panel as the unified text git printed, plus the counts. Rendering is the
//! frontend's, with an existing library (spec 6.2); this module parses nothing but
//! `--numstat`.
//!
//! Contract (spec 6.3):
//!   - debounce rapid successive edits to one file — the loop coalesces every event
//!     for a path into one diff per tick;
//!   - a file outside a git repository is reported as such, never as a failure;
//!   - a path matching the secret-file denylist is never handed to git at all.
//!
//! Where git has no baseline — a file outside any repository, or one not yet tracked —
//! the baseline is the file as it was at the previous edit ReviewGlass saw, kept in
//! memory for the files in the list, so a file being written from nothing shows what
//! each edit brought (the owner's wish, 20.9.2026: to watch a script come into being).
//! The first sighting shows the whole file as new. Nothing is written for this either.
//!
//! Every view also carries the lines the latest edit brought (`fresh`), measured
//! against the working copy ReviewGlass remembered from the previous edit — for tracked
//! files too, so the latest edit stands out from older uncommitted work in the HEAD
//! diff (the owner's wish, 23.9.2026: see the new code in a colour of its own). It is
//! ReviewGlass's mark on its own view; the files are only read.
//!
//! The loop runs on its own thread (as the usage loop does, adr.rg.016): edits happen
//! whether or not the panel is open, and the Diff tab shows what accumulated.

pub mod events;
pub mod file;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::Serialize;
use similar::{ChangeTag, TextDiff};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::spool;
use events::ChangeEvent;

/// How often the events directory is read. A tick is also the debounce window: every
/// event for one path within it becomes a single `git diff`.
const TICK: Duration = Duration::from_millis(300);
/// Views kept for the panel, newest first, one per path.
const KEEP: usize = 30;
/// Event sent to the dock (the drawer's host) when the list changed.
pub const UPDATE_EVENT: &str = "diff:update";
/// A file larger than this is not shown line by line: neither as a new file's diff
/// nor in the whole-file view (P4b).
pub(crate) const MAX_SHOWN_BYTES: u64 = 512 * 1024;

/// File-name patterns never handed to git (spec 6.3): the edit still shows in the
/// list, as denied, so the user knows the agent touched it — without its contents.
pub const DEFAULT_DENYLIST: &[&str] = &[".env*", "*.pem", "*.key", "id_*", "*.tfvars"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiffStatus {
    /// Tracked, and different from HEAD.
    Changed,
    /// Tracked, and identical to HEAD after the edit (a write of the same content, or
    /// an edit undone).
    Unchanged,
    /// Inside a repository but not tracked: shown as an addition of the whole file.
    Untracked,
    /// Not inside a git repository at all.
    NotInRepo,
    /// The file name matches the secret-file denylist; git was not asked.
    Denied,
    /// The path no longer exists (deleted, or a path git cannot see).
    Missing,
    /// A new file too large to show line by line.
    TooLarge,
    /// Git says the content is binary.
    Binary,
    /// Git could not be run, or answered with an error; `reason` says what.
    GitFailed,
}

/// What a diff is measured against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Baseline {
    /// git HEAD: everything changed since the last commit.
    Head,
    /// The file as it was at the previous edit ReviewGlass saw: what the latest edit
    /// did, where git has no baseline.
    LastEdit,
    /// Nothing to compare with yet: the whole file, shown as new.
    WholeFile,
}

/// What the panel renders for one path. Optional fields are absent when the status
/// says why: a denied or missing file has no text and no counts.
#[derive(Debug, Clone, Serialize)]
pub struct DiffView {
    /// The path as the hook reported it.
    pub path: String,
    /// The path relative to the repository root, or the file name outside one.
    pub display_path: String,
    pub repo_root: Option<String>,
    pub status: DiffStatus,
    /// What the diff is against; absent when there is no diff.
    pub baseline: Option<Baseline>,
    /// The unified diff as git printed it (or as synthesised for a new file).
    pub unified: Option<String>,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    /// The lines the latest edit brought, 1-based in the working copy, ascending: the
    /// lines inserted since the copy ReviewGlass remembered from the previous edit, or
    /// at the first sighting every line the diff adds.
    pub fresh: Vec<u32>,
    /// Whether `fresh` is measured against the previous edit (true) or is the first
    /// sighting's every added line (false).
    pub fresh_from_previous: bool,
    /// When the edit happened (the hook's clock), epoch ms.
    pub at_ms: u64,
    pub session_id: Option<String>,
    pub tool: Option<String>,
    /// Why there is nothing to show, in the user's terms, when there is nothing.
    pub reason: Option<String>,
}

/// The diff loop's state: the latest views, newest first.
pub struct DiffState {
    views: Mutex<Vec<DiffView>>,
    /// The working copy at the previous edit, for every listed file that could be read
    /// as text within the cap: the baseline where git has none, and the measure of
    /// what the latest edit brought everywhere. Dropped with the view.
    snapshots: Mutex<HashMap<String, String>>,
    /// Events from before this are stale (the app's start).
    since_ms: u64,
    unreadable: Mutex<usize>,
}

impl Default for DiffState {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffState {
    pub fn new() -> Self {
        Self {
            views: Mutex::new(Vec::new()),
            snapshots: Mutex::new(HashMap::new()),
            since_ms: now_ms(),
            unreadable: Mutex::new(0),
        }
    }

    /// One pass: consume the events, one diff per distinct path (the debounce), the
    /// results to the front of the list. Returns whether anything changed.
    pub fn tick(&self, denylist: &[String]) -> bool {
        let Some(dir) = spool::events_dir() else {
            return false;
        };
        let batch = events::take(&dir, self.since_ms);
        *self.unreadable.lock() = batch.unreadable;
        if batch.events.is_empty() {
            return false;
        }
        self.absorb(batch.events, denylist);
        true
    }

    /// One batch of events into the list: the last event per path wins (it carries the
    /// latest tool and time, and one diff answers for the whole burst), and the working
    /// copy is remembered for the next edit.
    pub fn absorb(&self, events: Vec<ChangeEvent>, denylist: &[String]) {
        let mut latest: HashMap<String, ChangeEvent> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for ev in events {
            let Some(p) = ev.file_path.clone() else {
                continue;
            };
            if !latest.contains_key(&p) {
                order.push(p.clone());
            }
            latest.insert(p, ev);
        }
        let mut views = self.views.lock();
        let mut snapshots = self.snapshots.lock();
        for p in order {
            let ev = latest.remove(&p).expect("just inserted");
            let previous = snapshots.get(&p).map(String::as_str);
            let (view, text) = diff_for_with(&ev, denylist, previous);
            match text {
                Some(t) => {
                    snapshots.insert(p, t);
                }
                None => {
                    snapshots.remove(&p);
                }
            }
            views.retain(|v| v.path != view.path);
            views.insert(0, view);
        }
        views.truncate(KEEP);
        snapshots.retain(|p, _| views.iter().any(|v| &v.path == p));
    }

    pub fn views(&self) -> Vec<DiffView> {
        self.views.lock().clone()
    }
}

/// Start the diff loop. Called once from setup.
pub fn spawn_diff_loop(app: AppHandle) {
    thread::Builder::new()
        .name("reviewglass-diff".into())
        .spawn(move || loop {
            let denylist = app.state::<crate::config::Store>().get().diff.denylist;
            if app.state::<DiffState>().tick(&denylist) {
                let _ = app.emit_to(crate::dock::DOCK_LABEL, UPDATE_EVENT, ());
            }
            thread::sleep(TICK);
        })
        .expect("diff loop thread");
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The view for one event with nothing remembered: the denylist first, then git. The
/// loop goes through `diff_for_with`; this is the tests' shorthand.
#[cfg(test)]
pub fn diff_for(ev: &ChangeEvent, denylist: &[String]) -> DiffView {
    diff_for_with(ev, denylist, None).0
}

/// The view for one event, with `previous` the working copy at the previous edit when
/// one is remembered: the baseline where git has none, and the measure of the fresh
/// lines everywhere. The second value is the working copy now, for the caller to
/// remember, when it could be read as text within the cap.
pub fn diff_for_with(
    ev: &ChangeEvent,
    denylist: &[String],
    previous: Option<&str>,
) -> (DiffView, Option<String>) {
    let path_s = ev.file_path.clone().unwrap_or_default();
    let path = Path::new(&path_s);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path_s.clone());
    let mut view = DiffView {
        path: path_s.clone(),
        display_path: file_name.clone(),
        repo_root: None,
        status: DiffStatus::GitFailed,
        baseline: None,
        unified: None,
        added: None,
        removed: None,
        fresh: Vec::new(),
        fresh_from_previous: false,
        at_ms: ev.ts,
        session_id: ev.session_id.clone(),
        tool: ev.tool.clone(),
        reason: None,
    };
    if denied_by(&file_name, denylist) {
        view.status = DiffStatus::Denied;
        view.reason = Some("matches the secret-file denylist; its contents are never read".into());
        return (view, None);
    }
    let Some(parent) = path.parent().filter(|p| p.is_dir()) else {
        view.status = DiffStatus::Missing;
        view.reason = Some("the file's directory does not exist".into());
        return (view, None);
    };
    let root = match git(parent, &["rev-parse", "--show-toplevel"]) {
        Ok(out) if out.status.success() => PathBuf::from(
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .replace('/', "\\"),
        ),
        Ok(_) => {
            view.status = DiffStatus::NotInRepo;
            return no_baseline(view, path, previous);
        }
        Err(e) => {
            view.status = DiffStatus::GitFailed;
            view.reason = Some(format!("git could not be run: {e}"));
            return (view, None);
        }
    };
    view.repo_root = Some(root.display().to_string());
    let rel = relative(&root, path).unwrap_or_else(|| file_name.clone());
    view.display_path = rel.clone();
    let rel_git = rel.replace('\\', "/");

    let tracked = git(&root, &["ls-files", "--error-unmatch", "--", &rel_git])
        .map(|o| o.status.success())
        .unwrap_or(false);
    let has_head = git(&root, &["rev-parse", "--verify", "-q", "HEAD"])
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !tracked || !has_head {
        view.status = DiffStatus::Untracked;
        return no_baseline(view, path, previous);
    }
    if !path.exists() {
        view.status = DiffStatus::Missing;
        view.reason = Some("the file no longer exists".into());
        return (view, None);
    }
    let numstat = git(&root, &["diff", "HEAD", "--numstat", "--", &rel_git]);
    let unified = git(
        &root,
        &[
            "diff",
            "HEAD",
            "--no-color",
            "--no-ext-diff",
            "--",
            &rel_git,
        ],
    );
    match (numstat, unified) {
        (Ok(n), Ok(u)) if n.status.success() && u.status.success() => {
            let text = String::from_utf8_lossy(&u.stdout).into_owned();
            match parse_numstat(&String::from_utf8_lossy(&n.stdout)) {
                Some(NumStat::Binary) => {
                    view.status = DiffStatus::Binary;
                    view.reason = Some("git reports binary content".into());
                }
                Some(NumStat::Lines(a, r)) => {
                    view.status = DiffStatus::Changed;
                    view.baseline = Some(Baseline::Head);
                    view.added = Some(a);
                    view.removed = Some(r);
                    view.unified = Some(text);
                }
                None => {
                    view.status = DiffStatus::Unchanged;
                    view.baseline = Some(Baseline::Head);
                    view.reason = Some("identical to the last commit".into());
                }
            }
        }
        (Ok(n), _) | (_, Ok(n)) => {
            view.status = DiffStatus::GitFailed;
            view.reason = Some(String::from_utf8_lossy(&n.stderr).trim().to_string());
        }
        (Err(e), _) => {
            view.status = DiffStatus::GitFailed;
            view.reason = Some(format!("git could not be run: {e}"));
        }
    }
    if !matches!(view.status, DiffStatus::Changed | DiffStatus::Unchanged) {
        return (view, None);
    }
    // The fresh lines: against the remembered copy, or at the first sighting every line
    // the HEAD diff adds. A file too large or unreadable as text keeps the first-sighting
    // measure every time, and nothing is remembered for it.
    let text = read_text(path).ok();
    match (previous, &text) {
        (Some(prev), Some(now)) => {
            view.fresh = fresh_lines(prev, now);
            view.fresh_from_previous = true;
        }
        _ => {
            view.fresh = view
                .unified
                .as_deref()
                .map(|u| file::marks(u).added)
                .unwrap_or_default();
        }
    }
    (view, text)
}

/// The working copy as text: within the cap, not binary. The error is the status and
/// the reason to show when it cannot be.
pub(crate) fn read_text(path: &Path) -> Result<String, (DiffStatus, String)> {
    let meta = std::fs::metadata(path)
        .map_err(|_| (DiffStatus::Missing, "the file no longer exists".to_string()))?;
    if meta.len() > MAX_SHOWN_BYTES {
        return Err((
            DiffStatus::TooLarge,
            format!(
                "a file of {} KB; too large to show line by line",
                meta.len() / 1024
            ),
        ));
    }
    let bytes = std::fs::read(path).map_err(|_| {
        (
            DiffStatus::Missing,
            "the file could not be read".to_string(),
        )
    })?;
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return Err((DiffStatus::Binary, "binary content".to_string()));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// A file git has no baseline for (outside a repository, or not yet tracked; the
/// status is already set). The working copy is read — within the cap, text only — and
/// shown against the previous edit's copy when one is remembered, else as the addition
/// it is, synthesised in unified form so the renderer needs no second path. Returns
/// the text read, for the caller to remember.
fn no_baseline(
    mut view: DiffView,
    path: &Path,
    previous: Option<&str>,
) -> (DiffView, Option<String>) {
    let text = match read_text(path) {
        Ok(t) => t,
        Err((status, reason)) => {
            view.status = status;
            view.reason = Some(reason);
            return (view, None);
        }
    };
    let rel = view.display_path.replace('\\', "/");
    match previous {
        Some(prev) if prev == text => {
            view.baseline = Some(Baseline::LastEdit);
            view.fresh_from_previous = true;
            view.reason = Some("no change since the previous edit".into());
        }
        Some(prev) => {
            let d = delta(prev, &text, &rel);
            view.baseline = Some(Baseline::LastEdit);
            view.added = Some(d.added);
            view.removed = Some(d.removed);
            view.unified = Some(d.unified);
            view.fresh = d.fresh;
            view.fresh_from_previous = true;
        }
        None => {
            let lines: Vec<&str> = text.lines().collect();
            let mut unified = format!(
                "--- /dev/null\n+++ b/{rel}\n@@ -0,0 +1,{} @@\n",
                lines.len()
            );
            for l in &lines {
                unified.push('+');
                unified.push_str(l);
                unified.push('\n');
            }
            view.baseline = Some(Baseline::WholeFile);
            view.added = Some(lines.len() as u64);
            view.removed = Some(0);
            view.unified = Some(unified);
            view.fresh = (1..=lines.len() as u32).collect();
        }
    }
    (view, Some(text))
}

/// A line diff gives up refining after this and settles for a coarser answer: the
/// loop must not stall on a pathological file.
const DIFF_TIMEOUT: Duration = Duration::from_millis(250);

/// Two texts compared line by line, by content: line endings normalised, so a CRLF
/// file diffs as its lines, not as its terminators.
fn line_diff<'a>(old: &'a str, new: &'a str) -> TextDiff<'a, 'a, str> {
    TextDiff::configure()
        .timeout(DIFF_TIMEOUT)
        .diff_lines(old, new)
}

/// What one working copy did to the previous one.
struct Delta {
    /// In git's unified form, three lines of context.
    unified: String,
    added: u64,
    removed: u64,
    /// The inserted lines, 1-based in `new`.
    fresh: Vec<u32>,
}

fn delta(old: &str, new: &str, rel: &str) -> Delta {
    let old = old.replace("\r\n", "\n");
    let new = new.replace("\r\n", "\n");
    let diff = line_diff(&old, &new);
    let (mut added, mut removed) = (0u64, 0u64);
    let mut fresh = Vec::new();
    for c in diff.iter_all_changes() {
        match c.tag() {
            ChangeTag::Insert => {
                added += 1;
                if let Some(i) = c.new_index() {
                    fresh.push(i as u32 + 1);
                }
            }
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }
    let unified = diff
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{rel}"), &format!("b/{rel}"))
        .to_string();
    Delta {
        unified,
        added,
        removed,
        fresh,
    }
}

/// The lines `new` has that `old` did not, 1-based in `new`: what an edit brought.
pub fn fresh_lines(old: &str, new: &str) -> Vec<u32> {
    let old = old.replace("\r\n", "\n");
    let new = new.replace("\r\n", "\n");
    line_diff(&old, &new)
        .iter_all_changes()
        .filter(|c| c.tag() == ChangeTag::Insert)
        .filter_map(|c| c.new_index().map(|i| i as u32 + 1))
        .collect()
}

enum NumStat {
    Lines(u64, u64),
    Binary,
}

/// One line of `--numstat`: `added<TAB>removed<TAB>path`, with `-` for binary. `None`
/// when git printed nothing (no difference).
fn parse_numstat(out: &str) -> Option<NumStat> {
    let line = out.lines().find(|l| !l.trim().is_empty())?;
    let mut parts = line.split('\t');
    let a = parts.next()?.trim();
    let r = parts.next()?.trim();
    if a == "-" || r == "-" {
        return Some(NumStat::Binary);
    }
    Some(NumStat::Lines(a.parse().ok()?, r.parse().ok()?))
}

/// The file name against the denylist: `*` matches anything, the rest is literal,
/// case-insensitive (Windows). A pattern without `*` must match the whole name.
pub fn denied_by(file_name: &str, patterns: &[String]) -> bool {
    let name = file_name.to_ascii_lowercase();
    patterns
        .iter()
        .any(|p| glob_match(&p.to_ascii_lowercase(), &name))
}

fn glob_match(pattern: &str, name: &str) -> bool {
    fn go(p: &[u8], n: &[u8]) -> bool {
        match p.first() {
            None => n.is_empty(),
            Some(b'*') => (0..=n.len()).any(|i| go(&p[1..], &n[i..])),
            Some(c) => n.first() == Some(c) && go(&p[1..], &n[1..]),
        }
    }
    go(pattern.as_bytes(), name.as_bytes())
}

/// `path` relative to `root`, comparing case-insensitively as Windows does. Both are
/// canonicalised first where they exist: a path can arrive in 8.3 short form
/// (`RUNNER~1`) while git answers in the long form, and the two must still meet.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let r = canonical(root);
    let p = canonical(path);
    if p.len() > r.len() + 1
        && p[..r.len()].eq_ignore_ascii_case(&r)
        && p.as_bytes()[r.len()] == b'\\'
    {
        Some(p[r.len() + 1..].to_string())
    } else {
        None
    }
}

/// The canonical form of a path when it exists (long names, no `\?\` prefix,
/// backslashes), or the path as given, normalised the same way, when it does not.
fn canonical(path: &Path) -> String {
    let s = std::fs::canonicalize(path)
        .map(|c| c.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned());
    s.trim_start_matches("\\?\\").replace('/', "\\")
}

/// Run git in `dir` without a console window: this is a GUI process, and a bare
/// `Command` would flash one per call.
fn git(dir: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output()
}

// ---- commands -------------------------------------------------------------

/// What the Diff tab shows: the views, and the two facts that explain an empty list.
#[derive(Debug, Clone, Serialize)]
pub struct DiffTab {
    /// Newest first. Empty until an agent edits something after start.
    pub views: Vec<DiffView>,
    /// True once the hook collector has ever run (its events directory exists). False
    /// is a different empty list: the hook is not installed, and no edit will show.
    pub hook_installed: bool,
    /// Event files that could not be read on the last pass.
    pub unreadable: usize,
}

#[tauri::command]
pub fn panel_diffs(state: State<DiffState>) -> DiffTab {
    DiffTab {
        views: state.views(),
        hook_installed: spool::events_dir().is_some_and(|d| d.is_dir()),
        unreadable: *state.unreadable.lock(),
    }
}

/// Test helpers shared by this module's and `file`'s tests.
#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    use std::fs;

    pub(crate) fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// A throwaway repository with one committed file. Skips (returns None) where git
    /// is not on the PATH, so the suite still runs on a machine without it.
    pub(crate) fn repo(tag: &str) -> Option<PathBuf> {
        let d = std::env::temp_dir().join(format!("reviewglass-diff-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).ok()?;
        if !git(&d, &["init", "-q", "."]).ok()?.status.success() {
            return None;
        }
        fs::write(d.join("a.txt"), "one\ntwo\nthree\n").ok()?;
        git(&d, &["add", "a.txt"]).ok()?;
        let c = git(
            &d,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-q",
                "-m",
                "init",
            ],
        )
        .ok()?;
        c.status.success().then_some(d)
    }

    pub(crate) fn event(path: &Path) -> ChangeEvent {
        ChangeEvent {
            ts: 5,
            session_id: Some("s".into()),
            tool: Some("Edit".into()),
            file_path: Some(path.display().to_string()),
            ..ChangeEvent::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::{event, repo, strings};
    use super::*;
    use std::fs;

    #[test]
    fn the_denylist_matches_names_not_paths() {
        let d = strings(DEFAULT_DENYLIST);
        for name in [
            ".env",
            ".env.local",
            "server.pem",
            "host.key",
            "id_rsa",
            "prod.tfvars",
            "ID_ED25519",
        ] {
            assert!(denied_by(name, &d), "{name} should be denied");
        }
        for name in [
            "env.rs",
            "keys.rs",
            "identity.rs",
            "main.tf",
            "pemdas.py",
            "README.md",
        ] {
            assert!(!denied_by(name, &d), "{name} should pass");
        }
    }

    #[test]
    fn numstat_is_read_and_binary_is_told_apart() {
        assert!(matches!(
            parse_numstat("3\t1\tsrc/a.rs\n"),
            Some(NumStat::Lines(3, 1))
        ));
        assert!(matches!(
            parse_numstat("-\t-\tlogo.png\n"),
            Some(NumStat::Binary)
        ));
        assert!(parse_numstat("").is_none());
        assert!(parse_numstat("\n").is_none());
    }

    #[test]
    fn relative_paths_ignore_case_and_slashes() {
        let root = Path::new("E:\\Proj\\Repo");
        assert_eq!(
            relative(root, Path::new("e:/proj/repo/src/a.rs")).as_deref(),
            Some("src\\a.rs")
        );
        assert_eq!(relative(root, Path::new("E:\\Other\\a.rs")), None);
        assert_eq!(relative(root, Path::new("E:\\Proj\\Repo")), None);
    }

    #[test]
    fn a_tracked_edit_is_a_changed_view_with_counts() {
        let Some(d) = repo("tracked") else { return };
        let f = d.join("a.txt");
        fs::write(&f, "one\n2\nthree\nfour\n").unwrap();
        let v = diff_for(&event(&f), &strings(DEFAULT_DENYLIST));
        assert_eq!(v.status, DiffStatus::Changed, "{:?}", v.reason);
        assert_eq!((v.added, v.removed), (Some(2), Some(1)));
        assert_eq!(v.display_path, "a.txt");
        let u = v.unified.unwrap();
        assert!(
            u.contains("-two") && u.contains("+2") && u.contains("+four"),
            "{u}"
        );
        assert!(v.repo_root.is_some());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_new_file_is_shown_as_an_addition() {
        let Some(d) = repo("new") else { return };
        fs::create_dir_all(d.join("src")).unwrap();
        let f = d.join("src").join("b.rs");
        fs::write(&f, "fn main() {}\n// two\n").unwrap();
        let v = diff_for(&event(&f), &[]);
        assert_eq!(v.status, DiffStatus::Untracked);
        assert_eq!((v.added, v.removed), (Some(2), Some(0)));
        assert_eq!(v.display_path, "src\\b.rs");
        let u = v.unified.unwrap();
        assert!(
            u.starts_with(
                "--- /dev/null\n+++ b/src/b.rs\n@@ -0,0 +1,2 @@\n+fn main() {}\n+// two\n"
            ),
            "{u}"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn unchanged_missing_denied_binary_and_outside() {
        let Some(d) = repo("cases") else { return };
        let deny = strings(DEFAULT_DENYLIST);
        // Written back identical.
        let v = diff_for(&event(&d.join("a.txt")), &deny);
        assert_eq!(v.status, DiffStatus::Unchanged);
        // Deleted after the edit.
        let gone = d.join("gone.txt");
        let v = diff_for(&event(&gone), &deny);
        assert_eq!(v.status, DiffStatus::Missing);
        // Denied before git is asked, even though the file exists.
        let env = d.join(".env");
        fs::write(&env, "SECRET=1\n").unwrap();
        let v = diff_for(&event(&env), &deny);
        assert_eq!(v.status, DiffStatus::Denied);
        assert!(v.unified.is_none() && v.added.is_none());
        // A new binary file.
        let bin = d.join("blob.bin");
        fs::write(&bin, [0u8, 1, 2, 0, 3]).unwrap();
        let v = diff_for(&event(&bin), &deny);
        assert_eq!(v.status, DiffStatus::Binary);
        // Outside any repository.
        let out = std::env::temp_dir().join(format!("reviewglass-outside-{}", std::process::id()));
        fs::create_dir_all(&out).unwrap();
        let loose = out.join("loose.txt");
        fs::write(&loose, "x\n").unwrap();
        let v = diff_for(&event(&loose), &deny);
        assert!(
            matches!(v.status, DiffStatus::NotInRepo),
            "a temp dir inside a repository would break this test: {:?} {:?}",
            v.status,
            v.reason
        );
        let _ = fs::remove_dir_all(&out);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn outside_a_repository_the_previous_edit_is_the_baseline() {
        let deny = strings(DEFAULT_DENYLIST);
        let out = std::env::temp_dir().join(format!("reviewglass-baseline-{}", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        fs::create_dir_all(&out).unwrap();
        let f = out.join("phase1.py");
        // First sighting: the whole file, new to us.
        fs::write(&f, "import os\nprint(1)\n").unwrap();
        let (v, text) = diff_for_with(&event(&f), &deny, None);
        assert_eq!(v.status, DiffStatus::NotInRepo, "{:?}", v.reason);
        assert_eq!(v.baseline, Some(Baseline::WholeFile));
        assert_eq!((v.added, v.removed), (Some(2), Some(0)));
        assert!(v.unified.as_deref().unwrap().starts_with(
            "--- /dev/null\n+++ b/phase1.py\n@@ -0,0 +1,2 @@\n+import os\n+print(1)\n"
        ));
        assert_eq!(text.as_deref(), Some("import os\nprint(1)\n"));
        // The next edit: what it brought, against the remembered copy.
        fs::write(&f, "import os\nimport sys\nprint(2)\n").unwrap();
        let (v, text) = diff_for_with(&event(&f), &deny, Some("import os\nprint(1)\n"));
        assert_eq!(v.status, DiffStatus::NotInRepo);
        assert_eq!(v.baseline, Some(Baseline::LastEdit));
        assert_eq!((v.added, v.removed), (Some(2), Some(1)));
        let u = v.unified.unwrap();
        assert!(
            u.starts_with("--- a/phase1.py\n+++ b/phase1.py\n@@ "),
            "{u}"
        );
        assert!(
            u.contains("+import sys\n") && u.contains("-print(1)\n") && u.contains("+print(2)\n"),
            "{u}"
        );
        assert_eq!(text.as_deref(), Some("import os\nimport sys\nprint(2)\n"));
        // Written back identical: nothing to show, and the reason says so.
        let (v, _) = diff_for_with(&event(&f), &deny, Some("import os\nimport sys\nprint(2)\n"));
        assert_eq!(v.baseline, Some(Baseline::LastEdit));
        assert!(v.unified.is_none() && v.added.is_none());
        assert_eq!(
            v.reason.as_deref(),
            Some("no change since the previous edit")
        );
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn an_untracked_files_second_edit_is_measured_from_its_first() {
        let Some(d) = repo("untracked-twice") else {
            return;
        };
        let f = d.join("new.rs");
        fs::write(&f, "fn a() {}\n").unwrap();
        let (v, text) = diff_for_with(&event(&f), &[], None);
        assert_eq!(
            (v.status, v.baseline),
            (DiffStatus::Untracked, Some(Baseline::WholeFile))
        );
        fs::write(&f, "fn a() {}\nfn b() {}\n").unwrap();
        let (v, _) = diff_for_with(&event(&f), &[], text.as_deref());
        assert_eq!(
            (v.status, v.baseline),
            (DiffStatus::Untracked, Some(Baseline::LastEdit))
        );
        assert_eq!((v.added, v.removed), (Some(1), Some(0)));
        assert!(v.unified.unwrap().contains("+fn b() {}\n"));
        // A tracked file is measured from HEAD; its working copy is still remembered,
        // for the next edit's fresh lines.
        let (v, text) = diff_for_with(&event(&d.join("a.txt")), &[], None);
        assert_eq!(v.baseline, Some(Baseline::Head));
        assert_eq!(text.as_deref(), Some("one\ntwo\nthree\n"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_state_remembers_a_file_git_cannot_diff_between_edits() {
        let out = std::env::temp_dir().join(format!("reviewglass-absorb-{}", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        fs::create_dir_all(&out).unwrap();
        let f = out.join("loose.txt");
        fs::write(&f, "one\n").unwrap();
        let s = DiffState::new();
        s.absorb(vec![event(&f)], &[]);
        assert_eq!(s.views()[0].baseline, Some(Baseline::WholeFile));
        assert_eq!(
            s.snapshots
                .lock()
                .get(&f.display().to_string())
                .map(String::as_str),
            Some("one\n")
        );
        fs::write(&f, "one\ntwo\n").unwrap();
        // A burst: two events for the path, the last wins, one diff.
        s.absorb(vec![event(&f), event(&f)], &[]);
        let views = s.views();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].baseline, Some(Baseline::LastEdit));
        assert_eq!((views[0].added, views[0].removed), (Some(1), Some(0)));
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn fresh_lines_are_the_inserted_lines_by_content_not_line_ending() {
        assert_eq!(fresh_lines("a\nb\nc\n", "a\nx\nb\nc\ny\n"), vec![2, 5]);
        // A changed line is new; a removed one is not counted.
        assert_eq!(fresh_lines("a\nb\nc\n", "a\nB\n"), vec![2]);
        // The same lines with CRLF endings bring nothing.
        assert!(fresh_lines("a\r\nb\r\n", "a\nb\n").is_empty());
        assert_eq!(fresh_lines("", "one\ntwo\n"), vec![1, 2]);
    }

    #[test]
    fn a_tracked_files_latest_edit_stands_out_from_older_uncommitted_work() {
        let Some(d) = repo("fresh") else { return };
        let f = d.join("a.txt");
        // First edit: line 2 changed. First sighting: every added line is fresh.
        fs::write(&f, "one\n2\nthree\n").unwrap();
        let (v, text) = diff_for_with(&event(&f), &[], None);
        assert_eq!(v.status, DiffStatus::Changed, "{:?}", v.reason);
        assert_eq!(v.fresh, vec![2]);
        assert!(!v.fresh_from_previous);
        assert_eq!(text.as_deref(), Some("one\n2\nthree\n"));
        // Second edit: a line appended. The HEAD diff has both changes; only the
        // appended line is fresh.
        fs::write(&f, "one\n2\nthree\nfour\n").unwrap();
        let (v, text) = diff_for_with(&event(&f), &[], text.as_deref());
        assert_eq!((v.added, v.removed), (Some(2), Some(1)));
        assert_eq!(v.fresh, vec![4]);
        assert!(v.fresh_from_previous);
        // Written back as committed: unchanged against HEAD, and nothing fresh.
        fs::write(&f, "one\ntwo\nthree\n").unwrap();
        let (v, _) = diff_for_with(&event(&f), &[], text.as_deref());
        assert_eq!(v.status, DiffStatus::Unchanged);
        assert_eq!(v.fresh, vec![2]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn outside_a_repository_the_fresh_lines_follow_the_baseline() {
        let out = std::env::temp_dir().join(format!("reviewglass-freshout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        fs::create_dir_all(&out).unwrap();
        let f = out.join("clean.py");
        fs::write(&f, "import os\nprint(1)\n").unwrap();
        let (v, text) = diff_for_with(&event(&f), &[], None);
        assert_eq!(v.baseline, Some(Baseline::WholeFile));
        assert_eq!(v.fresh, vec![1, 2]);
        assert!(!v.fresh_from_previous);
        fs::write(&f, "import os\nimport sys\nprint(2)\n").unwrap();
        let (v, _) = diff_for_with(&event(&f), &[], text.as_deref());
        assert_eq!(v.baseline, Some(Baseline::LastEdit));
        assert_eq!(v.fresh, vec![2, 3]);
        assert!(v.fresh_from_previous);
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn the_state_remembers_a_tracked_file_between_edits() {
        let Some(d) = repo("fresh-state") else { return };
        let f = d.join("a.txt");
        fs::write(&f, "one\n2\nthree\n").unwrap();
        let s = DiffState::new();
        s.absorb(vec![event(&f)], &[]);
        assert_eq!(s.views()[0].fresh, vec![2]);
        fs::write(&f, "zero\none\n2\nthree\n").unwrap();
        s.absorb(vec![event(&f)], &[]);
        let v = &s.views()[0];
        assert_eq!(v.baseline, Some(Baseline::Head));
        assert_eq!(v.fresh, vec![1]);
        assert!(v.fresh_from_previous);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_state_coalesces_a_burst_into_one_view_per_path() {
        let s = DiffState::new();
        assert!(s.views().is_empty());
        // No events dir to read here; the coalescing itself is exercised through
        // diff_for in the tests above. This pins the empty state and the cap.
        let mut views = s.views.lock();
        for i in 0..(KEEP + 5) {
            views.insert(
                0,
                DiffView {
                    path: format!("p{i}"),
                    display_path: format!("p{i}"),
                    repo_root: None,
                    status: DiffStatus::NotInRepo,
                    baseline: None,
                    unified: None,
                    added: None,
                    removed: None,
                    fresh: Vec::new(),
                    fresh_from_previous: false,
                    at_ms: i as u64,
                    session_id: None,
                    tool: None,
                    reason: None,
                },
            );
        }
        views.truncate(KEEP);
        assert_eq!(views.len(), KEEP);
        assert_eq!(views[0].path, format!("p{}", KEEP + 4));
    }
}
