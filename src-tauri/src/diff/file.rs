//! The whole-file view (P4b): the working copy of one edited file, read at the
//! moment it is asked for and handed to the panel with the lines the last diff added
//! marked, so a hunk can be read in the context git's three lines do not give. The
//! hunk view stays the default; this opens from it and returns to it.
//!
//! A read surface, narrowed twice. Only a path the hook has reported since the app
//! started can be opened, so the panel's IPC cannot be turned into a file reader; and
//! the secret-file denylist, the size cap and the binary check apply exactly as they
//! do to a new file's diff. Nothing is written, nothing is kept beyond the call.

use std::path::Path;

use serde::Serialize;
use tauri::State;

use super::{denied_by, DiffState, DiffStatus, DiffView, MAX_SHOWN_BYTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileStatus {
    /// The text is there, with the marks.
    Shown,
    /// Not a path the agent has edited since the app started; nothing was read.
    NotOffered,
    /// The file name matches the secret-file denylist; nothing was read.
    Denied,
    /// The path no longer exists, or is not a file that can be read.
    Missing,
    /// Larger than the cap; not shown line by line.
    TooLarge,
    /// Binary content.
    Binary,
}

/// One hunk of the last diff, by its new-side lines: where the file view scrolls to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Hunk {
    /// The first new-side line to look at (1-based). For a hunk that removed lines
    /// and added none, the line the removal sits before.
    pub start: u32,
    /// New-side lines in the hunk; 0 when it only removed.
    pub lines: u32,
}

/// What the panel renders for the whole file. Optional fields are absent when the
/// status says why.
#[derive(Debug, Clone, Serialize)]
pub struct FileView {
    pub path: String,
    pub display_path: String,
    pub repo_root: Option<String>,
    pub status: FileStatus,
    /// The working copy as text (lossy UTF-8); absent when the status says why.
    pub text: Option<String>,
    /// Lines the last diff added, 1-based, ascending.
    pub added: Vec<u32>,
    /// New-side line numbers before which the last diff removed lines; one past the
    /// last line for a removal at the end.
    pub removed_before: Vec<u32>,
    pub hunks: Vec<Hunk>,
    /// Why there is nothing to show, in the user's terms, when there is nothing.
    pub reason: Option<String>,
}

/// The new-side positions a unified diff touches.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Marks {
    pub added: Vec<u32>,
    pub removed_before: Vec<u32>,
    pub hunks: Vec<Hunk>,
}

/// Read the hunk headers and the line prefixes of a unified diff; the text of the
/// lines is not interpreted. Lines before the first `@@` (the file headers) are
/// skipped, so a `+++ b/path` line is never taken for an addition.
pub fn marks(unified: &str) -> Marks {
    let mut m = Marks::default();
    let mut in_hunk = false;
    let mut new_line: u32 = 0;
    for line in unified.lines() {
        if let Some(rest) = line.strip_prefix("@@ ") {
            let plus = rest.split(' ').find(|p| p.starts_with('+')).unwrap_or("+1");
            let (start, count) = range(&plus[1..]);
            // An empty new range names the line before the removal; the view looks
            // at the line after it.
            let first = if count == 0 { start + 1 } else { start };
            m.hunks.push(Hunk {
                start: first,
                lines: count,
            });
            new_line = first;
            in_hunk = true;
            continue;
        }
        if line.starts_with("diff ") {
            in_hunk = false;
            continue;
        }
        if !in_hunk {
            continue;
        }
        match line.as_bytes().first() {
            Some(b'+') => {
                m.added.push(new_line);
                new_line += 1;
            }
            Some(b'-') => {
                if m.removed_before.last() != Some(&new_line) {
                    m.removed_before.push(new_line);
                }
            }
            Some(b' ') => new_line += 1,
            // "\ No newline at end of file", or an empty line git printed as such.
            _ => {}
        }
    }
    m
}

/// `c,d` or `c` from a hunk header: the start and the count (1 when absent).
fn range(s: &str) -> (u32, u32) {
    let mut it = s.split(',');
    let start = it.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    let count = it.next().map(|v| v.parse().unwrap_or(0)).unwrap_or(1);
    (start, count)
}

