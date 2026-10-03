//! Raw `EDIT` control messages the portable `Edit` node needs beyond
//! `WM_GETTEXT`/`WM_SETTEXT` (handled generically by `sys::window`).

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Controls::{EM_SETCUEBANNER, EM_SETPASSWORDCHAR};
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

/// The mask a password edit shows per character: U+2022 BULLET, the glyph the
/// portable `Edit` paints, so both backends look alike.
const PASSWORD_CHAR: u16 = 0x2022;

/// Masks the edit's text (`EM_SETPASSWORDCHAR` with a mask character, which
/// adds `ES_PASSWORD`) or shows it again (a zero character removes the style).
/// A masked `EDIT` refuses `WM_COPY` and `WM_CUT` itself, so the text cannot
/// leave through the clipboard. The control redraws its visible text itself.
pub(crate) fn set_password(hwnd: Hwnd, password: bool) {
    let mask = if password { PASSWORD_CHAR } else { 0 };
    send(hwnd, EM_SETPASSWORDCHAR, usize::from(mask), 0);
}
