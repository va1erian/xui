//! Applying a portable [`Cursor`] to the pointer, handled on `WM_SETCURSOR`.

use windows::Win32::UI::WindowsAndMessaging::{
    IDC_ARROW, IDC_HAND, IDC_IBEAM, LoadCursorW, SetCursor, WM_SETCURSOR,
};

use xui_core::backend::Cursor;

/// The `WM_SETCURSOR` message id.
pub(crate) const WM_SETCURSOR_ID: u32 = WM_SETCURSOR;

/// Shows `cursor` over the window whose `WM_SETCURSOR` is being handled.
pub(crate) fn apply(cursor: Cursor) {
    let resource = match cursor {
        Cursor::Default => IDC_ARROW,
        Cursor::Hand => IDC_HAND,
        Cursor::Text => IDC_IBEAM,
    };
    // SAFETY: a null module selects the shared system cursor, which stays valid
    // for the lifetime of the process.
    if let Ok(handle) = unsafe { LoadCursorW(None, resource) } {
        // SAFETY: `handle` is a shared system cursor; setting it affects only
        // the current cursor.
        unsafe {
            let _ = SetCursor(Some(handle));
        }
    }
}
