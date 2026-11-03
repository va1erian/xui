//! Plain values from NetSurf's C types.

use std::ffi::{c_char, c_int};

use netsurf_sys as ns;
use xui_litehtml::Rect;

use crate::fonts::FontReq;
use crate::record::{PlotKind, Style, argb};

/// # Safety
/// `s` must point at `len` readable bytes (or be null with `len` 0).
pub(super) unsafe fn text<'a>(s: *const c_char, len: usize) -> std::borrow::Cow<'a, str> {
    if s.is_null() || len == 0 {
        return std::borrow::Cow::Borrowed("");
    }
    // SAFETY: the caller guarantees `len` readable bytes at `s`.
    let bytes = unsafe { std::slice::from_raw_parts(s.cast::<u8>(), len) };
    String::from_utf8_lossy(bytes)
}

/// # Safety
/// `f` must point at a valid `nsx_font` whose family bytes are readable.
pub(super) unsafe fn font(f: *const ns::nsx_font) -> FontReq {
    // SAFETY: the caller guarantees `f` is valid for the call.
    let f = unsafe { &*f };
    // SAFETY: `family`/`family_len` describe an lwc string NetSurf holds.
    let named = unsafe { text(f.family, f.family_len) };
    FontReq {
        family: FontReq::family_for(Some(&named), f.generic),
        size: f.size_px,
        weight: f.weight.clamp(100, 900) as u16,
        italic: f.italic != 0,
    }
}

/// # Safety
/// `s` must point at a valid `nsx_style`.
pub(super) unsafe fn style(s: *const ns::nsx_style) -> Style {
    // SAFETY: the caller guarantees `s` is valid for the call.
    let s = unsafe { &*s };
    Style {
        fill_kind: PlotKind::from_raw(s.fill_kind),
        fill: argb(s.fill),
        stroke_kind: PlotKind::from_raw(s.stroke_kind),
        stroke: argb(s.stroke),
        stroke_width: s.stroke_width,
    }
}

pub(super) fn rect(x0: c_int, y0: c_int, x1: c_int, y1: c_int) -> Rect {
    Rect::new(x0 as f32, y0 as f32, x1 as f32, y1 as f32)
}
