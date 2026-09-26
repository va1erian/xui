#![forbid(unsafe_code)]

//! Frame presentation: a `GlWidget` frame through the window's GL surface when
//! the window has GL content and a context could be created, otherwise a
//! `softbuffer` copy of the software composite.

use std::num::NonZeroU32;

use xui_core::geometry::Rect;

use super::super::render;
use super::App;
use crate::Surface;

impl App<'_> {
    /// Presents one frame of `raw`: an OpenGL frame when the window has GL
    /// content and a context could be created, otherwise the software
    /// composite (including a GL widget's fallback paint).
    pub(super) fn redraw(&mut self, raw: u64) {
        let window_id = Self::window_id(raw);
        let (width, height, background, gl, window) = match self.shared.windows.borrow().get(&raw) {
            Some(state) => (
                state.size.0,
                state.size.1,
                state.theme.background,
                state.gl.clone(),
                state.window.clone(),
            ),
            None => return,
        };
        let (Some(width), Some(height)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
            return;
        };

        if let (Some(widget), Some(window)) = (gl, window) {
            let bounds = Rect::new(0, 0, width.get() as i32, height.get() as i32);
            let painted = {
                let mut windows = self.shared.windows.borrow_mut();
                let Some(state) = windows.get_mut(&raw) else {
                    return;
                };
                let theme = state.theme;
                state.renderer.frame(
                    &window,
                    width.get(),
                    height.get(),
                    background,
                    |gl| widget.paint_gl(gl, bounds, &theme),
                    |gl| widget.gl_teardown(gl),
                )
            };
            if painted {
                return;
            }
            // The context failed: fall through to the software fallback, which
            // paints the widget through `GlWidget::paint`.
        }

        let handle = self
            .shared
            .windows
            .borrow()
            .get(&raw)
            .and_then(|state| state.window.clone());
        let Some(real) = self.windows.get_mut(&raw) else {
            return;
        };
        if !handle.is_some_and(|handle| real.ensure_software(&handle)) {
            return;
        }
        let Some(software) = real.surface_mut() else {
            return;
        };
        if software.resize(width, height).is_err() {
            return;
        }
        let mut surface = Surface::new(width.get(), height.get());
        render::composite(&self.shared, window_id, &mut surface);
        let image = surface.to_image();
        let Ok(mut buffer) = software.buffer_mut() else {
            return;
        };
        for (destination, pixel) in buffer.iter_mut().zip(image.pixels.as_chunks::<4>().0) {
            // softbuffer wants `0x00RRGGBB`; the pixmap is premultiplied RGBA
            // over an opaque background, so the channels are final.
            *destination =
                (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2]);
        }
        let _ = buffer.present();
    }
}
