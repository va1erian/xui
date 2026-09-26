#![forbid(unsafe_code)]

//! [`GlSurface`]: an OpenGL context bound to a `winit` window.
//!
//! `glutin`'s `unsafe` lives in [`crate::sys::gl`]; this is the safe view a
//! [`GlWidget`](crate::GlWidget) draws through. The surface makes the context
//! current, applies the viewport and clears the framebuffer when a frame
//! begins, and presents it when the frame ends.

use std::cell::Cell;

use winit::window::Window;
use xui_core::color::Color;

use crate::sys::gl::Context;

/// Why an OpenGL context or frame failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlError(String);

impl GlError {
    /// The error as text.
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GlError {}

/// An OpenGL context for one window.
///
/// Created lazily by the backend's GL content path and resized from the window's
/// size in physical pixels; the context is made current on the UI thread when a
/// frame begins.
pub struct GlSurface {
    context: Context,
    pixels: Cell<(u32, u32)>,
    dpi: Cell<u32>,
}

impl GlSurface {
    /// Creates a context on `window`'s client area. Fails when no GL display,
    /// config or context can be created (a headless session, no driver), so the
    /// caller can fall back to software.
    pub fn new(window: &Window) -> Result<GlSurface, GlError> {
        let size = window.inner_size();
        let pixels = (size.width.max(1), size.height.max(1));
        let context = Context::new(window, pixels.0, pixels.1).map_err(GlError)?;
        Ok(GlSurface {
            context,
            pixels: Cell::new(pixels),
            dpi: Cell::new(96),
        })
    }

    /// Resizes the framebuffer to `width` x `height` physical pixels; the
    /// viewport is applied when the next frame begins.
    pub fn resize(&self, width: i32, height: i32) {
        self.pixels.set(non_zero_pixels(width, height));
    }

    /// Records the window's dots-per-inch, for a widget that reads it.
    /// OpenGL draws in physical pixels, so this does not scale the viewport.
    pub fn set_dpi(&self, dpi: u32) {
        self.dpi.set(dpi);
    }

    /// The window's dots-per-inch.
    pub fn dpi(&self) -> u32 {
        self.dpi.get()
    }

    /// Starts a frame: makes the context current, resizes the surface and the
    /// viewport to the framebuffer, clears to `background` and returns the
    /// `glow` context to draw with.
    pub fn begin_frame(&self, background: Color) -> &glow::Context {
        self.context.make_current();
        let (width, height) = self.pixels.get();
        self.context.resize(width, height);
        self.context.set_viewport(width as i32, height as i32);
        self.context.clear_to(background);
        self.context.glow()
    }

    /// Makes the context current and runs `f` with it, without starting a
    /// frame (no resize, viewport or clear). Use it for GL work outside a
    /// paint, such as freeing GPU resources.
    pub fn with_gl<R>(&self, f: impl FnOnce(&glow::Context) -> R) -> R {
        self.context.make_current();
        f(self.context.glow())
    }

    /// Presents the frame.
    pub fn end_frame(&self) -> Result<(), GlError> {
        self.context.swap().map_err(GlError)
    }
}

/// A framebuffer size, never smaller than one pixel on either axis. A minimised
/// window reports zero, which no OpenGL surface accepts.
fn non_zero_pixels(width: i32, height: i32) -> (u32, u32) {
    (width.max(1) as u32, height.max(1) as u32)
}

#[cfg(test)]
mod tests {
    use super::non_zero_pixels;

    #[test]
    fn a_zero_sized_framebuffer_is_clamped_to_one_pixel() {
        assert_eq!(non_zero_pixels(0, 0), (1, 1));
        assert_eq!(non_zero_pixels(-4, 12), (1, 12));
        assert_eq!(non_zero_pixels(800, 600), (800, 600));
    }
}
