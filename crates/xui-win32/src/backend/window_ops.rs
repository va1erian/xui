#![forbid(unsafe_code)]

//! Window-level [`Backend`](xui_core::backend::Backend) bodies: create a
//! top-level window, and its title, enable state, native handle, capture and
//! modal loop. Split from `contract.rs` (which delegates to these) so both files
//! stay under the size limit.

use std::cell::Cell;
use std::rc::Rc;

use xui_core::backend::{
    BackendError, NativeWindowHandle, PlatformSpec, Result as BackendResult, WindowId,
};
use xui_core::image::Image;
use xui_core::{Rect, Theme};

use super::handler::{TopHandler, WindowShared};
use super::{BackendWindow, Win32Backend, chrome};
use crate::sys;
use crate::window::{Window, WindowClass, WindowExStyle, WindowStyle};

impl Win32Backend {
    /// Creates a top-level window; the body of
    /// [`Backend::open_window`](xui_core::backend::Backend::open_window).
    pub(super) fn open(&self, spec: &PlatformSpec) -> BackendResult<WindowId> {
        let id = WindowId::from_raw(Self::allocate(&self.next_window));
        let shared = WindowShared::new();
        let background = Theme::light().background;
        // The spec is in Dip and the window does not exist yet, so convert at
        // the process-wide system DPI; once it exists `dpi()` reads the
        // monitor it landed on.
        let dpi = sys::dpi::system_dpi();
        let bounds = Rect::new(
            0,
            0,
            spec.width.to_px(dpi).value(),
            spec.height.to_px(dpi).value(),
        );
        let class = WindowClass::register("xui.backend", background)
            .map_err(|_| BackendError::CreateFailed("window class"))?;
        let window = Window::create(
            class,
            None,
            WindowStyle::overlapped().clip_children(),
            WindowExStyle::new(),
            bounds,
            &spec.title,
            TopHandler::new(id, Rc::clone(&shared)),
        )
        .map_err(|_| BackendError::CreateFailed("window"))?;
        chrome::apply(spec, &window);
        window.show();
        self.windows.borrow_mut().insert(
            id.raw(),
            BackendWindow {
                window,
                shared,
                theme: Cell::new(Theme::light()),
            },
        );
        Ok(id)
    }

    /// Replaces a window's title.
    pub(super) fn set_title(&self, window: WindowId, title: &str) {
        if let Some(entry) = self.windows.borrow().get(&window.raw()) {
            let _ = entry.window.set_title(title);
        }
    }

    /// Enables or disables a whole window.
    pub(super) fn set_enabled(&self, window: WindowId, enabled: bool) {
        if let Some(hwnd) = self.window_hwnd(window) {
            sys::window::enable_window(hwnd, enabled);
        }
    }

    /// The window's native handle, if it is open.
    pub(super) fn native_handle(&self, window: WindowId) -> Option<NativeWindowHandle> {
        self.window_hwnd(window)
            .map(|hwnd| NativeWindowHandle::from_raw(hwnd.raw()))
    }

    /// Renders the window into an image, through WGC when the `wgc` feature is
    /// on and `PrintWindow` otherwise.
    pub(super) fn capture_image(&self, window: WindowId) -> BackendResult<Image> {
        let Some(hwnd) = self.window_hwnd(window) else {
            return Err(BackendError::Other("no such window".into()));
        };
        #[cfg(feature = "wgc")]
        let captured = crate::capture::capture_hwnd(hwnd)
            .map_err(|error| BackendError::Other(error.to_string()))?;
        #[cfg(not(feature = "wgc"))]
        let captured = {
            let rect = sys::window::window_rect(hwnd);
            let raw = sys::capture::capture(hwnd, rect.width(), rect.height())
                .map_err(|error| BackendError::Other(error.to_string()))?;
            crate::capture::RgbaImage {
                width: raw.width as u32,
                height: raw.height as u32,
                pixels: raw.pixels,
            }
        };
        Image::from_rgba(captured.width, captured.height, captured.pixels)
            .map_err(|error| BackendError::Other(error.to_string()))
    }

    /// Runs a nested message loop until `window` is destroyed.
    pub(super) fn run_modal_loop(&self, window: WindowId) -> BackendResult<()> {
        let Some(hwnd) = self.window_hwnd(window) else {
            return Err(BackendError::Other("no such window".into()));
        };
        crate::looper::run_modal(hwnd);
        Ok(())
    }
}
