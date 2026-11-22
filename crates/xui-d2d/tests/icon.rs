#![cfg(windows)]
// `native` reads a window's icons back through Win32, the one place this test
// needs `unsafe`, so it is confined to that module.

//! A window's icon on the backend: the executable's icon shows with no app
//! code, the app's icon reaches the title bar and task bar at the window's DPI
//! and is reloaded when the DPI changes, and the window's native handle is
//! exposed.
//!
//! On MSVC the build script links an icon group (id 1, 16 and 32 px) into
//! every test binary, so the resource tests have an executable icon to find.
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
        GCLP_HICON, GetClassLongPtrW, GetIconInfo, HICON, ICON_BIG, ICON_SMALL, ICONINFO, IsWindow,
        SM_CXICON, SM_CXSMICON, SendMessageW, WM_DPICHANGED, WM_GETICON,
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

    /// The big and small icon sizes Windows uses at `dpi`.
    pub fn icon_sizes(dpi: u32) -> (i32, i32) {
        // SAFETY: plain metric queries.
        unsafe {
            (
                GetSystemMetricsForDpi(SM_CXICON, dpi),
                GetSystemMetricsForDpi(SM_CXSMICON, dpi),
            )
        }
    }

    /// Whether the window's class carries an icon.
    pub fn class_has_icon(raw: usize) -> bool {
        // SAFETY: a class query on a live window.
        unsafe { GetClassLongPtrW(hwnd(raw), GCLP_HICON) != 0 }
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

/// Asserts that the window shows its own big and small icons at the sizes of
/// `dpi`.
fn assert_icons_at(raw: usize, dpi: u32) {
    let (big, small) = native::icon_sizes(dpi);
    assert_eq!(
        native::icon_width(raw, false),
        Some(big),
        "big icon at {dpi}"
    );
    assert_eq!(
        native::icon_width(raw, true),
        Some(small),
        "small icon at {dpi}"
    );
}

/// A DPI the window is not at, to move it to.
fn other_dpi(backend: &D2dBackend, window: WindowId) -> u32 {
    if backend.dpi(window) == 192 { 96 } else { 192 }
}

#[cfg(target_env = "msvc")]
#[test]
fn a_window_shows_the_executables_icon_with_no_app_code() {
    let backend = D2dBackend::new();
    let (window, raw) = open(&backend);
    assert!(native::class_has_icon(raw), "the class carries the icon");
    assert_icons_at(raw, backend.dpi(window));
    let dpi = other_dpi(&backend, window);
    native::change_dpi(raw, dpi);
    assert_icons_at(raw, dpi);
    backend.close_window(window);
}

#[cfg(target_env = "msvc")]
#[test]
fn a_resource_icon_loads_at_the_windows_dpi_and_follows_it() {
    let backend = D2dBackend::new();
    let (window, raw) = open(&backend);
    backend.set_window_icon(window, &picture(64));
    backend.set_window_icon(window, &WindowIcon::resource(1));
    assert_icons_at(raw, backend.dpi(window));
    let dpi = other_dpi(&backend, window);
    native::change_dpi(raw, dpi);
    assert_icons_at(raw, dpi);
    backend.close_window(window);
}

#[test]
fn an_image_icon_is_shown_at_the_windows_icon_sizes() {
    let backend = D2dBackend::new();
    let (window, raw) = open(&backend);
    backend.set_window_icon(window, &picture(256));
    assert_icons_at(raw, backend.dpi(window));
    backend.close_window(window);
}

#[test]
fn the_icon_is_reloaded_when_the_dpi_changes() {
    let backend = D2dBackend::new();
    let (window, raw) = open(&backend);
    backend.set_window_icon(window, &picture(64));
    let dpi = other_dpi(&backend, window);
    native::change_dpi(raw, dpi);
    assert_icons_at(raw, dpi);
    backend.close_window(window);
}

#[test]
fn a_missing_resource_leaves_the_current_icon_and_its_dpi_reload() {
    let backend = D2dBackend::new();
    let (window, raw) = open(&backend);
    backend.set_window_icon(window, &picture(32));
    backend.set_window_icon(window, &WindowIcon::resource(0x7FFF));
    assert_icons_at(raw, backend.dpi(window));
    // The image the window kept, not the missing resource, follows the DPI.
    let dpi = other_dpi(&backend, window);
    native::change_dpi(raw, dpi);
    assert_icons_at(raw, dpi);
    backend.close_window(window);
}
