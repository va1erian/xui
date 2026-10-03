//! The drawing callbacks of one redraw, forwarded to its [`Recorder`].

use std::ffi::{c_char, c_int, c_uint, c_void};

use netsurf_sys as ns;
use xui_litehtml::{Point, Rect};

use super::convert::{font, rect, style, text};
use crate::record::{BitmapPixels, Recorder, argb};

/// # Safety
/// `rec` is the `&mut Recorder` `Window::redraw` passed for this call.
unsafe fn recorder<'a, 'b>(rec: *mut c_void) -> &'a mut Recorder<'b> {
    // SAFETY: the caller guarantees the recorder is alive and unaliased.
    unsafe { &mut *(rec as *mut Recorder<'b>) }
}

pub(super) unsafe extern "C" fn sink_clip(
    rec: *mut c_void,
    x0: c_int,
    y0: c_int,
    x1: c_int,
    y1: c_int,
) {
    // SAFETY: called within `Window::redraw`.
    unsafe { recorder(rec) }.clip(rect(x0, y0, x1, y1));
}

pub(super) unsafe extern "C" fn sink_rect(
    rec: *mut c_void,
    x0: c_int,
    y0: c_int,
    x1: c_int,
    y1: c_int,
    s: *const ns::nsx_style,
) {
    // SAFETY: called within `Window::redraw` with a valid style.
    unsafe { recorder(rec).rect(rect(x0, y0, x1, y1), &style(s)) }
}

pub(super) unsafe extern "C" fn sink_line(
    rec: *mut c_void,
    x0: c_int,
    y0: c_int,
    x1: c_int,
    y1: c_int,
    s: *const ns::nsx_style,
) {
    let (a, b) = (
        Point::new(x0 as f32, y0 as f32),
        Point::new(x1 as f32, y1 as f32),
    );
    // SAFETY: called within `Window::redraw` with a valid style.
    unsafe { recorder(rec).line(a, b, &style(s)) }
}

pub(super) unsafe extern "C" fn sink_disc(
    rec: *mut c_void,
    x: c_int,
    y: c_int,
    radius: c_int,
    s: *const ns::nsx_style,
) {
    let c = Point::new(x as f32, y as f32);
    // SAFETY: called within `Window::redraw` with a valid style.
    unsafe { recorder(rec).disc(c, radius as f32, &style(s)) }
}

pub(super) unsafe extern "C" fn sink_polygon(
    rec: *mut c_void,
    p: *const c_int,
    n: c_uint,
    s: *const ns::nsx_style,
) {
    if p.is_null() {
        return;
    }
    // SAFETY: NetSurf passes `n` x,y pairs at `p`.
    let coords = unsafe { std::slice::from_raw_parts(p, n as usize * 2) };
    let points = coords
        .chunks_exact(2)
        .map(|c| Point::new(c[0] as f32, c[1] as f32))
        .collect();
    // SAFETY: called within `Window::redraw` with a valid style.
    unsafe { recorder(rec).polygon(points, &style(s)) }
}

#[allow(clippy::too_many_arguments)]
pub(super) unsafe extern "C" fn sink_bitmap(
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
) {
    if rgba.is_null() || bw <= 0 || bh <= 0 || stride < bw as usize * 4 {
        return;
    }
    // SAFETY: the bitmap's buffer holds `bh` rows of `stride` bytes.
    let pixels = unsafe { std::slice::from_raw_parts(rgba, stride * bh as usize) };
    let px = BitmapPixels {
        id: id as usize,
        generation,
        width: bw as u32,
        height: bh as u32,
        stride,
        rgba: pixels,
    };
    let dest = Rect::from_min_size(x as f32, y as f32, w as f32, h as f32);
    // SAFETY: called within `Window::redraw`.
    unsafe { recorder(rec) }.bitmap(&px, dest, (repeat_x != 0, repeat_y != 0));
}

pub(super) unsafe extern "C" fn sink_text(
    rec: *mut c_void,
    f: *const ns::nsx_font,
    x: c_int,
    y: c_int,
    s: *const c_char,
    len: usize,
    colour: u32,
) {
    // SAFETY: NetSurf passes a valid font and `len` bytes at `s`.
    let (req, s) = unsafe { (font(f), text(s, len)) };
    // SAFETY: called within `Window::redraw`.
    unsafe { recorder(rec) }.text(&req, x as f32, y as f32, &s, argb(colour));
}
