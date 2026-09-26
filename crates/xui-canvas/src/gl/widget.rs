#![forbid(unsafe_code)]

//! The application-facing seam: a [`GlWidget`] draws itself with either OpenGL
//! or the software [`Canvas`], so a window stays usable when no GL context can
//! be created.

use xui_core::backend::Canvas;
use xui_core::{Rect, Theme};

/// A window-level GPU-rendered widget.
///
/// Install one with
/// [`WinitBackend::set_gl_content`](crate::WinitBackend::set_gl_content); the
/// backend then presents OpenGL frames for the window instead of the software
/// copy. When no GL context can be created (a headless session, no driver) the
/// backend paints [`GlWidget::paint`] into the software surface instead, so the
/// window keeps working.
///
/// Installing GL content takes over the whole client area: the GL widget must
/// be the window's sole content. Compositing CPU nodes under (or over) a GL
/// frame is not part of this seam yet.
///
/// The widget is shared (`&self`), so state that changes while painting lives in
/// `Cell`/`RefCell` fields.
pub trait GlWidget: 'static {
    /// Paints the software fallback into `canvas`, covering `bounds` (the whole
    /// client area, in device pixels). Draw only from `theme`'s semantic tokens.
    ///
    /// Called only when no GL context is available; a widget should mirror its
    /// GPU output closely enough to stay readable. The default draws nothing.
    fn paint(&self, canvas: &mut dyn Canvas, bounds: Rect, theme: &Theme);

    /// Paints one OpenGL frame. `gl` is the window's context, made current with
    /// the viewport set to `bounds` and the framebuffer cleared to the theme
    /// background; issue GL calls through it and the backend presents the frame
    /// when this returns. Do not present the frame yourself.
    fn paint_gl(&self, gl: &glow::Context, bounds: Rect, theme: &Theme);

    /// Releases the GPU resources created in [`GlWidget::paint_gl`].
    ///
    /// Called with `gl` made current, just before the window's
    /// [`GlSurface`](crate::GlSurface) is dropped: when the window closes, or
    /// when a failed frame makes the backend fall back to software. Free
    /// anything that must be destroyed with the context current here. The
    /// default does nothing.
    fn gl_teardown(&self, gl: &glow::Context) {
        let _ = gl;
    }
}
