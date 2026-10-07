//! A second GUI launch restores the existing window, including one hidden in the tray.

use std::ptr;
use std::time::Duration;

use anyhow::{Result, anyhow};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow,
};

pub struct InstanceGuard(HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // SAFETY: the guard owns the successful CreateMutexW handle.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn restore_existing_window() {
    let title = wide("QuotaKeeper");
    for _ in 0..10 {
        // SAFETY: null class and a valid, terminated title are accepted by FindWindowW.
        let hwnd = unsafe { FindWindowW(ptr::null(), title.as_ptr()) };
        if !hwnd.is_null() {
            // SAFETY: hwnd was returned by FindWindowW and remains owned by Windows.
            unsafe {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// None means another instance already owns the app mutex and has been restored.
pub fn acquire() -> Result<Option<InstanceGuard>> {
    let name = wide("Local\\TuiTokenMonitorGui");
    // SAFETY: the null security attributes and terminated name are valid Win32 arguments.
    let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return Err(anyhow!(
            "CreateMutexW failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: GetLastError is called immediately after CreateMutexW on this thread.
    let already_exists = unsafe { GetLastError() == ERROR_ALREADY_EXISTS };
    if already_exists {
        // SAFETY: the returned handle is valid but belongs to this unsuccessful launch.
        unsafe {
            CloseHandle(handle);
        }
        restore_existing_window();
        Ok(None)
    } else {
        Ok(Some(InstanceGuard(handle)))
    }
}
