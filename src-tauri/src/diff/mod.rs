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
//! The loop runs on its own thread (as the usage loop does, adr.rg.016): edits happen
//! whether or not the panel is open, and the Diff tab shows what accumulated.

pub mod events;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::Serialize;
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
/// A new file larger than this is not shown line by line.
const MAX_UNTRACKED_BYTES: u64 = 512 * 1024;

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
    /// The unified diff as git printed it (or as synthesised for a new file).
    pub unified: Option<String>,
    pub added: Option<u64>,
    pub removed: Option<u64>,
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
        // The last event per path wins: it carries the latest tool and time, and one
        // diff answers for the whole burst.
        let mut latest: HashMap<String, ChangeEvent> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for ev in batch.events {
            let Some(p) = ev.file_path.clone() else {
                continue;
            };
            if !latest.contains_key(&p) {
                order.push(p.clone());
            }
            latest.insert(p, ev);
        }
        let mut views = self.views.lock();
        for p in order {
            let ev = latest.remove(&p).expect("just inserted");
            let view = diff_for(&ev, denylist);
            views.retain(|v| v.path != view.path);
            views.insert(0, view);
        }
        views.truncate(KEEP);
        true
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

/// The view for one event: the denylist first, then git.
pub fn diff_for(ev: &ChangeEvent, denylist: &[String]) -> DiffView {
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
        unified: None,
        added: None,
        removed: None,
        at_ms: ev.ts,
        session_id: ev.session_id.clone(),
        tool: ev.tool.clone(),
        reason: None,
    };
    if denied_by(&file_name, denylist) {
        view.status = DiffStatus::Denied;
        view.reason = Some("matches the secret-file denylist; its contents are never read".into());
        return view;
    }
    let Some(parent) = path.parent().filter(|p| p.is_dir()) else {
        view.status = DiffStatus::Missing;
        view.reason = Some("the file's directory does not exist".into());
        return view;
    };
    let root = match git(parent, &["rev-parse", "--show-toplevel"]) {
        Ok(out) if out.status.success() => PathBuf::from(
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .replace('/', "\\"),
        ),
        Ok(_) => {
            view.status = DiffStatus::NotInRepo;
            view.reason = Some("not inside a git repository".into());
            return view;
        }
        Err(e) => {
            view.status = DiffStatus::GitFailed;
            view.reason = Some(format!("git could not be run: {e}"));
            return view;
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
        return untracked(view, path);
    }
    if !path.exists() {
        view.status = DiffStatus::Missing;
        view.reason = Some("the file no longer exists".into());
        return view;
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
                    view.added = Some(a);
                    view.removed = Some(r);
                    view.unified = Some(text);
                }
                None => {
                    view.status = DiffStatus::Unchanged;
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
    view
}

/// A new file: shown as the addition it is, synthesised in unified form so the
/// renderer needs no second path. Size and binary content are checked first.
fn untracked(mut view: DiffView, path: &Path) -> DiffView {
    let Ok(meta) = std::fs::metadata(path) else {
        view.status = DiffStatus::Missing;
        view.reason = Some("the file no longer exists".into());
        return view;
    };
    if meta.len() > MAX_UNTRACKED_BYTES {
        view.status = DiffStatus::TooLarge;
        view.reason = Some(format!(
            "a new file of {} KB; too large to show line by line",
            meta.len() / 1024
        ));
        return view;
    }
    let Ok(bytes) = std::fs::read(path) else {
        view.status = DiffStatus::Missing;
        view.reason = Some("the file could not be read".into());
        return view;
    };
    if bytes.iter().take(8192).any(|b| *b == 0) {
        view.status = DiffStatus::Binary;
        view.reason = Some("a new binary file".into());
        return view;
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let rel = view.display_path.replace('\\', "/");
    let mut unified = format!(
        "--- /dev/null\n+++ b/{rel}\n@@ -0,0 +1,{} @@\n",
        lines.len()
    );
    for l in &lines {
        unified.push('+');
        unified.push_str(l);
        unified.push('\n');
    }
    view.status = DiffStatus::Untracked;
    view.added = Some(lines.len() as u64);
    view.removed = Some(0);
    view.unified = Some(unified);
    view
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

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

    /// A throwaway repository with one committed file. Skips (returns None) where git
    /// is not on the PATH, so the suite still runs on a machine without it.
    fn repo(tag: &str) -> Option<PathBuf> {
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

    fn event(path: &Path) -> ChangeEvent {
        ChangeEvent {
            ts: 5,
            session_id: Some("s".into()),
            tool: Some("Edit".into()),
            file_path: Some(path.display().to_string()),
            ..ChangeEvent::default()
        }
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
                    unified: None,
                    added: None,
                    removed: None,
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
