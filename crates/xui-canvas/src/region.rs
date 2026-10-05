#![forbid(unsafe_code)]

//! Repainting part of a [`Surface`]: painters draw in surface coordinates,
//! but only the pixels of one rectangle are rasterised.
//!
//! A canvas clip cannot do this: a rectangular clip trims each *shape* to the
//! clip before it is filled or stroked, so a node straddling the clip edge
//! would get a border drawn along it. A region instead paints into a scratch
//! pixmap the size of the region: shapes are built in surface coordinates and
//! shifted onto it by whole pixels as they are rasterised, so every shape
//! keeps its real geometry and the pixmap's edge cuts its pixels. The cost
//! follows the region's area, not the size of the shapes that touch it.
//!
//! Axis-aligned shapes, text and images come out exactly as a whole-surface
//! paint draws them. A slanted or curved edge that crosses the region's edge
//! is clipped by tiny-skia, which re-slopes it from the clip point, so its
//! anti-aliased pixels can differ by a supersampling step or two (as in
//! tiny-skia's own tiled drawing of large pixmaps).

use tiny_skia::{IntSize, Pixmap};
use xui_core::geometry::Rect;

use crate::Surface;
use crate::canvas::SkiaCanvas;
use crate::image_cache::ImageCache;
use crate::paint::intersect;

/// The part of a [`Surface`] being repainted by [`Surface::paint_region`].
pub struct Region<'a> {
    /// The surface's own pixmap for a whole-surface region, else the scratch.
    pixmap: &'a mut Pixmap,
    area: Rect,
    images: &'a mut ImageCache,
}

impl Region<'_> {
    /// The repainted rectangle, in surface coordinates: the requested region
    /// trimmed to the surface.
    pub fn area(&self) -> Rect {
        self.area
    }

    /// Runs `draw` with a canvas over `bounds` (in surface coordinates) at a
    /// dots-per-inch of `dpi`, like [`Surface::with_canvas_at`]; only the
    /// region's pixels change.
    pub fn with_canvas_at<R>(
        &mut self,
        bounds: Rect,
        dpi: u32,
        draw: impl FnOnce(&mut SkiaCanvas) -> R,
    ) -> R {
        let mut canvas = self.canvas(bounds, dpi);
        draw(&mut canvas)
    }

    /// Like [`Region::with_canvas_at`], for a widget painted over its
    /// ancestors, like [`Surface::with_canvas_over_parents`].
    pub fn with_canvas_over_parents<R>(
        &mut self,
        bounds: Rect,
        dpi: u32,
        draw: impl FnOnce(&mut SkiaCanvas) -> R,
    ) -> R {
        let mut canvas = self.canvas(bounds, dpi);
        canvas.over_parents = true;
        draw(&mut canvas)
    }

    fn canvas(&mut self, bounds: Rect, dpi: u32) -> SkiaCanvas<'_> {
        let mut canvas = SkiaCanvas::new(self.pixmap, self.images, bounds, dpi);
        canvas.origin = (self.area.left, self.area.top);
        canvas
    }
}

impl Surface {
    /// Repaints `region` (surface coordinates) of the surface: `paint` draws
    /// through the [`Region`]'s canvases as it would over the whole surface,
    /// but pixels outside `region` are neither rasterised nor changed.
    ///
    /// The region starts with the surface's current pixels, so painters blend
    /// over them exactly as they would in place. A region off the surface
    /// still runs `paint` and changes nothing.
    pub fn paint_region<R>(&mut self, region: Rect, paint: impl FnOnce(&mut Region<'_>) -> R) -> R {
        let (width, height) = self.size();
        let whole = Rect::new(0, 0, width as i32, height as i32);
        let area = intersect(region, whole);
        // A whole-surface repaint (a first frame, a resize) needs no copy.
        if area == whole {
            return paint(&mut Region {
                pixmap: &mut self.pixmap,
                area,
                images: &mut self.images,
            });
        }
        let area = if area.is_empty() {
            Rect::new(0, 0, 0, 0)
        } else {
            area
        };
        // The scratch buffer is kept between calls, so a repaint per frame
        // does not allocate (and fault in) a fresh pixmap each time; it is
        // filled row by row from the surface, never zeroed first.
        let mut data = std::mem::take(&mut self.scratch);
        let row = area.width() as usize * 4;
        data.clear();
        for y in area.top..area.bottom {
            let at = (y as usize * width as usize + area.left as usize) * 4;
            data.extend_from_slice(&self.pixmap.data()[at..at + row]);
        }
        if area.is_empty() {
            data.extend_from_slice(&[0; 4]);
        }
        let size = IntSize::from_wh(area.width().max(1) as u32, area.height().max(1) as u32)
            .expect("non-zero size");
        let mut scratch = Pixmap::from_vec(data, size).expect("sized to the region");
        let result = paint(&mut Region {
            pixmap: &mut scratch,
            area,
            images: &mut self.images,
        });
        let data = scratch.take();
        if !area.is_empty() {
            let surface = self.pixmap.data_mut();
            for (y, from) in (area.top..area.bottom).zip(data.chunks_exact(row)) {
                let at = (y as usize * width as usize + area.left as usize) * 4;
                surface[at..at + row].copy_from_slice(from);
            }
        }
        self.scratch = data;
        result
    }
}
