//! The two Win32 calls the app makes outside Tauri: a message box when the
//! window can't open at all (no WebView2, say), and waiting for the previous
//! copy to exit when an update starts the new one.

#[cfg(windows)]
use windows::Win32::Foundation::CloseHandle;
#[cfg(windows)]
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
#[cfg(windows)]
use windows::core::HSTRING;

/// Shows `text` in an error box and waits for OK.
#[cfg(windows)]
pub fn error_box(title: &str, text: &str) {
    // SAFETY: MessageBoxW only reads the two strings, which HSTRING keeps
    // alive and NUL-terminated for the call; no owner window.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            &HSTRING::from(title),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Waits, at most `millis`, for process `pid` to exit.
#[cfg(windows)]
pub fn wait_for_exit(pid: u32, millis: u32) {
    // SAFETY: the handle comes from OpenProcess, is only waited on, and is
    // closed once; a process that already exited just fails to open.
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            let _ = WaitForSingleObject(handle, millis);
            let _ = CloseHandle(handle);
        }
    }
}

#[cfg(not(windows))]
pub fn error_box(title: &str, text: &str) {
    eprintln!("{title}: {text}");
}

#[cfg(not(windows))]
pub fn wait_for_exit(_pid: u32, _millis: u32) {}
