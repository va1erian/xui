#![forbid(unsafe_code)]

//! [`Win32Canvas`](super::Win32Canvas)'s text, layout and image drawing, split
//! from `canvas.rs` so both files stay under the size limit.

use crate::d2d::{Path, PathBuilder, PointF, RectF};
use crate::gdi;
use xui_core::backend::{PathPlacement, PathSeg, Rgba, TextLayout, TextMetrics, TextStyle};
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

    /// `at` composed with the canvas's own transform: the placement that maps
    /// path space straight to device pixels.
    pub(crate) fn device_placement(&self, at: PathPlacement) -> PathPlacement {
        PathPlacement::new(
            at.scale * self.scale,
            self.tx + at.x * self.scale,
            self.ty + at.y * self.scale,
        )
    }

    /// Builds `path` as a Direct2D geometry in device space, or `None` when
    /// it cannot be built (a device error) or is empty.
    pub(crate) fn device_path(&self, path: &[PathSeg], at: PathPlacement) -> Option<Path> {
        let at = self.device_placement(at);
        let point = |x: f32, y: f32| {
            let (x, y) = at.apply(x, y);
            PointF::new(x, y)
        };
        let mut builder = PathBuilder::new().ok()?;
        for seg in path {
            match *seg {
                PathSeg::MoveTo(x, y) => builder.move_to(point(x, y)),
                PathSeg::LineTo(x, y) => builder.line_to(point(x, y)),
                PathSeg::CubicTo(x1, y1, x2, y2, x, y) => {
                    builder.cubic_to(point(x1, y1), point(x2, y2), point(x, y))
                }
                PathSeg::Close => builder.close(),
            };
        }
        builder.build().ok()
    }
}
