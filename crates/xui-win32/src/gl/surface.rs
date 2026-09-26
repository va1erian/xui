#![forbid(unsafe_code)]

//! [`GlSurface`]: an OpenGL context bound to a window.
//!
//! WGL's `unsafe` lives in [`crate::sys::gl`]; this is the safe view a
//! [`CustomWidget`](crate::CustomWidget) draws through. A `Renderer::Gl`
//! widget receives the [`glow::Context`] of its surface in
//! [`paint_gl`](crate::CustomWidget::paint_gl), already made current with the
//! viewport set and the framebuffer cleared to the theme background; the
//! surface presents the frame when the widget returns.
//!
//! The surface lifecycle — make current, viewport, clear, present — is owned by
//! [`xui_gpu::GlSurface`]; this only binds it to the WGL context and validates
//! the window after a successful present.

use crate::color::Color;
use crate::error::{Error, Result};
use crate::hwnd::Hwnd;
use crate::sys;
use crate::sys::gl::Context;

/// An OpenGL context for one window.
///
/// Created lazily by the custom-widget paint path (like
/// [`D2dSurface`](crate::d2d::D2dSurface)) and resized from `WM_SIZE`. The
/// context is made current on the UI thread when a frame begins.
pub struct GlSurface {
    surface: xui_gpu::GlSurface,
    hwnd: Hwnd,
}

impl GlSurface {
    /// Creates a context on `hwnd`'s client area. Fails when WGL cannot create
    /// a context (no driver, a remote session), so the caller can fall back to
    /// GDI.
    pub fn new(hwnd: Hwnd) -> Result<GlSurface> {
        let context = Context::new(hwnd)?;
        let client = sys::window::client_rect(hwnd);
        let pixels = (client.width().max(1) as u32, client.height().max(1) as u32);
        let dpi = sys::dpi::window_dpi(hwnd);
        context.set_vsync(true);
        Ok(GlSurface {
            surface: xui_gpu::GlSurface::new(Box::new(context), pixels, dpi),
            hwnd,
        })
    }

    /// Resizes the framebuffer to the client size in device pixels (call on
    /// `WM_SIZE`). The viewport is applied when the next frame begins.
    pub fn resize(&self, width: i32, height: i32) {
        self.surface.resize(width, height);
    }

    /// Applies a new DPI (call on `WM_DPICHANGED`). OpenGL draws in physical
    /// pixels, so this only feeds [`GlSurface::dpi`]; `begin_frame` re-reads
    /// the window's DPI each frame, which child windows are never told about.
    pub fn set_dpi(&self, dpi: u32) {
        self.surface.set_dpi(dpi);
    }

    /// The window's dots-per-inch.
    pub fn dpi(&self) -> u32 {
        self.surface.dpi()
    }

    /// Starts a frame: makes the context current, applies the viewport, clears
    /// to `background` and returns the `glow` context to draw with. The
    /// depth buffer is cleared too, so 3D widgets can depth-test.
    pub fn begin_frame(&self, background: Color) -> &glow::Context {
        self.surface.begin_frame(background)
    }

    /// Makes the context current and runs `f` with it, without starting a
    /// frame (no viewport or clear). Use it for GL work outside a paint —
    /// freeing GPU resources, or uploading an asset up front. The returned
    /// value is `f`'s.
    pub fn with_gl<R>(&self, f: impl FnOnce(&glow::Context) -> R) -> R {
        self.surface.with_gl(f)
    }

    /// Presents the frame and validates the window's whole client area.
    ///
    /// A frame covers every pixel (the framework clears it), so the update
    /// region is fully painted and must be validated, exactly as the Direct2D
    /// path does; otherwise Windows keeps sending `WM_PAINT`.
    pub fn end_frame(&self) -> Result<()> {
        self.surface
            .end_frame()
            .map_err(|error| Error::Gl(error.to_string()))?;
        sys::window::validate(self.hwnd);
        Ok(())
    }
}
