#![forbid(unsafe_code)]

//! The state machine behind a window's GL content: [`RendererState`] creates the
//! [`GlSurface`] on the first frame and falls back to software for good when the
//! context cannot be created or a frame cannot be presented.

use winit::window::Window;
use xui_core::color::Color;

use super::GlSurface;

/// Which renderer currently draws a window's GL content.
pub(crate) enum RendererState {
    /// Not yet decided: the first frame creates the surface or falls back.
    Untried,
    /// Drawing with OpenGL.
    Gl(Box<GlSurface>),
    /// No GL context; the widget's software paint runs instead.
    Software,
}

impl RendererState {
    /// Paints one OpenGL frame, creating the surface on first use. `draw`
    /// receives the current [`glow::Context`] with the viewport set and the
    /// framebuffer cleared to `background`.
    ///
    /// When a frame cannot be presented, `teardown` runs with the context still
    /// current before the surface is dropped and the renderer falls back to
    /// software, so the widget can free its GPU resources.
    ///
    /// Returns `false` when no frame was presented, so the caller paints the
    /// software fallback instead.
    pub(crate) fn frame(
        &mut self,
        window: &Window,
        width: u32,
        height: u32,
        background: Color,
        draw: impl FnOnce(&glow::Context),
        teardown: impl FnOnce(&glow::Context),
    ) -> bool {
        if matches!(self, RendererState::Untried) {
            *self = match GlSurface::new(window) {
                Ok(surface) => RendererState::Gl(Box::new(surface)),
                Err(_) => RendererState::Software,
            };
        }
        let RendererState::Gl(surface) = self else {
            return false;
        };
        surface.resize(width as i32, height as i32);
        let gl = surface.begin_frame(background);
        draw(gl);
        if surface.end_frame().is_err() {
            surface.with_gl(teardown);
            *self = RendererState::Software;
            return false;
        }
        true
    }

    /// Drops the OpenGL surface, if one exists, first running `teardown` with
    /// its context current. Use it when the GL content is removed so GPU
    /// resources are freed while the context is still live.
    pub(crate) fn teardown_gl(&mut self, teardown: impl FnOnce(&glow::Context)) {
        if let RendererState::Gl(surface) = self {
            surface.with_gl(teardown);
            *self = RendererState::Untried;
        }
    }
}
