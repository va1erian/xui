#![forbid(unsafe_code)]

//! The low-level child-window helpers [`Win32Backend`](crate::Win32Backend)
//! uses to host a native control for a portable [`NodeKind`](xui_core::widget)
//! (today, `Edit`'s real `EDIT` window).

use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::hwnd::Hwnd;
use crate::sys;

/// Creates a child control window from a system window class and returns its
/// safe handle.
pub(crate) fn create_child(
    name: &'static str,
    class: &str,
    parent: Hwnd,
    style: u32,
    ex_style: u32,
    id: usize,
    bounds: Rect,
) -> Result<Hwnd> {
    sys::control::init_common_controls()?;
    let hwnd = sys::window::create_control(class, style, ex_style, parent, id, bounds)
        .map(sys::hwnd_from)
        .map_err(|_| Error::CreateControl(name))?;
    sys::control::apply_ui_font(hwnd, sys::dpi::window_dpi(hwnd));
    Ok(hwnd)
}

thread_local! {
    /// Source of unique, non-zero control ids for this thread. The ids are
    /// internal only: notifications are routed by `HWND`, never by id.
    static NEXT_CONTROL_ID: std::cell::Cell<usize> = const { std::cell::Cell::new(1) };
}

/// The next unique control id, used purely to satisfy the Win32 child-id slot.
pub(crate) fn next_id() -> usize {
    NEXT_CONTROL_ID.with(|next| {
        let id = next.get();
        next.set(id.wrapping_add(1).max(1));
        id
    })
}

/// Standard child-window style bits, as raw values so the safe modules don't
/// need the `windows` crate.
pub(crate) mod style {
    pub(crate) const WS_CHILD: u32 = 0x4000_0000;
    pub(crate) const WS_VISIBLE: u32 = 0x1000_0000;
    pub(crate) const WS_BORDER: u32 = 0x0080_0000;
    pub(crate) const WS_TABSTOP: u32 = 0x0001_0000;
}
