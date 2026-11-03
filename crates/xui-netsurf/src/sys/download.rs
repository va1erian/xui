//! The download callbacks NetSurf makes, and the cancel call.

use std::ffi::{CStr, c_char, c_int, c_void};

use netsurf_sys as ns;

use super::host::engine;
use crate::download::{DownloadId, DownloadInfo};
use crate::engine::WinState;

/// # Safety
/// `s` is null or a NUL-terminated string.
unsafe fn string(s: *const c_char) -> String {
    if s.is_null() {
        return String::new();
    }
    // SAFETY: the caller guarantees a NUL-terminated string.
    unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
}

#[allow(clippy::too_many_arguments)]
pub(super) unsafe extern "C" fn host_dl_start(
    ctx: *mut c_void,
    w: *mut c_void,
    id: u64,
    url: *const c_char,
    filename: *const c_char,
    mime: *const c_char,
    total: u64,
) -> c_int {
    // SAFETY: NetSurf passes NUL-terminated strings (or null).
    let (url, filename, mime) = unsafe { (string(url), string(filename), string(mime)) };
    let info = DownloadInfo {
        id: DownloadId(id),
        url,
        filename,
        mime,
        total: (total > 0).then_some(total),
    };
    // SAFETY: a non-null `w` is a live window's state.
    let view = (!w.is_null()).then(|| unsafe { &*(w as *const WinState) }.reporter());
    // SAFETY: `ctx` is the engine.
    c_int::from(unsafe { engine(ctx) }.downloads.start(info, view))
}

pub(super) unsafe extern "C" fn host_dl_data(
    ctx: *mut c_void,
    id: u64,
    data: *const u8,
    len: usize,
) -> c_int {
    let bytes = if data.is_null() || len == 0 {
        &[][..]
    } else {
        // SAFETY: NetSurf passes `len` readable bytes at `data` for the call.
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    // SAFETY: `ctx` is the engine.
    c_int::from(unsafe { engine(ctx) }.downloads.data(id, bytes))
}

pub(super) unsafe extern "C" fn host_dl_end(ctx: *mut c_void, id: u64, error: *const c_char) {
    // SAFETY: null, or a NUL-terminated message from NetSurf.
    let error = (!error.is_null()).then(|| unsafe { string(error) });
    // SAFETY: `ctx` is the engine.
    unsafe { engine(ctx) }.downloads.end(id, error);
}

/// Stops download `id`; its end is reported through `host_dl_end`.
pub(crate) fn cancel_download(id: u64) {
    // SAFETY: called on the engine thread; an unknown id is ignored.
    unsafe { ns::nsx_download_cancel(id) }
}
