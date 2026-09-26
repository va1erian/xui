//! A themed one-pixel border for a native `EDIT`.
//!
//! `WS_EX_CLIENTEDGE` draws the classic sunken bevel, which is light whatever
//! the theme. The backend instead gives the edit a plain `WS_BORDER` and
//! subclasses its `WM_PAINT` to re-stroke the one-pixel frame with the theme's
//! `border` (or `border_focused` while it holds focus), the same border the
//! portable `Edit` paints on the canvas backend. The edit's text and background
//! keep coming from the central `WM_CTLCOLOREDIT` answer.
//!
//! The frame is part of the edit's client area (the `EDIT` class draws its
//! `WS_BORDER` itself), so it is over-painted after the control has painted,
//! not through `WM_NCPAINT` (which the class never sends).
//!
//! All `unsafe` in this crate lives under `sys`; every block below carries a
//! `// SAFETY:` note.
//!
//! ```text
//! install(hwnd, theme);          // on creation
//! set_theme(hwnd, new_theme);    // on a live theme switch
//! ```

use std::cell::RefCell;
use std::collections::HashMap;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    FrameRect, GetWindowDC, HDC, RDW_INVALIDATE, RDW_UPDATENOW, RedrawWindow, ReleaseDC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, WM_KILLFOCUS, WM_NCDESTROY, WM_PAINT, WM_SETFOCUS,
};

use crate::hwnd::Hwnd;
use crate::theme::Theme;

use super::{hwnd_from, raw_hwnd};

const EDGE_SUBCLASS_ID: usize = 0x7875_6564; // "xued"

thread_local! {
    /// The theme each subclassed edit's frame is stroked with, keyed by `HWND`.
    /// The subclass is stateless, so the state lives here and is dropped on
    /// `WM_NCDESTROY` (a `refdata` pointer would have to outlive the window).
    static EDGES: RefCell<HashMap<usize, Theme>> = RefCell::new(HashMap::new());
}

/// Gives the `WS_BORDER` edit `hwnd` a themed frame and records `theme`.
pub(crate) fn install(hwnd: Hwnd, theme: Theme) {
    EDGES.with(|map| {
        map.borrow_mut().insert(hwnd.raw(), theme);
    });
    // SAFETY: `edge_proc` keeps no state of its own (the per-window theme lives
    // in `EDGES`), so `refdata` is unused; the subclass removes itself on
    // `WM_NCDESTROY`, so nothing outlives the window. A failed install merely
    // leaves the default `WS_BORDER` and is ignored.
    unsafe {
        let _ = SetWindowSubclass(raw_hwnd(hwnd), Some(edge_proc), EDGE_SUBCLASS_ID, 0);
    }
}

/// Updates `hwnd`'s frame colour, repainting when it changed.
pub(crate) fn set_theme(hwnd: Hwnd, theme: Theme) {
    let changed = EDGES.with(|map| {
        let mut map = map.borrow_mut();
        match map.get_mut(&hwnd.raw()) {
            Some(slot) if *slot != theme => {
                *slot = theme;
                true
            }
            _ => false,
        }
    });
    if changed {
        repaint(hwnd);
    }
}

/// Invalidates and repaints `hwnd` so its frame is stroked with the new colour.
fn repaint(hwnd: Hwnd) {
    // SAFETY: `hwnd` is live; only this edit is invalidated and painted now, and
    // a stale handle makes the call a documented no-op.
    unsafe {
        let _ = RedrawWindow(
            Some(raw_hwnd(hwnd)),
            None,
            None,
            RDW_INVALIDATE | RDW_UPDATENOW,
        );
    }
}

/// Strokes `hwnd`'s one-pixel frame with the current theme colour. Called after
/// the control has painted its client, so it only overwrites the border.
fn paint(hwnd: HWND) {
    // SAFETY: the window DC is released below; a stale handle makes `GetWindowDC`
    // return an invalid DC, which is checked.
    unsafe {
        let dc = GetWindowDC(Some(hwnd));
        if dc.is_invalid() {
            return;
        }
        paint_into(hwnd, dc);
        let _ = ReleaseDC(Some(hwnd), dc);
    }
}

