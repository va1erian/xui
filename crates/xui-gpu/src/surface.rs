#![forbid(unsafe_code)]

//! [`GlSurface`]: an OpenGL context bound to a window, shared by every backend.
//!
//! The platform context creation stays in each backend behind [`GlContext`];
//! this owns the lifecycle in terms of that opaque handle.

use std::cell::{Cell, RefCell};

use xui_core::color::Color;

use crate::sys::Offscreen;
use crate::{GlContext, GlError};

/// How many distinct offscreen sizes a surface keeps framebuffers for. A window
/// has one base layer plus its node-level GL panes; caching a few sizes avoids
/// rebuilding a framebuffer every frame when they differ.
const MAX_OFFSCREENS: usize = 4;

/// An OpenGL context for one window.
///
/// Created by a backend and resized from the window's size in physical pixels;
/// the context is made current on the UI thread when a frame begins. It draws
/// either straight to the window ([`begin_frame`](GlSurface::begin_frame) /
/// [`end_frame`](GlSurface::end_frame)) or into a texture read back as pixels
/// ([`render_offscreen`](GlSurface::render_offscreen)), so the same context
/// serves a window-level frame and a GL widget composited into a software
/// surface.
pub struct GlSurface {
    context: Box<dyn GlContext>,
    pixels: Cell<(u32, u32)>,
    dpi: Cell<u32>,
    offscreen: RefCell<Vec<Offscreen>>,
}

impl GlSurface {
    /// Wraps a platform context. `pixels` is the initial framebuffer size and
    /// `dpi` the window's dots-per-inch; both are clamped to at least one.
    pub fn new(context: Box<dyn GlContext>, pixels: (u32, u32), dpi: u32) -> GlSurface {
        GlSurface {
            context,
            pixels: Cell::new((pixels.0.max(1), pixels.1.max(1))),
            dpi: Cell::new(dpi),
            offscreen: RefCell::new(Vec::new()),
        }
    }

    /// Resizes the framebuffer to `width` x `height` physical pixels; the
    /// viewport is applied when the next frame begins.
    pub fn resize(&self, width: i32, height: i32) {
        self.pixels.set(non_zero_pixels(width, height));
    }

    /// Records the window's dots-per-inch, for a widget that reads it. OpenGL
    /// draws in physical pixels, so this does not scale the viewport.
    pub fn set_dpi(&self, dpi: u32) {
        self.dpi.set(dpi);
    }

    /// The window's dots-per-inch.
    pub fn dpi(&self) -> u32 {
        self.dpi.get()
    }

    /// Starts a frame: makes the context current, resizes the window surface,
    /// applies the viewport, clears to `background` and returns the `glow`
    /// context to draw with.
    pub fn begin_frame(&self, background: Color) -> &glow::Context {
        self.context.make_current();
        let (width, height) = self.pixels.get();
        if let Some(dpi) = self.context.prepare_frame(width, height) {
            self.dpi.set(dpi);
        }
        self.context.set_viewport(width as i32, height as i32);
        self.context.clear_to(background);
        self.context.glow()
    }

    /// Makes the context current and runs `f` with it, without starting a frame
    /// (no resize, viewport or clear). Use it for GL work outside a paint, such
    /// as freeing GPU resources.
    pub fn with_gl<R>(&self, f: impl FnOnce(&glow::Context) -> R) -> R {
        self.context.make_current();
        f(self.context.glow())
    }

    /// Presents the frame.
    pub fn end_frame(&self) -> Result<(), GlError> {
        self.context.present().map_err(GlError::from)
    }

    /// Renders `draw` into a texture of `width` x `height` and reads it back as
    /// top-down RGBA pixels, so a backend can composite a GL widget through its
    /// ordinary software painter model.
    ///
    /// The context is made current and the framebuffer is cleared to
    /// `background` with the viewport covering the texture before `draw` runs.
    /// Framebuffers are cached per size (up to a few), so a stable set of GL
    /// panes does not rebuild one every frame. The caller must leave the
    /// framebuffer it was given bound.
    ///
    /// This allocates a readback buffer each call, which is unavoidable: the
    /// pixels are handed to the software compositor. Returns `None` when no
    /// framebuffer could be created (a lost context), so the caller falls back
    /// to software.
    pub fn render_offscreen(
        &self,
        width: u32,
        height: u32,
        background: Color,
        draw: impl FnOnce(&glow::Context),
    ) -> Option<Vec<u8>> {
        let width = width.max(1);
        let height = height.max(1);
        self.context.make_current();
        let gl = self.context.glow();
        let mut cache = self.offscreen.borrow_mut();
        let offscreen = match cache.iter().position(|o| o.size() == (width, height)) {
            Some(at) => &cache[at],
            None => {
                if let Some(previous) = (cache.len() >= MAX_OFFSCREENS).then(|| cache.remove(0)) {
                    previous.delete(gl);
                }
                cache.push(Offscreen::new(gl, width, height)?);
                cache.last().expect("just pushed")
            }
        };
        offscreen.begin(gl, width, height, background);
        draw(gl);
        let pixels = offscreen.read(gl, width, height);
        Offscreen::unbind(gl);
        Some(pixels)
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
