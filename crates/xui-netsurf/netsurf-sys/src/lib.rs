//! Raw bindings to `csrc/nsx.h`: the NetSurf browser core behind a flat C
//! interface of plain values. See that header for each function's contract;
//! `xui-netsurf` is the safe layer.
//!
//! Every function here, and every callback NetSurf makes, must stay on the one
//! thread that called [`nsx_init`]: the core keeps global state.

#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_uint, c_void};

// The core's gzip support; listed so the archive is linked.
use libz_sys as _;

/// A font as NetSurf asks for it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct nsx_font {
    /// The first family the CSS named (not NUL terminated), or null.
    pub family: *const c_char,
    /// Length of `family` in bytes.
    pub family_len: usize,
    /// 0 sans-serif, 1 serif, 2 monospace, 3 cursive, 4 fantasy.
    pub generic: c_int,
    /// The em size in CSS pixels.
    pub size_px: f32,
    /// 100 to 900.
    pub weight: c_int,
    /// Italic or oblique.
    pub italic: c_int,
}

/// No stroke or fill.
pub const NSX_PLOT_NONE: c_int = 0;
/// A solid stroke or fill.
pub const NSX_PLOT_SOLID: c_int = 1;
/// A dotted stroke.
pub const NSX_PLOT_DOT: c_int = 2;
/// A dashed stroke.
pub const NSX_PLOT_DASH: c_int = 3;

/// A plot style; colours are straight `0xAARRGGBB`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct nsx_style {
    /// One of the `NSX_PLOT_*` kinds.
    pub fill_kind: c_int,
    /// The fill colour.
    pub fill: u32,
    /// One of the `NSX_PLOT_*` kinds.
    pub stroke_kind: c_int,
    /// The stroke colour.
    pub stroke: u32,
    /// The stroke width in CSS pixels.
    pub stroke_width: f32,
}

/// The document's size changed.
pub const NSX_EVENT_UPDATE_EXTENT: c_int = 1;
/// A load started.
pub const NSX_EVENT_START_THROBBER: c_int = 3;
/// A load finished.
pub const NSX_EVENT_STOP_THROBBER: c_int = 4;
/// A new page replaced the old one.
pub const NSX_EVENT_NEW_CONTENT: c_int = 6;

/// The pointer moved.
pub const NSX_MOUSE_MOVE: c_int = 0;
/// The primary button went down.
pub const NSX_MOUSE_PRESS: c_int = 1;
/// The primary button came up without a drag (a click).
pub const NSX_MOUSE_CLICK: c_int = 2;
/// The primary button came up after a drag.
pub const NSX_MOUSE_RELEASE: c_int = 3;

/// What the host provides for the engine's life (`nsx_host` in `nsx.h`).
#[repr(C)]
pub struct nsx_host {
    pub ctx: *mut c_void,
    pub text_width: unsafe extern "C" fn(
        ctx: *mut c_void,
        f: *const nsx_font,
        s: *const c_char,
        len: usize,
    ) -> c_int,
    pub text_position: unsafe extern "C" fn(
        ctx: *mut c_void,
        f: *const nsx_font,
        s: *const c_char,
        len: usize,
        x: c_int,
        offset: *mut usize,
        actual_x: *mut c_int,
    ),
    pub text_split: unsafe extern "C" fn(
        ctx: *mut c_void,
        f: *const nsx_font,
        s: *const c_char,
        len: usize,
        x: c_int,
        offset: *mut usize,
        actual_x: *mut c_int,
    ),
    pub win_invalidate: unsafe extern "C" fn(ctx: *mut c_void, win: *mut c_void),
    pub win_event: unsafe extern "C" fn(ctx: *mut c_void, win: *mut c_void, event: c_int),
    pub win_title: unsafe extern "C" fn(ctx: *mut c_void, win: *mut c_void, title: *const c_char),
    pub win_url: unsafe extern "C" fn(ctx: *mut c_void, win: *mut c_void, url: *const c_char),
    pub win_size: unsafe extern "C" fn(
        ctx: *mut c_void,
        win: *mut c_void,
        width: *mut c_int,
        height: *mut c_int,
    ),
    pub win_pointer: unsafe extern "C" fn(ctx: *mut c_void, win: *mut c_void, shape: c_int),
    pub resource: unsafe extern "C" fn(
        ctx: *mut c_void,
        path: *const c_char,
        data: *mut *const u8,
        len: *mut usize,
    ) -> c_int,
}

