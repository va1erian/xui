#![forbid(unsafe_code)]

//! Frame presentation: the software composite, which includes the window's GL
//! content rendered through the node painter model, copied to the window with
//! `softbuffer`.

use std::num::NonZeroU32;

use super::super::render;
use super::App;
use crate::Surface;

impl App<'_> {
    /// Presents one frame of `raw`: the software composite, including any GL
    /// content rendered into it, and the GL widget's software fallback when no
    /// context can be created.
    pub(super) fn redraw(&mut self, raw: u64) {
        let window_id = Self::window_id(raw);
        let (width, height, handle) = match self.shared.windows.borrow().get(&raw) {
            Some(state) => (state.size.0, state.size.1, state.window.clone()),
            None => return,
        };
        let (Some(width), Some(height)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
            return;
        };

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
