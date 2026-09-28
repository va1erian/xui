//! Raw `EDIT` control messages the portable `Edit` node needs beyond
//! `WM_GETTEXT`/`WM_SETTEXT` (handled generically by `sys::window`).

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Controls::EM_SETCUEBANNER;
use windows::Win32::UI::WindowsAndMessaging::SendMessageW;

use crate::hwnd::Hwnd;

use super::raw_hwnd;

fn send(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
    // SAFETY: only integer values are forwarded; the caller guarantees any
    // pointer in `lparam` points at a valid buffer for the duration.
    unsafe {
        SendMessageW(
            raw_hwnd(hwnd),
            msg,
            Some(WPARAM(wparam)),
            Some(LPARAM(lparam)),
        )
        .0
    }
}

/// Shows `text` as grey placeholder text while the edit is empty.
pub(crate) fn set_cue(hwnd: Hwnd, text: &str, show_when_focused: bool) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` is a nul-terminated UTF-16 buffer alive across the call.
    send(
        hwnd,
        EM_SETCUEBANNER,
        usize::from(show_when_focused),
        wide.as_ptr() as isize,
    );
}
