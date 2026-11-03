//! The system clipboard, as text.

use std::thread::sleep;
use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::CF_UNICODETEXT;

use super::message_window::MessageWindow;

/// How often opening is retried while another process holds the clipboard.
const OPEN_ATTEMPTS: u32 = 5;

/// Closes the clipboard when dropped, so every exit path releases it.
struct Open;

thread_local! {
    // The thread's clipboard owner; see [`MessageWindow`].
    static OWNER: Option<MessageWindow> = MessageWindow::new();
}

impl Open {
    fn new() -> Option<Open> {
        let owner = OWNER.with(|owner| owner.as_ref().map(MessageWindow::raw));
        for attempt in 0..OPEN_ATTEMPTS {
            // SAFETY: opens the clipboard for this thread's owner window; paired
            // with the `CloseClipboard` in `Drop`.
            if unsafe { OpenClipboard(owner) }.is_ok() {
                return Some(Open);
            }
            if attempt + 1 < OPEN_ATTEMPTS {
                sleep(Duration::from_millis(10));
            }
        }
        None
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
    // system, so it is locked, read and unlocked without being freed. The read
    // is bounded by the block's own size, so text another process left without
    // a terminating NUL cannot be read past its end.
    unsafe {
        let handle = GetClipboardData(u32::from(CF_UNICODETEXT.0)).ok()?;
        let global = HGLOBAL(handle.0);
        let capacity = GlobalSize(global) / size_of::<u16>();
        let data = GlobalLock(global) as *const u16;
        if data.is_null() {
            return None;
        }
        let units = std::slice::from_raw_parts(data, capacity);
        let length = units.iter().position(|unit| *unit == 0).unwrap_or(capacity);
        let text = String::from_utf16_lossy(&units[..length]);
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
    // while locked, and handed to the system, which owns it only when
    // `SetClipboardData` succeeds; on every earlier or failing path it is freed
    // here.
    unsafe {
        let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, units.len() * size_of::<u16>()) else {
            return;
        };
        let target = GlobalLock(global) as *mut u16;
        if target.is_null() {
            let _ = GlobalFree(Some(global));
            return;
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), target, units.len());
        let _ = GlobalUnlock(global);
        let _ = EmptyClipboard();
        if SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(global.0))).is_err() {
            let _ = GlobalFree(Some(global));
        }
    }
}
