#![forbid(unsafe_code)]

//! The window-level GL content seam on [`WinitBackend`]: installing and
//! removing a [`GlWidget`] for a window, and scheduling its repaints.

use std::rc::Rc;

use xui_core::backend::WindowId;

use super::WinitBackend;
use crate::gl::GlWidget;

impl WinitBackend {
    /// Installs `widget` as `window`'s GPU-rendered content.
    ///
    /// From the next repaint on, the backend presents [`GlWidget::paint_gl`]
    /// frames through a [`GlSurface`](crate::GlSurface) covering the whole
    /// client area instead of copying the software surface. When no GL context
    /// can be created, or a frame cannot be presented, the backend falls back to
    /// [`GlWidget::paint`] on the software surface for good.
    ///
    /// The GL content takes over the window: it must be the window's sole
    /// content, since existing CPU nodes are not composited into the GL frame.
    pub fn set_gl_content<W: GlWidget + 'static>(&self, window: WindowId, widget: W) {
        if let Some(state) = self.shared.windows.borrow_mut().get_mut(&window.raw()) {
            let previous = state.gl.replace(Rc::new(widget));
            // Free the replaced widget's GPU objects with the context current.
            if let Some(previous) = previous {
                state.renderer.teardown_gl(|gl| previous.gl_teardown(gl));
            }
        }
        self.shared.request_redraw(window);
    }

    /// Removes `window`'s GPU-rendered content, releasing its GPU resources
    /// with the context current. The window returns to the software path.
    pub fn clear_gl_content(&self, window: WindowId) {
        let removed = self
            .shared
            .windows
            .borrow_mut()
            .get_mut(&window.raw())
            .and_then(|state| state.gl.take());
        if let Some(widget) = removed
            && let Some(state) = self.shared.windows.borrow_mut().get_mut(&window.raw())
        {
            state.renderer.teardown_gl(|gl| widget.gl_teardown(gl));
        }
        self.shared.request_redraw(window);
    }

    /// Runs `window`'s GL widget teardown with its context current, if it has
    /// one. Called before the window is destroyed, while its surface is still
    /// live.
    pub(crate) fn teardown_window_gl(&self, window: WindowId) {
        let widget = self
            .shared
            .windows
            .borrow()
            .get(&window.raw())
            .and_then(|state| state.gl.clone());
        if let Some(widget) = widget
            && let Some(state) = self.shared.windows.borrow_mut().get_mut(&window.raw())
        {
            state.renderer.teardown_gl(|gl| widget.gl_teardown(gl));
        }
    }

    /// Schedules a repaint of `window`. Use it for window-level GL content,
    /// which has no node to invalidate.
    pub fn request_redraw(&self, window: WindowId) {
        self.shared.request_redraw(window);
    }
}
