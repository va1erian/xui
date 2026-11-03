//! Moving a window by dragging a client region.
//!
//! A custom (frameless) title bar marks one of its nodes as a drag region; a
//! left press there starts the same modal move loop the system caption uses.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{HTCAPTION, SendMessageW, WM_NCLBUTTONDOWN};

use crate::hwnd::Hwnd;

use super::raw_hwnd;

/// Starts a window move, as if the user had dragged the caption: releases any
/// mouse capture, then sends the non-client button-down that puts the window
/// into the system's modal move loop. `hwnd` is the top-level window; the call
/// returns once the user releases the mouse.
pub(crate) fn begin_move(hwnd: Hwnd) {
    // SAFETY: `hwnd` is a live top-level window. `WM_NCLBUTTONDOWN` with
    // `HTCAPTION` is the documented way to start a caption drag; the system
    // owns the modal loop it runs and the parameters are plain integers.
    unsafe {
        let _ = ReleaseCapture();
        let _ = SendMessageW(
            raw_hwnd(hwnd),
            WM_NCLBUTTONDOWN,
            Some(WPARAM(HTCAPTION as usize)),
            Some(LPARAM(0)),
        );
    }
}
