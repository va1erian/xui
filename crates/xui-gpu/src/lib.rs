#![warn(missing_docs)]

//! The shared OpenGL seam between xui's backends.
//!
//! Platform context creation stays in each backend — WGL in `xui-win32`,
//! `glutin` in `xui-canvas` — behind the opaque [`GlContext`] handle. This
//! crate owns the surface lifecycle in terms of that handle: resize, DPI,
//! [`begin_frame`](GlSurface::begin_frame)/[`end_frame`](GlSurface::end_frame)
//! for a window-backed frame, and
//! [`render_offscreen`](GlSurface::render_offscreen) so a backend can composite
//! a GL widget through its ordinary painter model. `glow` is re-exported so an
//! implementor names the exact version this crate links against.

mod error;
mod surface;
mod sys;

pub use error::GlError;
pub use surface::GlSurface;

/// The OpenGL binding a GL widget draws with.
pub use glow;

use xui_core::color::Color;

/// The platform-specific OpenGL context behind a [`GlSurface`].
///
/// A backend implements this for the context it creates (its WGL or `glutin`
/// wrapper); the [`GlSurface`] then drives it without knowing the platform. All
/// methods are called with the context current on the UI thread, except
/// [`make_current`](GlContext::make_current) itself.
pub trait GlContext {
    /// Makes this context current on the calling thread.
    fn make_current(&self);

    /// The `glow` wrapper over this context's entry points. Only valid while a
    /// context is current.
    fn glow(&self) -> &glow::Context;

    /// Sets the viewport to the whole framebuffer.
    fn set_viewport(&self, width: i32, height: i32);

    /// Clears the colour and depth buffers to `background`.
    fn clear_to(&self, background: Color);

    /// Presents the back buffer. The error text explains a failed frame so the
    /// caller can fall back.
    fn present(&self) -> Result<(), String>;

    /// Runs at the start of a presented frame, once the context is current.
    ///
    /// A backend resizes its window surface here and may return the window's
    /// freshest dots-per-inch for the surface to cache; `None` keeps the cached
    /// value. The default does nothing.
    fn prepare_frame(&self, _width: u32, _height: u32) -> Option<u32> {
        None
    }
}