fn file_name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// The view for a path nobody edited since start: nothing is read.
pub fn not_offered(path: &str) -> FileView {
    FileView {
        path: path.to_string(),
        display_path: file_name_of(path),
        repo_root: None,
        status: FileStatus::NotOffered,
        text: None,
        added: Vec::new(),
        removed_before: Vec::new(),
        hunks: Vec::new(),
        reason: Some("not a file the agent has edited since ReviewGlass started".into()),
    }
}

/// The whole file behind one diff view: the denylist first, then the size and the
/// binary check, then the text with the last diff's marks.
pub fn file_view(view: &DiffView, denylist: &[String]) -> FileView {
    let mut out = FileView {
        path: view.path.clone(),
        display_path: view.display_path.clone(),
        repo_root: view.repo_root.clone(),
        status: FileStatus::Shown,
        text: None,
        added: Vec::new(),
        removed_before: Vec::new(),
        hunks: Vec::new(),
        reason: None,
    };
    if view.status == DiffStatus::Denied || denied_by(&file_name_of(&view.path), denylist) {
        out.status = FileStatus::Denied;
        out.reason = Some("matches the secret-file denylist; its contents are never read".into());
        return out;
    }
    let path = Path::new(&view.path);
    let Ok(meta) = std::fs::metadata(path) else {
        out.status = FileStatus::Missing;
        out.reason = Some("the file no longer exists".into());
        return out;
    };
    if !meta.is_file() {
        out.status = FileStatus::Missing;
        out.reason = Some("not a file".into());
        return out;
    }
    if meta.len() > MAX_SHOWN_BYTES {
        out.status = FileStatus::TooLarge;
        out.reason = Some(format!(
            "{} KB; too large to show line by line",
            meta.len() / 1024
        ));
        return out;
    }
    let Ok(bytes) = std::fs::read(path) else {
        out.status = FileStatus::Missing;
        out.reason = Some("the file could not be read".into());
        return out;
    };
    if bytes.iter().take(8192).any(|b| *b == 0) {
        out.status = FileStatus::Binary;
        out.reason = Some("binary content".into());
        return out;
    }
    if let Some(u) = &view.unified {
        let m = marks(u);
        out.added = m.added;
        out.removed_before = m.removed_before;
        out.hunks = m.hunks;
    }
    out.text = Some(String::from_utf8_lossy(&bytes).into_owned());
    out
}

