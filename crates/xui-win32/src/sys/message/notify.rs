//! Decoding of `WM_NOTIFY` notifications.

use windows::Win32::Foundation::LPARAM;

use crate::message::Notify;

use super::{hwnd_from, notify_header};

/// Decodes a `WM_NOTIFY` into the typed [`Notify`] struct.
pub(crate) fn decode_notify(lparam: LPARAM) -> Notify {
    match notify_header(lparam) {
        Some((from, id, code)) => Notify {
            id,
            code,
            hwnd: hwnd_from(from),
        },
        None => Notify {
            id: 0,
            code: 0,
            hwnd: crate::Hwnd::NULL,
        },
    }
}

#[cfg(test)]
mod tests;