/// Where one redraw's drawing goes (`nsx_sink` in `nsx.h`).
#[repr(C)]
pub struct nsx_sink {
    pub rec: *mut c_void,
    pub clip: unsafe extern "C" fn(rec: *mut c_void, x0: c_int, y0: c_int, x1: c_int, y1: c_int),
    pub rect: unsafe extern "C" fn(
        rec: *mut c_void,
        x0: c_int,
        y0: c_int,
        x1: c_int,
        y1: c_int,
        s: *const nsx_style,
    ),
    pub line: unsafe extern "C" fn(
        rec: *mut c_void,
        x0: c_int,
        y0: c_int,
        x1: c_int,
        y1: c_int,
        s: *const nsx_style,
    ),
    pub disc: unsafe extern "C" fn(
        rec: *mut c_void,
        x: c_int,
        y: c_int,
        radius: c_int,
        s: *const nsx_style,
    ),
    pub polygon:
        unsafe extern "C" fn(rec: *mut c_void, p: *const c_int, n: c_uint, s: *const nsx_style),
    pub bitmap: unsafe extern "C" fn(
        rec: *mut c_void,
        id: *const c_void,
        generation: u32,
        rgba: *const u8,
        bw: c_int,
        bh: c_int,
        stride: usize,
        x: c_int,
        y: c_int,
        w: c_int,
        h: c_int,
        repeat_x: c_int,
        repeat_y: c_int,
    ),
    pub text: unsafe extern "C" fn(
        rec: *mut c_void,
        f: *const nsx_font,
        x: c_int,
        y: c_int,
        s: *const c_char,
        len: usize,
        colour: u32,
    ),
}

/// An opaque NetSurf browser window.
#[repr(C)]
pub struct gui_window {
    _private: [u8; 0],
}

unsafe extern "C" {
    pub fn nsx_init(host: *const nsx_host, messages: *const u8, len: usize) -> c_int;
    pub fn nsx_poll() -> c_int;
    pub fn nsx_fini();
    pub fn nsx_window_create(win: *mut c_void, url: *const c_char) -> *mut gui_window;
    pub fn nsx_window_navigate(gw: *mut gui_window, url: *const c_char) -> c_int;
    pub fn nsx_window_destroy(gw: *mut gui_window);
    pub fn nsx_window_reformat(gw: *mut gui_window);
    pub fn nsx_window_extent(gw: *mut gui_window, width: *mut c_int, height: *mut c_int) -> c_int;
    pub fn nsx_window_ready(gw: *mut gui_window) -> c_int;
    pub fn nsx_window_redraw(
        gw: *mut gui_window,
        x0: c_int,
        y0: c_int,
        x1: c_int,
        y1: c_int,
        sink: *const nsx_sink,
    ) -> c_int;
    pub fn nsx_window_mouse(gw: *mut gui_window, action: c_int, x: c_int, y: c_int);
    pub fn nsx_window_key(gw: *mut gui_window, key: u32) -> c_int;
}

/// NetSurf's user-agent stylesheet, quirks-mode and internal stylesheets and
/// ad-block list, which the core fetches as `resource:` URLs.
pub const RESOURCES: &[(&str, &[u8])] = &[
    (
        "default.css",
        include_bytes!("../vendor/netsurf/resources/default.css"),
    ),
    (
        "quirks.css",
        include_bytes!("../vendor/netsurf/resources/quirks.css"),
    ),
    (
        "internal.css",
        include_bytes!("../vendor/netsurf/resources/internal.css"),
    ),
    (
        "adblock.css",
        include_bytes!("../vendor/netsurf/resources/adblock.css"),
    ),
];

/// NetSurf's English message table, split from `resources/FatMessages`.
pub const MESSAGES: &[u8] = include_bytes!("../generated/resources/Messages");