/// The whole file for one path the Diff tab lists. A path the diff loop has not seen
/// since start is refused without a read.
#[tauri::command]
pub fn panel_file_view(
    state: State<DiffState>,
    store: State<crate::config::Store>,
    path: String,
) -> FileView {
    let denylist = store.get().diff.denylist;
    match state.views().into_iter().find(|v| v.path == path) {
        Some(v) => file_view(&v, &denylist),
        None => not_offered(&path),
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{event, repo, strings};
    use super::super::{diff_for, DEFAULT_DENYLIST};
    use super::*;
    use std::fs;

    #[test]
    fn marks_read_hunks_additions_and_removals_by_new_side_line() {
        let u = "diff --git a/x b/x\n--- a/x\n+++ b/x\n\
                 @@ -1,3 +1,4 @@\n one\n-two\n+2\n three\n+four\n\
                 @@ -10,2 +11,3 @@ fn ctx()\n ten\n+ten and a half\n eleven\n\
                 \\ No newline at end of file\n";
        let m = marks(u);
        assert_eq!(m.added, vec![2, 4, 12]);
        assert_eq!(m.removed_before, vec![2]);
        assert_eq!(
            m.hunks,
            vec![
                Hunk { start: 1, lines: 4 },
                Hunk {
                    start: 11,
                    lines: 3
                }
            ]
        );
    }

    #[test]
    fn marks_of_removals_only_and_of_a_synthesised_addition() {
        let m = marks("--- a/x\n+++ b/x\n@@ -1,3 +0,0 @@\n-one\n-two\n-three\n");
        assert!(m.added.is_empty());
        assert_eq!(m.removed_before, vec![1]);
        assert_eq!(m.hunks, vec![Hunk { start: 1, lines: 0 }]);
        // A removal at the end of a file with context: before line 3, i.e. one past
        // the last remaining line.
        let m = marks("@@ -1,3 +1,2 @@\n one\n two\n-three\n");
        assert_eq!(m.removed_before, vec![3]);
        // The diff the service synthesises for a new file.
        let m = marks("--- /dev/null\n+++ b/src/b.rs\n@@ -0,0 +1,2 @@\n+fn main() {}\n+// two\n");
        assert_eq!(m.added, vec![1, 2]);
        assert!(m.removed_before.is_empty());
        assert_eq!(m.hunks, vec![Hunk { start: 1, lines: 2 }]);
        // A header without a count means one line.
        assert_eq!(
            marks("@@ -1 +1 @@\n-a\n+b\n").hunks,
            vec![Hunk { start: 1, lines: 1 }]
        );
    }

    #[test]
    fn the_working_copy_is_shown_with_the_last_diffs_marks() {
        let Some(d) = repo("file") else { return };
        let f = d.join("a.txt");
        fs::write(&f, "one\n2\nthree\nfour\n").unwrap();
        let v = diff_for(&event(&f), &strings(DEFAULT_DENYLIST));
        assert_eq!(v.status, DiffStatus::Changed, "{:?}", v.reason);
        let fv = file_view(&v, &strings(DEFAULT_DENYLIST));
        assert_eq!(fv.status, FileStatus::Shown, "{:?}", fv.reason);
        assert_eq!(fv.text.as_deref(), Some("one\n2\nthree\nfour\n"));
        assert_eq!(fv.added, vec![2, 4]);
        assert_eq!(fv.removed_before, vec![2]);
        assert_eq!(fv.hunks, vec![Hunk { start: 1, lines: 4 }]);
        assert_eq!(fv.display_path, "a.txt");
        assert!(fv.repo_root.is_some());
        // Unchanged since the commit: the file still opens, with nothing marked.
        fs::write(&f, "one\ntwo\nthree\n").unwrap();
        let v = diff_for(&event(&f), &[]);
        assert_eq!(v.status, DiffStatus::Unchanged);
        let fv = file_view(&v, &[]);
        assert_eq!(fv.status, FileStatus::Shown);
        assert!(fv.added.is_empty() && fv.hunks.is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn denied_missing_too_large_binary_and_not_offered_read_nothing() {
        let deny = strings(DEFAULT_DENYLIST);
        let d = std::env::temp_dir().join(format!("reviewglass-file-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        // Denied by name, even with a view that was never marked denied.
        let env = d.join(".env");
        fs::write(&env, "SECRET=1\n").unwrap();
        let mut v = DiffView {
            path: env.display().to_string(),
            display_path: ".env".into(),
            repo_root: None,
            status: DiffStatus::NotInRepo,
            baseline: None,
            unified: None,
            added: None,
            removed: None,
            at_ms: 1,
            session_id: None,
            tool: None,
            reason: None,
        };
        let fv = file_view(&v, &deny);
        assert_eq!(fv.status, FileStatus::Denied);
        assert!(fv.text.is_none());
        // Missing.
        v.path = d.join("gone.txt").display().to_string();
        assert_eq!(file_view(&v, &deny).status, FileStatus::Missing);
        // Too large.
        let big = d.join("big.txt");
        fs::write(&big, vec![b'x'; (MAX_SHOWN_BYTES + 1) as usize]).unwrap();
        v.path = big.display().to_string();
        let fv = file_view(&v, &deny);
        assert_eq!(fv.status, FileStatus::TooLarge);
        assert!(fv.text.is_none());
        // Binary.
        let bin = d.join("blob.bin");
        fs::write(&bin, [0u8, 1, 2, 0, 3]).unwrap();
        v.path = bin.display().to_string();
        assert_eq!(file_view(&v, &deny).status, FileStatus::Binary);
        // Outside any repository the file still shows: the hook reported it.
        let loose = d.join("loose.txt");
        fs::write(&loose, "x\n").unwrap();
        v.path = loose.display().to_string();
        let fv = file_view(&v, &deny);
        assert_eq!(fv.status, FileStatus::Shown);
        assert_eq!(fv.text.as_deref(), Some("x\n"));
        // Not offered: nothing read, the reason named.
        let fv = not_offered(&loose.display().to_string());
        assert_eq!(fv.status, FileStatus::NotOffered);
        assert!(fv.text.is_none() && fv.reason.is_some());
        assert_eq!(fv.display_path, "loose.txt");
        let _ = fs::remove_dir_all(&d);
    }
}
