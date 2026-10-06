//! The host callbacks NetSurf makes for the life of the engine: text
//! measuring, window notifications and built-in resources.

use std::ffi::{c_char, c_int, c_void};

use netsurf_sys as ns;

use super::convert::{font, text};
use crate::engine::{Engine, WinState};
use crate::fonts::FontReq;

/// # Safety
/// `ctx` is the `&'static Engine` given to `init`.
pub(super) unsafe fn engine<'a>(ctx: *mut c_void) -> &'a Engine {
    // SAFETY: the context is the leaked engine, valid forever.
    unsafe { &*(ctx as *const Engine) }
}

/// # Safety
/// `win` is a live `WinState` (see the module docs).
unsafe fn win<'a>(win: *mut c_void) -> &'a WinState {
    // SAFETY: the caller guarantees the window state is alive.
    unsafe { &*(win as *const WinState) }
}

pub(super) unsafe extern "C" fn host_text_width(
    ctx: *mut c_void,
    f: *const ns::nsx_font,
    s: *const c_char,
    len: usize,
) -> c_int {
    // SAFETY: NetSurf passes a valid font and `len` bytes at `s`.
    let (req, s) = unsafe { (font(f), text(s, len)) };
    // SAFETY: `ctx` is the engine.
    unsafe { engine(ctx) }.fonts.width(&req, &s)
}

/// # Safety
/// As for the layout callbacks: valid font, `len` bytes at `s`, valid outs.
unsafe fn measure_into(
    ctx: *mut c_void,
    f: *const ns::nsx_font,
    s: *const c_char,
    len: usize,
    offset: *mut usize,
    actual_x: *mut c_int,
    measure: impl FnOnce(&Engine, &FontReq, &str) -> (usize, i32),
) {
    // SAFETY: NetSurf passes a valid font and `len` bytes at `s`.
    let (req, s) = unsafe { (font(f), text(s, len)) };
    // SAFETY: `ctx` is the engine.
    let (at, x) = measure(unsafe { engine(ctx) }, &req, &s);
    // A lossy conversion can lengthen the text; never point past the input.
    // SAFETY: NetSurf passes writable out pointers.
    unsafe {
        *offset = at.min(len);
        *actual_x = x;
    }
}

pub(super) unsafe extern "C" fn host_text_position(
    ctx: *mut c_void,
    f: *const ns::nsx_font,
    s: *const c_char,
    len: usize,
    x: c_int,
    offset: *mut usize,
    actual_x: *mut c_int,
) {
    // SAFETY: forwarded from NetSurf's layout call.
    unsafe {
        measure_into(ctx, f, s, len, offset, actual_x, |e, r, t| {
            e.fonts.position(r, t, x)
        })
    }
}

pub(super) unsafe extern "C" fn host_text_split(
    ctx: *mut c_void,
    f: *const ns::nsx_font,
    s: *const c_char,
    len: usize,
    x: c_int,
    offset: *mut usize,
    actual_x: *mut c_int,
) {
    // SAFETY: forwarded from NetSurf's layout call.
    unsafe {
        measure_into(ctx, f, s, len, offset, actual_x, |e, r, t| {
            e.fonts.split(r, t, x)
        })
    }
}

pub(super) unsafe extern "C" fn host_win_invalidate(_ctx: *mut c_void, w: *mut c_void) {
    // SAFETY: NetSurf only reports for live windows.
    unsafe { win(w) }.invalidate();
}

pub(super) unsafe extern "C" fn host_win_event(_ctx: *mut c_void, w: *mut c_void, event: c_int) {
    // SAFETY: NetSurf only reports for live windows.
    unsafe { win(w) }.event(event);
}

pub(super) unsafe extern "C" fn host_win_title(
    _ctx: *mut c_void,
    w: *mut c_void,
    title: *const c_char,
) {
    // SAFETY: a NUL-terminated title from NetSurf, for a live window.
    let title = unsafe { std::ffi::CStr::from_ptr(title) }.to_string_lossy();
    // SAFETY: NetSurf only reports for live windows.
    unsafe { win(w) }.title(&title);
}

pub(super) unsafe extern "C" fn host_win_url(
    _ctx: *mut c_void,
    w: *mut c_void,
    url: *const c_char,
) {
    // SAFETY: a NUL-terminated URL from NetSurf, for a live window.
    let url = unsafe { std::ffi::CStr::from_ptr(url) }.to_string_lossy();
    // SAFETY: NetSurf only reports for live windows.
    unsafe { win(w) }.url(&url);
}

pub(super) unsafe extern "C" fn host_win_size(
    _ctx: *mut c_void,
    w: *mut c_void,
    width: *mut c_int,
    height: *mut c_int,
) {
    // SAFETY: NetSurf only asks for live windows.
    let (vw, vh) = unsafe { win(w) }.size();
    // SAFETY: NetSurf passes writable out pointers.
    unsafe {
        *width = vw;
        *height = vh;
    }
}

pub(super) unsafe extern "C" fn host_win_pointer(_ctx: *mut c_void, w: *mut c_void, shape: c_int) {
    // SAFETY: NetSurf only reports for live windows.
    unsafe { win(w) }.pointer(shape);
}

pub(super) unsafe extern "C" fn host_resource(
    _ctx: *mut c_void,
    path: *const c_char,
    data: *mut *const u8,
    len: *mut usize,
) -> c_int {
    // SAFETY: a NUL-terminated resource name from NetSurf.
    let path = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy();
    match ns::RESOURCES.iter().find(|(name, _)| *name == path) {
        Some((_, bytes)) => {
            // SAFETY: writable out pointers; the bytes are 'static.
            unsafe {
                *data = bytes.as_ptr();
                *len = bytes.len();
            }
            1
        }
        None => 0,
    }
}

pub(super) unsafe extern "C" fn host_win_status(
    _ctx: *mut c_void,
    w: *mut c_void,
    text: *const c_char,
) {
    // SAFETY: a NUL-terminated text from NetSurf, for a live window.
    let text = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy();
    // SAFETY: NetSurf only reports for live windows.
    unsafe { win(w) }.status(&text);
}

pub(super) unsafe extern "C" fn host_launch_url(
    ctx: *mut c_void,
    w: *mut c_void,
    url: *const c_char,
) {
    // SAFETY: a NUL-terminated URL from NetSurf.
    let url = unsafe { std::ffi::CStr::from_ptr(url) }.to_string_lossy();
    if w.is_null() {
        // SAFETY: `ctx` is the engine.
        let last = unsafe { engine(ctx) }.last_window.borrow().clone();
        match last {
            Some(view) => view.send(crate::engine::Output::Launch(url.into_owned())),
            None => log::info!("xui-netsurf: no window to launch {url} from"),
        }
    } else {
        // SAFETY: the active window's handle, live for the host call.
        unsafe { win(w) }.launch(&url);
    }
}
