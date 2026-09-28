//! Optional OpenCode Go API key owned by this application.
//! Windows stores it as a generic Credential Manager entry, never in settings.json.

const GO_TARGET: &str = "TuiTokenMonitor/OpenCodeGo";
const GOAT_TARGET: &str = "TuiTokenMonitor/CommandCodeGoat";

pub fn valid_api_key(key: &str) -> bool {
    let trimmed = key.trim();
    !trimmed.is_empty() && trimmed.len() <= 2048 && !trimmed.chars().any(char::is_whitespace)
}

#[cfg(windows)]
mod win {
    use std::ptr;

    use anyhow::{Result, anyhow};
    use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, GetLastError};
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
        CredReadW, CredWriteW,
    };

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub(super) fn read(target: &str) -> Result<Option<String>> {
        let target = wide(target);
        let mut pointer: *mut CREDENTIALW = ptr::null_mut();
        // SAFETY: target is NUL-terminated and pointer is valid output storage.
        let ok = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) };
        if ok == 0 {
            // SAFETY: GetLastError is called immediately after the failing Win32 call.
            let error = unsafe { GetLastError() };
            return if error == ERROR_NOT_FOUND {
                Ok(None)
            } else {
                Err(anyhow!("Windows Credential Manager read failed: {error}"))
            };
        }
        // SAFETY: CredReadW returned an allocated CREDENTIALW. Blob points to its storage.
        let key = if unsafe { (*pointer).CredentialBlobSize } == 0
            || unsafe { (*pointer).CredentialBlob.is_null() }
        {
            Err(anyhow!("Stored API key is empty"))
        } else {
            // SAFETY: CredReadW returned a non-null blob with the reported byte length.
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    (*pointer).CredentialBlob,
                    (*pointer).CredentialBlobSize as usize,
                )
            };
            String::from_utf8(bytes.to_vec()).map_err(|_| anyhow!("Stored API key is not UTF-8"))
        };
        // SAFETY: pointer was allocated by CredReadW and is freed exactly once.
        unsafe { CredFree(pointer.cast()) };
        key.map(Some)
    }

    pub(super) fn write(target: &str, key: &str) -> Result<()> {
        if !super::valid_api_key(key) {
            return Err(anyhow!("Invalid OpenCode Go API key format"));
        }
        let mut name = wide(target);
        let mut blob = key.as_bytes().to_vec();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: name.as_mut_ptr(),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        // SAFETY: all buffers remain valid for the call; Credential Manager copies them.
        let ok = unsafe { CredWriteW(&credential, 0) };
        blob.fill(0);
        if ok == 0 {
            return Err(anyhow!("Windows Credential Manager write failed"));
        }
        Ok(())
    }

    pub(super) fn delete(target: &str) -> Result<()> {
        let name = wide(target);
        // SAFETY: target is NUL-terminated and the type matches write().
        let ok = unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok == 0 {
            // SAFETY: GetLastError is called immediately after the failing Win32 call.
            let error = unsafe { GetLastError() };
            if error != ERROR_NOT_FOUND {
                return Err(anyhow!("Windows Credential Manager delete failed: {error}"));
            }
        }
        Ok(())
    }
}

pub fn open_code_go_key() -> anyhow::Result<Option<String>> {
    #[cfg(windows)]
    {
        win::read(GO_TARGET)
    }
    #[cfg(not(windows))]
    {
        Ok(None)
    }
}

pub fn set_open_code_go_key(key: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        win::write(GO_TARGET, key)
    }
    #[cfg(not(windows))]
    {
        let _ = key;
        anyhow::bail!("Windows Credential Manager unavailable")
    }
}

pub fn delete_open_code_go_key() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        win::delete(GO_TARGET)
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

pub fn has_open_code_go_key() -> bool {
    open_code_go_key().ok().flatten().is_some()
}

pub fn command_code_key() -> anyhow::Result<Option<String>> {
    #[cfg(windows)]
    {
        win::read(GOAT_TARGET)
    }
    #[cfg(not(windows))]
    {
        Ok(None)
    }
}

pub fn set_command_code_key(key: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        win::write(GOAT_TARGET, key)
    }
    #[cfg(not(windows))]
    {
        let _ = key;
        anyhow::bail!("Windows Credential Manager unavailable")
    }
}

pub fn delete_command_code_key() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        win::delete(GOAT_TARGET)
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

pub fn has_command_code_key() -> bool {
    command_code_key().ok().flatten().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_validation_rejects_blank_and_header_controls() {
        assert!(valid_api_key("sk-test_123"));
        assert!(!valid_api_key("  "));
        assert!(!valid_api_key("key\r\nOther: injected"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_vault_roundtrip_is_isolated() {
        let target = format!("{GO_TARGET}/test-{}", std::process::id());
        win::write(&target, "test-only-key").unwrap();
        assert_eq!(
            win::read(&target).unwrap().as_deref(),
            Some("test-only-key")
        );
        win::delete(&target).unwrap();
        assert!(win::read(&target).unwrap().is_none());
    }
}
