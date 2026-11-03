//! Device-dependent bitmap and bitmap-brush caches. Bitmaps are uploaded
//! premultiplied from a retained `Arc` copy (owned by the surface), so they
//! can be re-uploaded lazily after a device loss.

#[cfg(test)]
use core::cell::Cell;
use std::collections::HashMap;

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_BRUSH_PROPERTIES, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_BITMAP_PROPERTIES, D2D1_EXTEND_MODE_WRAP,
    ID2D1Bitmap, ID2D1BitmapBrush, ID2D1RenderTarget,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;

use crate::d2d::{BASE_DPI, ImageId, Interpolation, Raster, RectF};

use super::target::{Target, rect_f};

thread_local! {
    /// How many device bitmaps this thread created, for tests asserting that
    /// a repeat draw hits the cache.
    #[cfg(test)]
    static CREATED: Cell<u64> = const { Cell::new(0) };
}

/// How many device bitmaps the thread's targets created. Test-only.
#[cfg(test)]
pub(crate) fn bitmaps_created() -> u64 {
    CREATED.with(Cell::get)
}

/// The default device-bitmap budget, in bytes of premultiplied RGBA. The
/// surface's retained-image cache is what a lost device re-uploads from; this
/// is the separate, device-resident copy, kept bounded so a long session cannot
/// pin a bitmap for every cover it has ever shown.
const DEVICE_BUDGET_BYTES: usize = 64 * 1024 * 1024;

/// One device bitmap and its LRU bookkeeping.
struct DeviceBitmap {
    bitmap: ID2D1Bitmap,
    bytes: usize,
    last_used: u64,
}

/// Bitmaps and tiled bitmap brushes, cached per render target, bounded by an
/// LRU over the bitmaps' bytes. A bitmap evicted here is re-created from the
/// pixel data its caller hands each draw — a surface's retained image, or the
/// portable image a [`DcCanvas::draw_image`](crate::d2d::DcCanvas::draw_image)
/// caller passes per paint — on its next use.
pub(crate) struct Images {
    bitmaps: HashMap<ImageId, DeviceBitmap>,
    tiled: HashMap<ImageId, ID2D1BitmapBrush>,
    clock: u64,
    bytes: usize,
    budget: usize,
}

impl Images {
    pub(crate) fn new() -> Images {
        Images {
            bitmaps: HashMap::new(),
            tiled: HashMap::new(),
            clock: 0,
            bytes: 0,
            budget: DEVICE_BUDGET_BYTES,
        }
    }

    /// Drops `id`'s device bitmap and tiled brush, releasing their memory.
    pub(crate) fn forget(&mut self, id: ImageId) {
        if let Some(entry) = self.bitmaps.remove(&id) {
            self.bytes = self.bytes.saturating_sub(entry.bytes);
        }
        self.tiled.remove(&id);
    }

    /// Drops every device bitmap and tiled brush, releasing their memory. The
    /// bitmaps are re-created lazily from the surface's retained images on
    /// their next use (or skipped if those were released too).
    pub(crate) fn clear(&mut self) {
        self.bitmaps.clear();
        self.tiled.clear();
        self.bytes = 0;
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// Evicts least-recently-used bitmaps until the device budget is met.
    fn evict(&mut self) {
        while self.bytes > self.budget && !self.bitmaps.is_empty() {
            let oldest = self
                .bitmaps
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(&id, _)| id);
            if let Some(id) = oldest {
                self.forget(id);
            }
        }
    }

