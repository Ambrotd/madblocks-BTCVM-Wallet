//! Copying the private key for backup, the way password managers do: to the
//! clipboard, marked so Windows keeps it out of clipboard history (Win+V)
//! and cloud sync and tells clipboard monitors to skip it, and cleared after
//! a minute unless something else has been copied since.

use std::time::Duration;
use windows::Win32::Foundation::{GlobalFree, HANDLE};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardSequenceNumber, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::core::w;
use zeroize::Zeroizing;

/// How long a copied key stays on the clipboard.
pub const CLEAR_AFTER: Duration = Duration::from_secs(60);

pub fn copy_secret(text: &str) -> windows::core::Result<()> {
    let utf16: Zeroizing<Vec<u8>> = Zeroizing::new(
        text.encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect(),
    );
    // SAFETY: the clipboard is opened and closed here, on this thread; each
    // block handed to SetClipboardData is a fresh, unlocked GlobalAlloc
    // block, which the system owns once the call succeeds.
    unsafe {
        OpenClipboard(None)?;
        let filled = (|| {
            EmptyClipboard()?;
            set(u32::from(CF_UNICODETEXT.0), &utf16)?;
            let off = 0u32.to_le_bytes();
            set(
                RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")),
                &off,
            )?;
            set(
                RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
                &off,
            )?;
            set(
                RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")),
                &off,
            )
        })();
        let _ = CloseClipboard();
        filled?;
    }
    // SAFETY: a plain query.
    let ours = unsafe { GetClipboardSequenceNumber() };
    std::thread::spawn(move || {
        std::thread::sleep(CLEAR_AFTER);
        // SAFETY: as above; only clears if the clipboard is still ours.
        unsafe {
            if GetClipboardSequenceNumber() == ours && OpenClipboard(None).is_ok() {
                let _ = EmptyClipboard();
                let _ = CloseClipboard();
            }
        }
    });
    Ok(())
}

/// Puts `data` on the open clipboard under `format`.
///
/// # Safety
/// The clipboard must be open on this thread.
unsafe fn set(format: u32, data: &[u8]) -> windows::core::Result<()> {
    unsafe {
        let block = GlobalAlloc(GMEM_MOVEABLE, data.len())?;
        let ptr = GlobalLock(block).cast::<u8>();
        if ptr.is_null() {
            let _ = GlobalFree(Some(block));
            return Err(windows::core::Error::from_thread());
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        let _ = GlobalUnlock(block);
        if let Err(e) = SetClipboardData(format, Some(HANDLE(block.0))) {
            let _ = GlobalFree(Some(block));
            return Err(e);
        }
        Ok(())
    }
}
