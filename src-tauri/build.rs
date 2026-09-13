use std::process::Command;

/// Stamp the build with the commit it was made from, so the bar can say which build
/// is being looked at: the version alone stays 0.1.0 across a day of builds.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    let hash = git(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "nogit".into());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let stamp = format!("{hash}{}", if dirty { "+" } else { "" });
    println!("cargo:rustc-env=RG_BUILD_COMMIT={stamp}");
    // Rebuild the stamp when the commit changes. Ask git where HEAD lives: in a
    // worktree .git is a file pointing elsewhere.
    for name in ["HEAD", "index"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    tauri_build::build()
}