    /// The bitmap for `id`, creating (and premultiplying) it from `raster` on
    /// first use; a repeat draw reads no pixels at all.
    fn bitmap(
        &mut self,
        render: &ID2D1RenderTarget,
        id: ImageId,
        raster: Raster<'_>,
    ) -> Option<ID2D1Bitmap> {
        let last_used = self.tick();
        if let Some(entry) = self.bitmaps.get_mut(&id) {
            entry.last_used = last_used;
            return Some(entry.bitmap.clone());
        }
        #[cfg(test)]
        CREATED.with(|created| created.set(created.get() + 1));
        let pixels = premultiply(raster.pixels);
        let properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: BASE_DPI,
            dpiY: BASE_DPI,
        };
        // SAFETY: `pixels` is a valid premultiplied buffer of the reported size
        // and pitch; the property struct is valid for the call.
        let bitmap = unsafe {
            render.CreateBitmap(
                D2D_SIZE_U {
                    width: raster.width,
                    height: raster.height,
                },
                Some(pixels.as_ptr().cast()),
                raster.width * 4,
                &properties,
            )
        }
        .ok()?;
        let bytes = raster.pixels.len();
        self.bitmaps.insert(
            id,
            DeviceBitmap {
                bitmap: bitmap.clone(),
                bytes,
                last_used,
            },
        );
        self.bytes += bytes;
        self.evict();
        Some(bitmap)
    }

    /// A wrap-mode bitmap brush for tiling `id`.
    fn tiled(
        &mut self,
        render: &ID2D1RenderTarget,
        id: ImageId,
        raster: Raster<'_>,
        interpolation: Interpolation,
    ) -> Option<ID2D1BitmapBrush> {
        if let Some(brush) = self.tiled.get(&id) {
            return Some(brush.clone());
        }
        let bitmap = self.bitmap(render, id, raster)?;
        let properties = D2D1_BITMAP_BRUSH_PROPERTIES {
            extendModeX: D2D1_EXTEND_MODE_WRAP,
            extendModeY: D2D1_EXTEND_MODE_WRAP,
            interpolationMode: interpolation_mode(interpolation),
        };
        // SAFETY: valid bitmap (from this target) and property struct.
        let brush = unsafe { render.CreateBitmapBrush(&bitmap, Some(&properties), None) }.ok()?;
        self.tiled.insert(id, brush.clone());
        Some(brush)
    }
}

/// The Direct2D interpolation mode for a bitmap draw.
fn interpolation_mode(
    interpolation: Interpolation,
) -> windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE {
    match interpolation {
        Interpolation::Nearest => D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
        Interpolation::Linear | Interpolation::HighQualityCubic => {
            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR
        }
    }
}

/// Converts straight-alpha RGBA to the premultiplied form Direct2D bitmaps use.
fn premultiply(pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len());
    let (chunks, _) = pixels.as_chunks::<4>();
    for &[r, g, b, a] in chunks {
        let alpha = u32::from(a);
        let channel = |c: u8| ((u32::from(c) * alpha + 127) / 255) as u8;
        out.extend_from_slice(&[channel(r), channel(g), channel(b), a]);
    }
    out
}

impl Target {
    pub(crate) fn draw_image(
        &mut self,
        id: ImageId,
        raster: Raster<'_>,
        dest: RectF,
        src: Option<RectF>,
        opacity: f32,
        interpolation: Interpolation,
    ) {
        if let Some(bitmap) = self.images.bitmap(&self.render, id, raster) {
            let dest_rect = rect_f(dest);
            let src_rect = src.map(rect_f);
            // SAFETY: valid bitmap (from this target) and rectangles; drawing
            // is active.
            unsafe {
                self.render.DrawBitmap(
                    &bitmap,
                    Some(&dest_rect),
                    opacity,
                    interpolation_mode(interpolation),
                    src_rect.as_ref().map(|rect| rect as *const _),
                )
            }
        }
    }

    pub(crate) fn fill_image_tiled(
        &mut self,
        id: ImageId,
        raster: Raster<'_>,
        dest: RectF,
        opacity: f32,
        interpolation: Interpolation,
    ) {
        if let Some(brush) = self.images.tiled(&self.render, id, raster, interpolation) {
            // SAFETY: the brush is from this target; drawing is active.
            unsafe {
                brush.SetOpacity(opacity);
                self.render.FillRectangle(&rect_f(dest), &brush);
            }
        }
    }
}
