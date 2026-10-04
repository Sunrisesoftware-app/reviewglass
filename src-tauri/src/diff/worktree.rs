//! A session's project's uncommitted changes (adr.rg.031): what the Diff tab and the
//! diff window show for a chosen session that has no edits in the last hour - one that
//! only waits on a timer, or one whose edits are older than the hook's hour.
//!
//! Read from git in the session's working folder: `git status` names the changed and
//! untracked files, and each is diffed the way an edit is (`diff_for_with`), the secret
//! denylist first. Nothing is remembered between reads and nothing is written. The
//! lines a view marks as fresh belong to the hook's edits, so a view from here marks
//! none: these are the project's changes, not the latest edit's.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::events::ChangeEvent;
use super::{diff_for_with, git, DiffState, DiffView};

/// At most this many files are diffed and listed, the newest first; the rest are
/// counted. A working tree with hundreds of changes is a merge or a generated folder,
/// and diffing every file of it would stall the tab for a picture nobody reads.
const MAX_FILES: usize = 40;

/// What the tab shows for a session with no recent edits.
#[derive(Debug, Clone, Serialize)]
pub struct WorktreeTab {
    pub session_id: String,
    /// The session's working folder, as its transcript or status line names it.
    pub cwd: Option<String>,
    /// The repository's root, when the folder is inside one.
    pub root: Option<String>,
    pub views: Vec<DiffView>,
    /// Changed files beyond `MAX_FILES`, not listed.
    pub more: usize,
    /// Why there is nothing to show, when there is nothing: no folder known, not a
    /// repository, git failed. Absent when `views` is the answer (empty = clean).
    pub reason: Option<String>,
}

/// The paths `git status --porcelain=v1 -z` names: changed, staged, untracked and
/// renamed (the new name); ignored files are not asked for. Relative to the root, with
/// forward slashes as git prints them.
pub fn changed_paths(porcelain: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fields = porcelain.split('\0').filter(|f| !f.is_empty());
    while let Some(entry) = fields.next() {
        if entry.len() < 4 {
            continue;
        }
        let (xy, path) = entry.split_at(3);
        let x = xy.as_bytes()[0];
        // A rename or a copy is followed by the old name, which is not listed.
        if x == b'R' || x == b'C' {
            fields.next();
        }
        if xy.starts_with("!!") {
            continue;
        }
        out.push(path.to_string());
    }
    out
}

fn mtime_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The uncommitted changes of the repository `cwd` is in, as views of `session_id`.
pub fn worktree_views(cwd: &Path, session_id: &str, denylist: &[String]) -> WorktreeTab {
    let mut tab = WorktreeTab {
        session_id: session_id.to_string(),
        cwd: Some(cwd.display().to_string()),
        root: None,
        views: Vec::new(),
        more: 0,
        reason: None,
    };
    if !cwd.is_dir() {
        tab.reason = Some("the session's working folder no longer exists".into());
        return tab;
    }
    let root = match git(cwd, &["rev-parse", "--show-toplevel"]) {
        Ok(o) if o.status.success() => {
            PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().replace('/', "\\"))
        }
        Ok(_) => {
            tab.reason = Some("the session's working folder is not in a git repository".into());
            return tab;
        }
        Err(e) => {
            tab.reason = Some(format!("git could not be run: {e}"));
            return tab;
        }
    };
    tab.root = Some(root.display().to_string());
    let status = match git(
        &root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    ) {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        Ok(o) => {
            tab.reason = Some(String::from_utf8_lossy(&o.stderr).trim().to_string());
            return tab;
        }
        Err(e) => {
            tab.reason = Some(format!("git could not be run: {e}"));
            return tab;
        }
    };
    let mut files: Vec<(PathBuf, u64)> = changed_paths(&status)
        .into_iter()
        .map(|rel| {
            let p = root.join(rel.replace('/', "\\"));
            let m = mtime_ms(&p);
            (p, m)
        })
        .collect();
    files.sort_by_key(|(_, m)| std::cmp::Reverse(*m));
    tab.more = files.len().saturating_sub(MAX_FILES);
    for (path, m) in files.into_iter().take(MAX_FILES) {
        let ev = ChangeEvent {
            ts: m,
            session_id: Some(session_id.to_string()),
            transcript_path: None,
            cwd: Some(cwd.display().to_string()),
            tool: None,
            tool_use_id: None,
            file_path: Some(path.display().to_string()),
        };
        let (mut view, _) = diff_for_with(&ev, denylist, None);
        view.fresh.clear();
        view.fresh_from_previous = false;
        tab.views.push(view);
    }
    tab
}

