//! The only `unsafe` in the crate: the `netsurf-sys` calls and the C callbacks
//! NetSurf makes into the engine. Everything here runs on the engine thread.
//!
//! Pointers handed to C:
//! - the host context is the `&'static Engine` (it is leaked, so it outlives
//!   NetSurf);
//! - fetches and images are named by value (an id, the source bytes for one
//!   call), never by a pointer kept on the Rust side;
//! - each window's handle is its `Box<WinState>`, which the engine keeps until
//!   after `nsx_window_destroy` (and the C side stops reporting to a window
//!   once destroy starts);
//! - a redraw's sink context is a `&mut Recorder` that lives for that call.

use std::ffi::{CString, c_void};
use std::sync::OnceLock;

use netsurf_sys as ns;

use crate::engine::{Engine, MouseAction, WinState};
use crate::record::Recorder;

mod convert;
mod download;
mod fetch;
mod host;
mod image;
mod sink;

pub(crate) use download::cancel_download;
use download::{host_dl_data, host_dl_end, host_dl_start};
pub(crate) use fetch::{deliver, register_fetcher};
use fetch::{host_fetch_abort, host_fetch_start};
use image::{host_image_decode, host_image_size};

use host::{
    host_launch_url, host_resource, host_text_position, host_text_split, host_text_width,
    host_win_event, host_win_invalidate, host_win_pointer, host_win_size, host_win_status,
    host_win_title, host_win_url,
};
use sink::{
    sink_bitmap, sink_clip, sink_disc, sink_line, sink_opacity, sink_polygon, sink_rect, sink_text,
};

pub(crate) const EVENT_UPDATE_EXTENT: i32 = ns::NSX_EVENT_UPDATE_EXTENT;
pub(crate) const EVENT_START_THROBBER: i32 = ns::NSX_EVENT_START_THROBBER;
pub(crate) const EVENT_STOP_THROBBER: i32 = ns::NSX_EVENT_STOP_THROBBER;
pub(crate) const EVENT_NEW_CONTENT: i32 = ns::NSX_EVENT_NEW_CONTENT;

/// The host table, which NetSurf keeps for the life of the process.
struct HostTable(ns::nsx_host);
// SAFETY: the table is only read by NetSurf on the engine thread; the context
// pointer it carries is an immutable `&'static Engine`.
unsafe impl Send for HostTable {}
// SAFETY: as above; nothing mutates the table after initialisation.
unsafe impl Sync for HostTable {}

static HOST: OnceLock<HostTable> = OnceLock::new();

/// Starts NetSurf with `engine` answering its callbacks.
pub(crate) fn init(engine: &'static Engine) -> Result<(), String> {
    let table = HOST.get_or_init(|| {
        HostTable(ns::nsx_host {
            ctx: engine as *const Engine as *mut c_void,
            text_width: host_text_width,
            text_position: host_text_position,
            text_split: host_text_split,
            win_invalidate: host_win_invalidate,
            win_event: host_win_event,
            win_title: host_win_title,
            win_url: host_win_url,
            win_size: host_win_size,
            win_pointer: host_win_pointer,
            resource: host_resource,
            fetch_start: host_fetch_start,
            fetch_abort: host_fetch_abort,
            image_size: host_image_size,
            image_decode: host_image_decode,
            win_status: host_win_status,
            launch_url: host_launch_url,
            dl_start: host_dl_start,
            dl_data: host_dl_data,
            dl_end: host_dl_end,
        })
    });
    // SAFETY: the table and the message bytes are 'static; this is the
    // engine thread, which makes every later NetSurf call.
    let ret = unsafe { ns::nsx_init(&table.0, ns::MESSAGES.as_ptr(), ns::MESSAGES.len()) };
    if ret == 0 {
        Ok(())
    } else {
        Err("nsx_init failed".to_string())
    }
}

/// Runs NetSurf's due timers; the ms until the next one, or -1.
pub(crate) fn poll() -> i32 {
    // SAFETY: called on the engine thread after `init`.
    unsafe { ns::nsx_poll() }
}

