//! A `winit` window made the child of another, whose handles a GPU API may
//! take on the thread that renders. `winit` hands its handles out only on the
//! event-loop thread, and leaves parenting to an `unsafe` attribute because it
//! cannot check the parent's handle.

use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
use winit::window::{Window, WindowAttributes};

/// A child window and its handles, read on the event-loop thread.
pub(crate) struct ChildWindow {
    window: Window,
    handle: RawWindowHandle,
    display: RawDisplayHandle,
}

// SAFETY: `winit::Window` is `Send` and `Sync`; the raw handles are only the
// identifiers of that same window and its display, which the OS lets any thread
// of the process present into on the platforms where they are handed out from
// any thread (see `HasWindowHandle` below; elsewhere `winit` checks the thread).
unsafe impl Send for ChildWindow {}
// SAFETY: as for `Send`: nothing here is mutated after creation.
unsafe impl Sync for ChildWindow {}

impl ChildWindow {
    /// The `winit` window, to place and show.
    pub(crate) fn window(&self) -> &Window {
        &self.window
    }
}

impl HasWindowHandle for ChildWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // An AppKit view must be used on the main thread, which `winit` checks.
        if cfg!(target_os = "macos") {
            return self.window.window_handle();
        }
        // SAFETY: `handle` was read from `window` when it was made, and `window`
        // is owned by `self`, so the handle names a live window for the borrow.
        Ok(unsafe { WindowHandle::borrow_raw(self.handle) })
    }
}

impl HasDisplayHandle for ChildWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        if cfg!(target_os = "macos") {
            return self.window.display_handle();
        }
        // SAFETY: as for the window handle: the display of a window `self` owns.
        Ok(unsafe { DisplayHandle::borrow_raw(self.display) })
    }
}

/// A hidden, undecorated child window in `parent`'s client area, which takes
/// neither the activation nor the focus from it.
pub(crate) fn create(event_loop: &ActiveEventLoop, parent: &Window) -> Result<ChildWindow, String> {
    let handle = parent
        .window_handle()
        .map_err(|error| format!("no parent handle: {error}"))?
        .as_raw();
    let attributes = WindowAttributes::default()
        .with_decorations(false)
        .with_resizable(false)
        .with_visible(false)
        .with_active(false);
    // SAFETY: `handle` is `parent`'s own, read from it just above, and `parent`
    // is borrowed for the whole call, so it names a live window while the child
    // is made. Afterwards the OS destroys the child with its parent, and the
    // handler stops placing a child once its parent window is gone
    // (`App::reconcile_native` runs before the closed windows are dropped).
    let attributes = unsafe { attributes.with_parent_window(Some(handle)) };
    let window = event_loop
        .create_window(attributes)
        .map_err(|error| error.to_string())?;
    let handle = window.window_handle().map_err(|e| e.to_string())?.as_raw();
    let display = window.display_handle().map_err(|e| e.to_string())?.as_raw();
    Ok(ChildWindow {
        window,
        handle,
        display,
    })
}
