//! Windows ending the session (shut down, restart, sign out) ends ReviewGlass cleanly.
//!
//! Every "cannot move state from Destroyed" in panic.log (6.10, 29.10 and 9.11.2026)
//! fell on the second Windows logged the shutdown from the Start menu (System log, event
//! 1074). tao's hidden "Tao Thread Event Target" window answers WM_ENDSESSION by
//! destroying the event loop, and then returns to a message loop that is still running:
//! the next message to any of ReviewGlass's windows asks the destroyed loop for work and
//! tao panics, which the release build turns into an abort. Stopping our own threads
//! (the shutting-down flag of 8.10.2026) could not help; the messages come from Windows.
//!
//! A subclass on that window, installed after tao's and so called before it, takes
//! WM_ENDSESSION first: it marks the app as shutting down and ends the process. Windows
//! ends it anyway once the message returns, and nothing is left to write: the config
//! is written atomically on every change (rg.config-store).

#[cfg(windows)]
mod imp {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumThreadWindows, GetClassNameW, WM_ENDSESSION,
    };

    /// tao's own window class for the event loop's message target.
    const TARGET_CLASS: &str = "Tao Thread Event Target";
    const SUBCLASS_ID: usize = 0x5247_454E; // "RGEN"

    unsafe extern "system" fn on_message(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        // wParam is TRUE when the session really ends (FALSE: the shutdown was
        // cancelled, and nothing has been done that needs undoing).
        if msg == WM_ENDSESSION && wparam.0 != 0 {
            crate::begin_shutdown();
            std::process::exit(0);
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }

    unsafe extern "system" fn each(hwnd: HWND, found: LPARAM) -> BOOL {
        let mut buf = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut buf) as usize;
        if String::from_utf16_lossy(&buf[..n.min(buf.len())]) == TARGET_CLASS
            && SetWindowSubclass(hwnd, Some(on_message), SUBCLASS_ID, 0).as_bool()
        {
            *(found.0 as *mut u32) += 1;
        }
        BOOL(1)
    }

    /// Subclass the event loop's message target on this (the main) thread. The answer
    /// is how many were found; none means a tao that names its window differently.
    pub fn install() -> u32 {
        let mut found = 0u32;
        // SAFETY: runs on the main thread, which owns the windows enumerated; `found`
        // outlives the synchronous enumeration that writes it.
        unsafe {
            let _ = EnumThreadWindows(
                GetCurrentThreadId(),
                Some(each),
                LPARAM(&mut found as *mut u32 as isize),
            );
        }
        found
    }
}

/// Call from `setup` (the main thread, after the event loop exists).
pub fn install() {
    #[cfg(windows)]
    {
        let n = imp::install();
        crate::measure::log(|| format!("session end: {n} event target(s) subclassed"));
        if n == 0 {
            eprintln!("reviewglass: session end: tao's event target not found");
        }
    }
}
