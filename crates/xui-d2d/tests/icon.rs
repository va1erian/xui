#![cfg(windows)]
// `native` reads a window's icons back through Win32, the one place this test
// needs `unsafe`, so it is confined to that module.

//! A window's icon on the backend: the app's icon reaches the title bar and
//! task bar at the window's DPI, is reloaded when the DPI changes, and the
//! window's native handle is exposed.
//!
//! No test enters the message loop: windows are created hidden and every
//! message here is sent synchronously, so there is nothing to hang on.

use xui_core::Image;
use xui_core::backend::{Backend, PlatformSpec, WindowIcon, WindowId};
use xui_d2d::D2dBackend;

mod native {
    #![allow(unsafe_code)]

    use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{BITMAP, DeleteObject, GetObjectW};
    use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetIconInfo, HICON, ICON_BIG, ICON_SMALL, ICONINFO, IsWindow, SM_CXSMICON, SendMessageW,
        WM_DPICHANGED, WM_GETICON,
    };

    fn hwnd(raw: usize) -> HWND {
        HWND(raw as *mut _)
    }

    /// Whether `raw` names a live window.
    pub fn is_window(raw: usize) -> bool {
        // SAFETY: a query that accepts any handle value.
        unsafe { IsWindow(Some(hwnd(raw))).as_bool() }
    }

    /// The width of the window's big or small icon, `None` when it has none
    /// of its own.
    pub fn icon_width(raw: usize, small: bool) -> Option<i32> {
        let kind = if small { ICON_SMALL } else { ICON_BIG };
        // SAFETY: `WM_GETICON` returns a handle the window keeps; its bitmaps
        // are copies this function reads and deletes.
        unsafe {
            let icon = SendMessageW(hwnd(raw), WM_GETICON, Some(WPARAM(kind as usize)), None);
            if icon.0 == 0 {
                return None;
            }
            let mut info = ICONINFO::default();
            GetIconInfo(HICON(icon.0 as *mut _), &mut info).ok()?;
            let mut bitmap = BITMAP::default();
            GetObjectW(
                info.hbmColor.into(),
                size_of::<BITMAP>() as i32,
                Some(&mut bitmap as *mut BITMAP as *mut _),
            );
            let _ = DeleteObject(info.hbmColor.into());
            let _ = DeleteObject(info.hbmMask.into());
            Some(bitmap.bmWidth)
        }
    }

    /// The small icon size Windows uses at `dpi`.
    pub fn small_icon_size(dpi: u32) -> i32 {
        // SAFETY: a plain metric query.
        unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) }
    }

    /// Tells the window its DPI changed to `dpi`, as Windows does when it
    /// moves to another monitor.
    pub fn change_dpi(raw: usize, dpi: u32) {
        let suggested = RECT {
            left: 0,
            top: 0,
            right: 400,
            bottom: 300,
        };
        let wparam = (dpi as usize) << 16 | dpi as usize;
        // SAFETY: `suggested` outlives the synchronous send, as the message's
        // contract requires of its RECT pointer.
        unsafe {
            SendMessageW(
                hwnd(raw),
                WM_DPICHANGED,
                Some(WPARAM(wparam)),
                Some(LPARAM(&suggested as *const RECT as isize)),
            )
        };
    }
}

fn open(backend: &D2dBackend) -> (WindowId, usize) {
    backend.init();
    let window = backend
        .open_window(&PlatformSpec::new("xui icon test"))
        .expect("window");
    let raw = backend.native_window(window).expect("an HWND").raw();
    (window, raw)
}

fn picture(side: u32) -> WindowIcon {
    WindowIcon::image(Image::from_rgba(side, side, vec![0x80; (side * side * 4) as usize]).unwrap())
}

#[test]
fn native_window_is_the_windows_hwnd() {
    let backend = D2dBackend::new();
    let (window, raw) = open(&backend);
    assert!(native::is_window(raw), "the handle names a live window");
    backend.close_window(window);
    assert!(backend.native_window(window).is_none(), "a closed window");
    assert!(!native::is_window(raw), "the window was destroyed");
}

#[test]
fn an_image_icon_is_shown_at_the_windows_icon_sizes() {
    let backend = D2dBackend::new();
    let (window, raw) = open(&backend);
    // The test executable has no icon resource, so no icon of its own yet.
    assert_eq!(native::icon_width(raw, true), None);

    backend.set_window_icon(window, &picture(256));
    let dpi = backend.dpi(window);
    assert_eq!(
        native::icon_width(raw, true),
        Some(native::small_icon_size(dpi)),
        "the image was resampled to the small icon size"
    );
    assert!(native::icon_width(raw, false).is_some(), "a big icon too");
    backend.close_window(window);
}

#[test]
fn the_icon_is_reloaded_when_the_dpi_changes() {
    let backend = D2dBackend::new();
    let (window, raw) = open(&backend);
    backend.set_window_icon(window, &picture(64));
    let dpi = if backend.dpi(window) == 192 { 96 } else { 192 };
    native::change_dpi(raw, dpi);
    assert_eq!(
        native::icon_width(raw, true),
        Some(native::small_icon_size(dpi)),
        "the small icon matches the new DPI"
    );
    backend.close_window(window);
}

#[test]
fn a_missing_resource_leaves_the_current_icon() {
    let backend = D2dBackend::new();
    let (window, raw) = open(&backend);
    backend.set_window_icon(window, &picture(32));
    let before = native::icon_width(raw, true);
    backend.set_window_icon(window, &WindowIcon::resource(0x7FFF));
    assert_eq!(native::icon_width(raw, true), before);
    backend.close_window(window);
}
