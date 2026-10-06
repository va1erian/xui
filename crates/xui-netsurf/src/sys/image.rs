//! The host callbacks behind NetSurf's PNG, JPEG and SVG handler. The decoders
//! are safe Rust, but a panic must not unwind into C, so each call stops one
//! and reports "no image".

use std::ffi::{c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::image;

/// # Safety
/// `data` is null or points at `len` readable bytes.
unsafe fn bytes<'a>(data: *const u8, len: usize) -> &'a [u8] {
    if data.is_null() || len == 0 {
        return &[];
    }
    // SAFETY: the caller guarantees `len` bytes at a non-null `data`.
    unsafe { std::slice::from_raw_parts(data, len) }
}

pub(super) unsafe extern "C" fn host_image_size(
    _ctx: *mut c_void,
    data: *const u8,
    len: usize,
    width: *mut c_int,
    height: *mut c_int,
    raster_width: *mut c_int,
    raster_height: *mut c_int,
) -> c_int {
    // SAFETY: NetSurf passes the content's source data, `len` bytes long.
    let data = unsafe { bytes(data, len) };
    let sizes = catch_unwind(|| image::sizes(data)).ok().flatten();
    let int = |(w, h): (u32, u32)| Some((c_int::try_from(w).ok()?, c_int::try_from(h).ok()?));
    let Some(((w, h), (rw, rh))) = sizes.and_then(|(l, r)| Some((int(l)?, int(r)?))) else {
        return 0;
    };
    // SAFETY: NetSurf passes writable out pointers.
    unsafe {
        *width = w;
        *height = h;
        *raster_width = rw;
        *raster_height = rh;
    }
    1
}

pub(super) unsafe extern "C" fn host_image_decode(
    _ctx: *mut c_void,
    data: *const u8,
    len: usize,
    pixels: *mut u8,
    width: c_int,
    height: c_int,
    opaque: *mut c_int,
) -> c_int {
    let (Ok(w), Ok(h)) = (u32::try_from(width), u32::try_from(height)) else {
        return 0;
    };
    if pixels.is_null() || !(1..=image::MAX_PIXELS).contains(&(u64::from(w) * u64::from(h))) {
        return 0;
    }
    // SAFETY: NetSurf passes the content's source data, `len` bytes long.
    let data = unsafe { bytes(data, len) };
    // SAFETY: the bitmap NetSurf passes is `width * height * 4` bytes (its
    // row stride is checked to be `width * 4`), and nothing else touches it
    // during the call; the size was bounded above.
    let out = unsafe { std::slice::from_raw_parts_mut(pixels, w as usize * h as usize * 4) };
    let decoded = catch_unwind(AssertUnwindSafe(|| image::decode(data, w, h, out)));
    match decoded {
        Ok(Some(is_opaque)) => {
            // SAFETY: NetSurf passes a writable out pointer.
            unsafe { *opaque = c_int::from(is_opaque) };
            1
        }
        _ => 0,
    }
}
