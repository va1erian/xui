//! The system clipboard, as text.

use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// Closes the clipboard when dropped, so every exit path releases it.
struct Open;

impl Open {
    fn new() -> Option<Open> {
        // SAFETY: opens the clipboard for the current task; paired with the
        // `CloseClipboard` in `Drop`.
        unsafe { OpenClipboard(None) }.ok().map(|()| Open)
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: the clipboard was opened by `Open::new`.
        let _ = unsafe { CloseClipboard() };
    }
}

/// The clipboard's text, or `None` when it holds none.
pub(crate) fn get() -> Option<String> {
    let _open = Open::new()?;
    // SAFETY: the clipboard is open; the returned handle is owned by the
    // system, so it is locked, read and unlocked without being freed.
    unsafe {
        let handle = GetClipboardData(u32::from(CF_UNICODETEXT.0)).ok()?;
        let global = HGLOBAL(handle.0);
        let data = GlobalLock(global) as *const u16;
        if data.is_null() {
            return None;
        }
        let mut length = 0;
        while *data.add(length) != 0 {
            length += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(data, length));
        let _ = GlobalUnlock(global);
        Some(text)
    }
}

/// Replaces the clipboard's text.
pub(crate) fn set(text: &str) {
    let Some(_open) = Open::new() else {
        return;
    };
    let units: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    // SAFETY: the clipboard is open. The block is sized for `units`, filled
    // while locked, and handed to the system, which owns it on success.
    unsafe {
        let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2) else {
            return;
        };
        let target = GlobalLock(global) as *mut u16;
        if target.is_null() {
            return;
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), target, units.len());
        let _ = GlobalUnlock(global);
        let _ = EmptyClipboard();
        let _ = SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(global.0)));
    }
}
