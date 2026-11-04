//! A `winit` window made the child of another, whose handles a GPU API may
//! take on the thread that renders. `winit` hands its handles out only on the
//! event-loop thread, and leaves parenting to an `unsafe` attribute because it
//! cannot check the parent's handle.
//!
//! Only Win32 and X11 are covered: there a child is a real child of its parent's
//! client area, and its handles may be used from any thread. Elsewhere `winit`
//! makes a separate top-level window (Wayland ignores the parent, macOS makes a
//! child `NSWindow` placed in screen coordinates that takes the key focus), so
//! [`create`] refuses and the node keeps drawing through its painter.

use std::sync::atomic::{AtomicBool, Ordering};

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
    /// Cleared when the node gives the window up, which the backend does
    /// before it destroys the parent (and this window with it): from then on
    /// the handles would name a window that may be gone.
    alive: AtomicBool,
}

// SAFETY: `winit::Window` is `Send` and `Sync`. The raw handles are a Win32 or
// X11 window and display (`create` makes no other kind), plain identifiers that
// any thread of the process may present into; nothing here changes after
// creation but `alive`, which is atomic.
unsafe impl Send for ChildWindow {}
// SAFETY: as for `Send`.
unsafe impl Sync for ChildWindow {}

impl ChildWindow {
    /// The `winit` window, to place and show.
    pub(crate) fn window(&self) -> &Window {
        &self.window
    }

    /// Says the window was given up: the handles are refused from now.
    pub(crate) fn orphan(&self) {
        self.alive.store(false, Ordering::Release);
    }

    fn live(&self) -> Result<(), HandleError> {
        if self.alive.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(HandleError::Unavailable)
        }
    }
}

impl HasWindowHandle for ChildWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.live()?;
        // SAFETY: `handle` was read from `window` when it was made and `window`
        // is owned by `self`. `alive` says it had not been given up when this
        // was called, and the backend gives it up before destroying its parent,
        // so it named a live window then. A parent closed on the event-loop
        // thread while another thread still uses the borrow leaves that thread
        // an identifier the OS rejects (presents fail), never memory of ours.
        Ok(unsafe { WindowHandle::borrow_raw(self.handle) })
    }
}

impl HasDisplayHandle for ChildWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.live()?;
        // SAFETY: as for the window handle: the display of that live window.
        Ok(unsafe { DisplayHandle::borrow_raw(self.display) })
    }
}

/// Whether `handle` is a window a child can be made in and used from any thread.
fn supported(handle: &RawWindowHandle) -> bool {
    matches!(
        handle,
        RawWindowHandle::Win32(_) | RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_)
    )
}

/// A hidden, undecorated child window in `parent`'s client area, which takes
/// neither the activation nor the focus from it; an error on a platform where
/// a child is not part of its parent (see the module).
pub(crate) fn create(event_loop: &ActiveEventLoop, parent: &Window) -> Result<ChildWindow, String> {
    let handle = parent
        .window_handle()
        .map_err(|error| format!("no parent handle: {error}"))?
        .as_raw();
    if !supported(&handle) {
        return Err("native child windows need Win32 or X11".into());
    }
    let attributes = WindowAttributes::default()
        .with_decorations(false)
        .with_resizable(false)
        .with_visible(false)
        .with_active(false);
    // SAFETY: `handle` is `parent`'s own, read from it just above, and `parent`
    // is borrowed for the whole call, so it names a live window while the child
    // is made. Later uses of the child's handles check `alive`, which the
    // backend clears when the window is given up, before a closed parent goes
    // (`App::reconcile_native` runs before `App::reconcile_windows`).
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
        alive: AtomicBool::new(true),
    })
}
