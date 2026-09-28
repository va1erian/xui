//! Common-control initialisation and the raw control-specific messages.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::HFONT;
use windows::Win32::UI::Controls::{
    ICC_BAR_CLASSES, ICC_LISTVIEW_CLASSES, ICC_STANDARD_CLASSES, ICC_TREEVIEW_CLASSES,
    INITCOMMONCONTROLSEX, INITCOMMONCONTROLSEX_ICC, InitCommonControlsEx,
};
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_SETFONT};

use crate::error::{Error, Result};
use crate::gdi::Font;
use crate::hwnd::Hwnd;

use super::raw_hwnd;

/// Registers the common-control classes (ListView, TreeView, status bar…).
pub(crate) fn init_common_controls() -> Result<()> {
    let flags = ICC_LISTVIEW_CLASSES.0
        | ICC_TREEVIEW_CLASSES.0
        | ICC_BAR_CLASSES.0
        | ICC_STANDARD_CLASSES.0;
    let classes = INITCOMMONCONTROLSEX {
        dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: INITCOMMONCONTROLSEX_ICC(flags),
    };
    // SAFETY: `classes` is fully initialised for the call.
    unsafe { InitCommonControlsEx(&classes) }
        .ok()
        .map_err(|_| Error::ControlsUnavailable)
}

pub(crate) fn send(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
    // SAFETY: only integer values are forwarded; the caller guarantees any
    // pointer in `lparam` points at a valid struct for the duration.
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

/// Gives a control a font (and asks it to repaint).
pub(crate) fn set_control_font(hwnd: Hwnd, font: HFONT) {
    send(hwnd, WM_SETFONT, font.0 as usize, 1);
}

/// Gives a control the shared UI font for `dpi`. Every control created through
/// `create_child` gets this, so no control is left on the stock font.
pub(crate) fn apply_ui_font(hwnd: Hwnd, dpi: u32) {
    if let Ok(font) = Font::shared_ui(dpi) {
        set_control_font(hwnd, font.raw());
    }
}