/// Strokes `hwnd`'s frame into `dc` with the theme colour for its focus state.
fn paint_into(hwnd: HWND, dc: HDC) {
    let window = hwnd_from(hwnd);
    let Some(theme) = EDGES.with(|map| map.borrow().get(&window.raw()).copied()) else {
        return;
    };
    // SAFETY: `GetFocus` only reads this thread's focus window and takes no
    // pointers.
    let color = if unsafe { GetFocus() } == hwnd {
        theme.border_focused
    } else {
        theme.border
    };
    let Some(brush) = crate::gdi::cache_brush(color) else {
        return;
    };
    let mut rect = RECT::default();
    // SAFETY: `hwnd` is live and `rect` is a valid out-pointer.
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return;
    }
    // The frame is the outermost pixel of the window, in window coordinates.
    let frame = RECT {
        left: 0,
        top: 0,
        right: rect.right - rect.left,
        bottom: rect.bottom - rect.top,
    };
    // SAFETY: `dc` is a live DC and `frame` is a valid rectangle for the call.
    unsafe {
        let _ = FrameRect(dc, &frame, brush);
    }
}

/// The subclass procedure installed by [`install`].
///
/// # Safety
/// Called by Windows for the subclass installed by `install`; the per-window
/// state lives in `EDGES` and is dropped on `WM_NCDESTROY`.
unsafe extern "system" fn edge_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _refdata: usize,
) -> LRESULT {
    match msg {
        // Let the control paint, then re-stroke the border it drew.
        WM_PAINT => {
            // SAFETY: forward to the subclass chain's original window procedure.
            let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            paint(hwnd);
            result
        }
        // Focus changes swap `border` for `border_focused`; repaint after the
        // edit has handled the focus move.
        WM_SETFOCUS | WM_KILLFOCUS => {
            // SAFETY: forward to the subclass chain's original window procedure.
            let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            repaint(hwnd_from(hwnd));
            result
        }
        WM_NCDESTROY => {
            EDGES.with(|map| map.borrow_mut().remove(&hwnd_from(hwnd).raw()));
            // SAFETY: removes this stateless subclass and forwards the
            // destruction to the original window procedure.
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(edge_proc), EDGE_SUBCLASS_ID);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
        }
        _ => {
            // SAFETY: forward to the subclass chain's original window procedure.
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, SelectObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WS_BORDER, WS_CHILD,
        WS_POPUP, WS_VISIBLE,
    };
    use windows::core::w;

    use super::*;

    const WIDTH: i32 = 120;
    const HEIGHT: i32 = 24;

    /// Renders `hwnd`'s frame into a zeroed 32-bpp DIB and returns its pixels
    /// (BGRA, one `u32` per pixel).
    fn render_frame(hwnd: HWND) -> Vec<u32> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: WIDTH,
                biHeight: -HEIGHT, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: a memory DC and a DIB section owned here; both are freed below
        // after the pixels have been copied out.
        unsafe {
            let dc = CreateCompatibleDC(None);
            let mut bits = std::ptr::null_mut();
            let bitmap =
                CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).expect("dib");
            let old = SelectObject(dc, bitmap.into());
            paint_into(hwnd, dc);
            let pixels =
                std::slice::from_raw_parts(bits as *const u32, (WIDTH * HEIGHT) as usize).to_vec();
            SelectObject(dc, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(dc);
            pixels
        }
    }

    /// The frame is stroked with the theme's border colour, not the unthemed
    /// `WS_BORDER` the class drew (the #110 defect).
    #[test]
    fn the_frame_follows_the_installed_theme() {
        // SAFETY: a hidden popup parent with a visible `WS_BORDER` `EDIT` child,
        // both destroyed below.
        let (parent, edit) = unsafe {
            let parent = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!(""),
                WINDOW_STYLE(WS_POPUP.0),
                -32000,
                -32000,
                200,
                60,
                None,
                None,
                None,
                None,
            )
            .expect("parent");
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                w!("hello"),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_BORDER.0),
                0,
                0,
                WIDTH,
                HEIGHT,
                Some(parent),
                None,
                None,
                None,
            )
            .expect("edit");
            (parent, edit)
        };
        install(hwnd_from(edit), Theme::dark());

        let pixels = render_frame(edit);
        let border = Theme::dark().border.to_colorref();
        assert_eq!(
            pixels[0] & 0x00FF_FFFF,
            border,
            "the top-left frame pixel must be the theme border"
        );
        assert_eq!(
            pixels[WIDTH as usize - 1] & 0x00FF_FFFF,
            border,
            "the top-right frame pixel must be the theme border"
        );

        // SAFETY: both windows were created above and are destroyed once; the
        // subclass removes itself on the edit's `WM_NCDESTROY`.
        unsafe {
            let _ = DestroyWindow(parent);
        }
    }
}
