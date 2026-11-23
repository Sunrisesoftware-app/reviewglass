//! About (adr.rg.037): which ReviewGlass this is, and where newer ones are.
//!
//! ReviewGlass never asks the network for a version. Settings > About shows the version
//! and the build, and a button opens the releases page in the default browser: the
//! browser fetches, the app does not. The page is a fixed address; nothing the page or a
//! file says can change where the button goes.

use serde::Serialize;

/// Where releases are published (adr.rg.037). Fixed here, never taken from input.
pub const RELEASES_URL: &str = "https://github.com/Sunrisesoftware-app/reviewglass/releases";

#[derive(Serialize)]
pub struct About {
    /// The version, as the three manifests carry it (`scripts/check-version.mjs`).
    pub version: &'static str,
    /// The commit the build was made from; a trailing `+` means uncommitted changes.
    pub commit: &'static str,
    pub releases_url: &'static str,
}

pub fn about() -> About {
    About {
        version: env!("CARGO_PKG_VERSION"),
        commit: env!("RG_BUILD_COMMIT"),
        releases_url: RELEASES_URL,
    }
}

#[tauri::command]
pub fn app_about() -> About {
    about()
}

/// Open the releases page in the default browser. Only this address, ever.
#[tauri::command]
pub fn about_open_releases() -> Result<(), String> {
    open_in_browser(RELEASES_URL)
}

#[cfg(windows)]
fn open_in_browser(url: &str) -> Result<(), String> {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let target = HSTRING::from(url);
    // SAFETY: every pointer is a valid null-terminated wide string that outlives the
    // call; a null window and a null directory are allowed.
    let r = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &target,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecute reports success as a value over 32.
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!(
            "the browser could not be opened (code {})",
            r.0 as isize
        ))
    }
}

#[cfg(not(windows))]
fn open_in_browser(_url: &str) -> Result<(), String> {
    Err("opening a browser is Windows-only in this build".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_releases_page_is_the_project_s_own_over_https() {
        assert!(RELEASES_URL.starts_with("https://github.com/Sunrisesoftware-app/reviewglass/"));
        assert!(RELEASES_URL.ends_with("/releases"));
    }

    #[test]
    fn about_carries_the_crate_version() {
        let a = about();
        assert_eq!(a.version, env!("CARGO_PKG_VERSION"));
        assert!(!a.version.is_empty());
    }
}
