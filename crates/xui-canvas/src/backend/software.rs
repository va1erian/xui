#![forbid(unsafe_code)]

//! The software presentation surface of a real window: a `softbuffer` context
//! and surface, created lazily on the first software frame.

use std::rc::Rc;

use winit::window::Window;

use super::SharedWindow;

/// A real window's software presentation surface.
///
/// Created lazily on the first presented frame. Even a window with GL content
/// owns one now: the GL widget is rendered into an offscreen framebuffer and
/// composited through this surface, and its context is never swapped, so
/// `softbuffer` remains the sole presenter and the two do not compete for the
/// `HWND`.
pub(crate) struct RealWindow {
    context: Option<softbuffer::Context<SharedWindow>>,
    surface: Option<softbuffer::Surface<SharedWindow, SharedWindow>>,
}

impl RealWindow {
    /// A window with no software surface yet.
    pub(crate) fn new() -> RealWindow {
        RealWindow {
            context: None,
            surface: None,
        }
    }

    /// Creates the software surface on first use. Returns whether a surface is
    /// available.
    pub(crate) fn ensure_software(&mut self, window: &Rc<Window>) -> bool {
        if self.surface.is_some() {
            return true;
        }
        let Ok(context) = softbuffer::Context::new(SharedWindow(Rc::clone(window))) else {
            return false;
        };
        let Ok(surface) = softbuffer::Surface::new(&context, SharedWindow(Rc::clone(window)))
        else {
            return false;
        };
        self.context = Some(context);
        self.surface = Some(surface);
        true
    }

    /// The software surface, once [`RealWindow::ensure_software`] has created
    /// it.
    pub(crate) fn surface_mut(
        &mut self,
    ) -> Option<&mut softbuffer::Surface<SharedWindow, SharedWindow>> {
        self.surface.as_mut()
    }
}