/// The working folder of a session: the live table's, else its transcript's.
fn cwd_of(app: &AppHandle, session_id: &str) -> Option<String> {
    app.state::<crate::panel::PanelState>()
        .session_cwd(session_id)
        .or_else(|| crate::session::transcript::find_by_id(session_id).and_then(|f| f.cwd))
}

/// The uncommitted changes of a session's project, for a session with no edits in the
/// hook's hour. Off the main thread: git may take a moment on a large tree. The views
/// are kept for `panel_file_view`, so the whole-file view works on them too.
#[tauri::command]
pub async fn panel_worktree(app: AppHandle, session_id: String) -> WorktreeTab {
    tauri::async_runtime::spawn_blocking(move || {
        let denylist = app.state::<crate::config::Store>().get().diff.denylist;
        let tab = match cwd_of(&app, &session_id) {
            Some(cwd) => worktree_views(Path::new(&cwd), &session_id, &denylist),
            None => WorktreeTab {
                session_id: session_id.clone(),
                cwd: None,
                root: None,
                views: Vec::new(),
                more: 0,
                reason: Some(
                    "the session's working folder is not known (no transcript in the last three days)"
                        .into(),
                ),
            },
        };
        app.state::<DiffState>().keep_worktree(tab.views.clone());
        tab
    })
    .await
    .unwrap_or_else(|e| WorktreeTab {
        session_id: String::new(),
        cwd: None,
        root: None,
        views: Vec::new(),
        more: 0,
        reason: Some(format!("the read failed: {e}")),
    })
}

#[cfg(test)]
mod tests {
    use super::super::testutil::repo;
    use super::*;
    use std::fs;

    #[test]
    fn porcelain_names_changed_untracked_and_renamed_files() {
        let out = " M src/a.rs\0?? new file.txt\0R  b2.rs\0b.rs\0D  gone.rs\0!! target/x\0";
        assert_eq!(
            changed_paths(out),
            vec!["src/a.rs", "new file.txt", "b2.rs", "gone.rs"]
        );
        assert!(changed_paths("").is_empty());
    }

    #[test]
    fn a_projects_uncommitted_changes_are_listed_with_no_fresh_lines() {
        let Some(d) = repo("worktree") else { return };
        fs::write(d.join("a.txt"), "one\ntwo changed\nthree\n").unwrap();
        fs::write(d.join("b.txt"), "a new file\n").unwrap();
        fs::write(d.join(".env"), "SECRET=1\n").unwrap();
        let deny: Vec<String> = super::super::DEFAULT_DENYLIST
            .iter()
            .map(|s| s.to_string())
            .collect();
        let tab = worktree_views(&d, "s-1", &deny);
        assert_eq!(tab.reason, None);
        let names: Vec<&str> = tab.views.iter().map(|v| v.display_path.as_str()).collect();
        assert!(
            names.contains(&"a.txt") && names.contains(&"b.txt"),
            "{names:?}"
        );
        let env = tab
            .views
            .iter()
            .find(|v| v.display_path.ends_with(".env"))
            .unwrap();
        assert_eq!(env.status, super::super::DiffStatus::Denied);
        assert!(tab.views.iter().all(|v| v.fresh.is_empty()));
        assert!(tab
            .views
            .iter()
            .all(|v| v.session_id.as_deref() == Some("s-1")));
    }

    #[test]
    fn a_folder_outside_a_repository_says_so() {
        let d = std::env::temp_dir().join(format!("rg-worktree-none-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let tab = worktree_views(&d, "s-2", &[]);
        assert!(tab.views.is_empty());
        assert!(tab.reason.is_some());
    }
}