/// A NetSurf browser window.
pub(crate) struct Window(*mut ns::gui_window);

impl Window {
    /// Opens `url`, reporting through `state` (which must stay where it is
    /// until [`Window::destroy`]).
    pub(crate) fn open(state: &WinState, url: &str) -> Option<Window> {
        let url = CString::new(url).ok()?;
        // SAFETY: `state` is boxed by the engine and outlives the window; the
        // URL is a valid C string for the call.
        let gw =
            unsafe { ns::nsx_window_create(state as *const WinState as *mut c_void, url.as_ptr()) };
        (!gw.is_null()).then_some(Window(gw))
    }

    pub(crate) fn navigate(&self, url: &str) -> bool {
        let Ok(url) = CString::new(url) else {
            return false;
        };
        // SAFETY: `self.0` is a live window; the URL lives for the call.
        unsafe { ns::nsx_window_navigate(self.0, url.as_ptr()) == 0 }
    }

    pub(crate) fn stop(&self) {
        // SAFETY: `self.0` is a live window.
        unsafe { ns::nsx_window_stop(self.0) }
    }

    pub(crate) fn download(&self, url: &str) -> bool {
        let Ok(url) = CString::new(url) else {
            return false;
        };
        // SAFETY: `self.0` is a live window; the URL lives for the call.
        unsafe { ns::nsx_window_download(self.0, url.as_ptr()) == 0 }
    }

    pub(crate) fn reformat(&self) {
        // SAFETY: `self.0` is a live window.
        unsafe { ns::nsx_window_reformat(self.0) }
    }

    /// The document size, once there is content.
    pub(crate) fn extent(&self) -> Option<(i32, i32)> {
        let (mut w, mut h) = (0, 0);
        // SAFETY: `self.0` is a live window; the out pointers are locals.
        let ok = unsafe { ns::nsx_window_extent(self.0, &mut w, &mut h) };
        (ok != 0).then_some((w, h))
    }

    pub(crate) fn ready(&self) -> bool {
        // SAFETY: `self.0` is a live window.
        unsafe { ns::nsx_window_ready(self.0) != 0 }
    }

    /// Draws the document area `(x0, y0, x1, y1)` into `rec`.
    pub(crate) fn redraw(&self, rec: &mut Recorder<'_>, area: (i32, i32, i32, i32)) {
        let sink = ns::nsx_sink {
            rec: rec as *mut Recorder<'_> as *mut c_void,
            clip: sink_clip,
            rect: sink_rect,
            line: sink_line,
            disc: sink_disc,
            polygon: sink_polygon,
            bitmap: sink_bitmap,
            text: sink_text,
            opacity: sink_opacity,
        };
        // SAFETY: `self.0` is a live window; `sink` and the recorder it points
        // at live for the call, and NetSurf drops the pointer when it returns.
        unsafe { ns::nsx_window_redraw(self.0, area.0, area.1, area.2, area.3, &sink) };
    }

    pub(crate) fn mouse(&self, action: MouseAction, x: i32, y: i32) {
        let action = match action {
            MouseAction::Move => ns::NSX_MOUSE_MOVE,
            MouseAction::Press => ns::NSX_MOUSE_PRESS,
            MouseAction::Click => ns::NSX_MOUSE_CLICK,
            MouseAction::Release => ns::NSX_MOUSE_RELEASE,
        };
        // SAFETY: `self.0` is a live window.
        unsafe { ns::nsx_window_mouse(self.0, action, x, y) }
    }

    pub(crate) fn key(&self, key: u32) {
        // SAFETY: `self.0` is a live window.
        unsafe { ns::nsx_window_key(self.0, key) };
    }

    /// Closes the window; NetSurf stops reporting to its state first.
    pub(crate) fn destroy(self) {
        // SAFETY: `self.0` is live and is not used again (consumed).
        unsafe { ns::nsx_window_destroy(self.0) }
    }
}
