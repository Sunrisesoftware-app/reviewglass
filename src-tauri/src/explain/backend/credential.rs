//! The remote backend's API key, in Windows Credential Manager and nowhere else (spec
//! 6.3): never in the settings file, the repository or a log, and never handed back to
//! the page - the page only learns whether a key is stored.
//!
//! A generic credential under one target name, persisted for this user on this machine.

/// The credential's target name in Windows Credential Manager.
pub const TARGET: &str = "ReviewGlass/explain-remote";

/// Store the key, replacing any stored one.
pub fn write(key: &str) -> Result<(), String> {
    write_at(TARGET, key)
}

/// The stored key, if any.
pub fn read() -> Option<String> {
    read_at(TARGET)
}

/// Remove the stored key; removing none is not an error.
pub fn delete() -> Result<(), String> {
    delete_at(TARGET)
}

pub fn stored() -> bool {
    read().is_some_and(|k| !k.trim().is_empty())
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn write_at(target: &str, key: &str) -> Result<(), String> {
    use windows::core::PWSTR;
    use windows::Win32::Security::Credentials::{
        CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };
    let key = key.trim();
    if key.is_empty() {
        return Err("An empty key is not stored.".into());
    }
    let mut target_w = wide(target);
    let mut user_w = wide("ReviewGlass");
    let mut blob = key.as_bytes().to_vec();
    let cred = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target_w.as_mut_ptr()),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: PWSTR(user_w.as_mut_ptr()),
        ..Default::default()
    };
    // SAFETY: every pointer in `cred` points into a buffer that outlives the call.
    let r = unsafe { CredWriteW(&cred, 0) };
    blob.iter_mut().for_each(|b| *b = 0);
    r.map_err(|e| format!("Windows Credential Manager refused the key: {e}"))
}

#[cfg(windows)]
fn read_at(target: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Security::Credentials::{
        CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
    };
    let target_w = wide(target);
    let mut p: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: CredReadW allocates the credential it returns; it is read, copied and freed
    // with CredFree before this function returns.
    unsafe {
        CredReadW(PCWSTR(target_w.as_ptr()), CRED_TYPE_GENERIC, None, &mut p).ok()?;
        if p.is_null() {
            return None;
        }
        let c = &*p;
        let bytes = if c.CredentialBlob.is_null() {
            Vec::new()
        } else {
            std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec()
        };
        CredFree(p as *const core::ffi::c_void);
        String::from_utf8(bytes).ok()
    }
}

#[cfg(windows)]
fn delete_at(target: &str) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_NOT_FOUND;
    use windows::Win32::Security::Credentials::{CredDeleteW, CRED_TYPE_GENERIC};
    let target_w = wide(target);
    // SAFETY: a plain call with a NUL-terminated name.
    match unsafe { CredDeleteW(PCWSTR(target_w.as_ptr()), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(()),
        Err(e) if e.code() == ERROR_NOT_FOUND.to_hresult() => Ok(()),
        Err(e) => Err(format!(
            "Windows Credential Manager could not remove the key: {e}"
        )),
    }
}

#[cfg(not(windows))]
fn write_at(_target: &str, _key: &str) -> Result<(), String> {
    Err("Keys are stored in Windows Credential Manager, which exists only on Windows.".into())
}

#[cfg(not(windows))]
fn read_at(_target: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
fn delete_at(_target: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Writes a throwaway credential under its own name, reads it back, removes it.
    /// Ignored in the suite: it touches the user's credential store.
    ///
    ///   cargo test --lib credential_round_trip -- --ignored
    #[test]
    #[ignore]
    fn credential_round_trip() {
        let target = format!("ReviewGlass/test-{}", std::process::id());
        assert!(read_at(&target).is_none());
        write_at(&target, "  sk-test-123 ").unwrap();
        assert_eq!(read_at(&target).as_deref(), Some("sk-test-123"));
        write_at(&target, "sk-test-456").unwrap();
        assert_eq!(read_at(&target).as_deref(), Some("sk-test-456"), "replaced");
        delete_at(&target).unwrap();
        assert!(read_at(&target).is_none());
        delete_at(&target).unwrap();
        assert!(write_at(&target, "  ").is_err());
    }
}
