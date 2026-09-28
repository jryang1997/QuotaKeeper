//! Windows login startup and an optional Ctrl+Alt+U global show/hide shortcut.

use std::ptr;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegCreateKeyExW,
    RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, RegisterHotKey, UnregisterHotKey,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{MSG, PM_REMOVE, PeekMessageW, WM_HOTKEY};

const RUN_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_NAME: &str = "TokenMonitorGui";
const HOTKEY_ID: i32 = 0x544d;

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open_run_key(run_path: &str, access: u32, create: bool) -> Result<Option<HKEY>> {
    let mut key: HKEY = ptr::null_mut();
    let path = wide(run_path);
    let status = if create {
        // SAFETY: all pointers refer to valid, terminated strings or output storage.
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                ptr::null(),
                0,
                access,
                ptr::null(),
                &mut key,
                ptr::null_mut(),
            )
        }
    } else {
        // SAFETY: path and output storage remain valid for the call.
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, access, &mut key) }
    };
    if status == ERROR_FILE_NOT_FOUND && !create {
        return Ok(None);
    }
    if status != 0 {
        return Err(anyhow!("Windows Run registry key: error {status}"));
    }
    Ok(Some(key))
}

pub fn autostart_enabled() -> Result<bool> {
    autostart_enabled_at(RUN_PATH, RUN_NAME)
}

fn autostart_enabled_at(run_path: &str, run_name: &str) -> Result<bool> {
    let Some(key) = open_run_key(run_path, KEY_QUERY_VALUE, false)? else {
        return Ok(false);
    };
    let name = wide(run_name);
    let mut value_type = 0;
    let mut size = 0;
    // SAFETY: key is opened and output locations are valid; a null data pointer queries size.
    let status = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            ptr::null(),
            &mut value_type,
            ptr::null_mut(),
            &mut size,
        )
    };
    // SAFETY: key was opened above.
    unsafe {
        RegCloseKey(key);
    }
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    if status != 0 {
        return Err(anyhow!("Windows Run registry query: error {status}"));
    }
    Ok(value_type == REG_SZ && size > 2)
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    set_autostart_at(RUN_PATH, RUN_NAME, enabled)
}

fn set_autostart_at(run_path: &str, run_name: &str, enabled: bool) -> Result<()> {
    let Some(key) = open_run_key(run_path, KEY_SET_VALUE, enabled)? else {
        return Ok(());
    };
    let name = wide(run_name);
    let status = if enabled {
        let path = std::env::current_exe()?;
        let command = wide(&format!("\"{}\"", path.display()));
        // SAFETY: data is a valid UTF-16 REG_SZ including a terminating NUL.
        unsafe {
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                command.as_ptr() as *const u8,
                (command.len() * 2) as u32,
            )
        }
    } else {
        // SAFETY: key and terminated name are valid.
        unsafe { RegDeleteValueW(key, name.as_ptr()) }
    };
    // SAFETY: key was opened above.
    unsafe {
        RegCloseKey(key);
    }
    if status != 0 && !(status == ERROR_FILE_NOT_FOUND && !enabled) {
        return Err(anyhow!("Windows Run registry write: error {status}"));
    }
    Ok(())
}

/// One worker owns the hotkey registration and message queue for the whole process.
pub fn global_shortcut_worker(
    on_hotkey: impl Fn() + Send + 'static,
    on_status: impl Fn(bool) + Send + 'static,
) -> mpsc::Sender<bool> {
    let (tx, rx) = mpsc::channel::<bool>();
    std::thread::spawn(move || {
        let mut registered = false;
        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(enabled) if enabled && !registered => {
                    // SAFETY: null HWND registers a thread hotkey; this worker pumps its queue.
                    registered = unsafe {
                        RegisterHotKey(
                            ptr::null_mut(),
                            HOTKEY_ID,
                            MOD_CONTROL | MOD_ALT,
                            b'U' as u32,
                        ) != 0
                    };
                    on_status(registered);
                }
                Ok(false) if registered => {
                    // SAFETY: this worker owns the registration.
                    unsafe {
                        UnregisterHotKey(ptr::null_mut(), HOTKEY_ID);
                    }
                    registered = false;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                _ => {}
            }
            let mut message: MSG = unsafe { std::mem::zeroed() };
            // SAFETY: message is writable; the thread owns its queue.
            while unsafe { PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
                if message.message == WM_HOTKEY && message.wParam == HOTKEY_ID as usize {
                    on_hotkey();
                }
            }
        }
        if registered {
            // SAFETY: this worker owns the registration.
            unsafe {
                UnregisterHotKey(ptr::null_mut(), HOTKEY_ID);
            }
        }
    });
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_registry_roundtrip_uses_isolated_value() {
        let name = format!("TokenMonitorGuiTest-{}", std::process::id());
        assert!(!autostart_enabled_at(RUN_PATH, &name).unwrap());
        set_autostart_at(RUN_PATH, &name, true).unwrap();
        assert!(autostart_enabled_at(RUN_PATH, &name).unwrap());
        set_autostart_at(RUN_PATH, &name, false).unwrap();
        assert!(!autostart_enabled_at(RUN_PATH, &name).unwrap());
    }
}
