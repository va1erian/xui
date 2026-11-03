//! The host fetcher's boundary: NetSurf's requests in (`fetch_start`,
//! `fetch_abort`) and the queued answers out (`nsx_fetch_*`), all on the
//! engine thread.

use std::ffi::{CStr, CString, c_char, c_void};

use netsurf_sys as ns;

use super::host::engine;
use crate::fetch::{FetchEvent, FetchMethod, FetchRequest};

/// Hands `http:` and `https:` URLs to the host fetcher from now on.
pub(crate) fn register_fetcher() -> bool {
    // SAFETY: called on the engine thread after `init`.
    unsafe { ns::nsx_fetch_register() == 0 }
}

/// Passes one queued answer for fetch `id` to NetSurf. Every call is made on
/// the engine thread, outside any NetSurf call, and the C side ignores an id
/// it no longer knows.
pub(crate) fn deliver(id: u64, event: FetchEvent) {
    match event {
        // SAFETY: plain values.
        FetchEvent::Status(code) => unsafe { ns::nsx_fetch_status(id, code.into()) },
        // SAFETY: the pointers and lengths describe live strings.
        FetchEvent::Header(name, value) => unsafe {
            ns::nsx_fetch_header(
                id,
                name.as_ptr().cast(),
                name.len(),
                value.as_ptr().cast(),
                value.len(),
            )
        },
        // SAFETY: the pointer and length describe a live buffer.
        FetchEvent::Data(bytes) => unsafe { ns::nsx_fetch_data(id, bytes.as_ptr(), bytes.len()) },
        // SAFETY: plain values.
        FetchEvent::Finish => unsafe { ns::nsx_fetch_finish(id) },
        FetchEvent::Fail(message) => {
            let message = CString::new(message.replace('\0', " ")).unwrap_or_default();
            // SAFETY: a NUL-terminated string that outlives the call.
            unsafe { ns::nsx_fetch_fail(id, message.as_ptr()) }
        }
    }
}

/// # Safety
/// `s` is a NUL-terminated string.
unsafe fn string(s: *const c_char) -> String {
    if s.is_null() {
        return String::new();
    }
    // SAFETY: the caller guarantees a NUL-terminated string.
    unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
}

/// # Safety
/// `r` is a valid request whose pointers live for the call.
unsafe fn request(r: &ns::nsx_request) -> FetchRequest {
    let headers = if r.headers.is_null() {
        &[][..]
    } else {
        // SAFETY: `header_count` string pointers at `headers`.
        unsafe { std::slice::from_raw_parts(r.headers, r.header_count) }
    };
    let headers = headers
        .iter()
        .filter_map(|&line| {
            // SAFETY: each is a NUL-terminated "Name: value" string.
            let line = unsafe { string(line) };
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect();
    let body = (!r.body.is_null()).then(|| {
        // SAFETY: `body_len` bytes at a non-null `body`.
        unsafe { std::slice::from_raw_parts(r.body, r.body_len) }.to_vec()
    });
    FetchRequest {
        // SAFETY: a NUL-terminated URL.
        url: unsafe { string(r.url) },
        method: match r.method {
            ns::NSX_METHOD_POST => FetchMethod::Post,
            ns::NSX_METHOD_HEAD => FetchMethod::Head,
            _ => FetchMethod::Get,
        },
        headers,
        body,
    }
}

pub(super) unsafe extern "C" fn host_fetch_start(ctx: *mut c_void, r: *const ns::nsx_request) {
    // SAFETY: NetSurf passes a valid request for the call.
    let r = unsafe { &*r };
    // SAFETY: as above.
    let req = unsafe { request(r) };
    // SAFETY: `ctx` is the engine.
    unsafe { engine(ctx) }.fetches.start(r.id, req);
}

pub(super) unsafe extern "C" fn host_fetch_abort(ctx: *mut c_void, id: u64) {
    // SAFETY: `ctx` is the engine.
    unsafe { engine(ctx) }.fetches.abort(id);
}
