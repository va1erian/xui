#![forbid(unsafe_code)]

//! [`Win32Canvas`](super::Win32Canvas)'s text, layout and image drawing, split
//! from `canvas.rs` so both files stay under the size limit.

use crate::d2d::RectF;
use crate::gdi;
use xui_core::backend::{Rgba, TextLayout, TextMetrics, TextStyle};
use xui_core::image::Image;
use xui_core::{Point, Rect};

use super::Win32Canvas;

impl Win32Canvas<'_> {
    pub(crate) fn paint_text(&mut self, text: &str, rect: Rect, style: &TextStyle) {
        let rect = self.rect(rect);
        crate::backend::text::draw(
            self.canvas,
            text,
            rect,
            style,
            self.dpi,
            Self::text_format(style),
        );
    }

    pub(crate) fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        crate::backend::text::measure_d2d(text, style, self.dpi)
            .unwrap_or_else(|| crate::backend::text::measure_gdi(text, style, self.dpi))
    }

    pub(crate) fn paint_layout(&mut self, layout: &dyn TextLayout, origin: Point, color: Rgba) {
        crate::backend::text::draw_layout(self.canvas, layout, self.point(origin), color);
    }

    pub(crate) fn paint_image(&mut self, image: &Image, rect: Rect) {
        let rect = self.rect(rect);
        if rect.is_empty() {
            return;
        }
        // Draw through the shared Direct2D frame, from the DC target's cached
        // device bitmap: the first draw of an image uploads it once and every
        // later draw — the same row icon on every repaint — is one cached
        // DrawBitmap inside the open frame, instead of a WIC resample, a GDI
        // DIB and a frame close/reopen per draw.
        if let Some(frame) = self.frame.as_mut() {
            frame.draw_image(image, RectF::from_rect(rect));
            return;
        }
        // Direct2D unavailable: the GDI fallback still pays a copy, a WIC
        // resample and a DIB per draw, with no frame that could stay open.
        let rgba = crate::RgbaImage {
            width: image.width(),
            height: image.height(),
            pixels: image.pixels().to_vec(),
        };
        let target = (rect.width().max(1) as u32, rect.height().max(1) as u32);
        let rgba = if (rgba.width, rgba.height) == target {
            rgba
        } else {
            match crate::imaging::resize(&rgba, target.0, target.1) {
                Ok(scaled) => scaled,
                Err(_) => return,
            }
        };
        let Ok(bitmap) =
            gdi::Bitmap::from_rgba(rgba.width as i32, rgba.height as i32, &rgba.pixels)
        else {
            return;
        };
        self.canvas.draw_bitmap(&bitmap, rect);
    }
}
