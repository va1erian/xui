//! Images: uploaded once per image identity and drawn scaled.

use std::collections::{HashMap, VecDeque};

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_PROPERTIES, ID2D1Bitmap,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

use super::RectF;
use super::shapes::rect_f;
use super::target::Target;

/// How many bytes of uploaded images a target keeps before it drops the oldest.
/// A viewport that streams a new image every frame must not grow without bound.
const BUDGET_BYTES: usize = 96 << 20;

/// Uploaded images by identity, oldest first, within a byte budget.
#[derive(Default)]
pub(super) struct BitmapCache {
    map: HashMap<u64, (ID2D1Bitmap, usize)>,
    order: VecDeque<u64>,
    bytes: usize,
}

impl BitmapCache {
    fn get(&self, id: u64) -> Option<ID2D1Bitmap> {
        self.map.get(&id).map(|(bitmap, _)| bitmap.clone())
    }

    fn insert(&mut self, id: u64, bitmap: ID2D1Bitmap, bytes: usize) {
        while self.bytes + bytes > BUDGET_BYTES
            && let Some(oldest) = self.order.pop_front()
        {
            if let Some((_, freed)) = self.map.remove(&oldest) {
                self.bytes -= freed;
            }
        }
        self.bytes += bytes;
        self.order.push_back(id);
        self.map.insert(id, (bitmap, bytes));
    }
}

impl Target {
    /// Draws the RGBA image `id` (`width` x `height`, top-down) into `dest`.
    /// The pixels are converted and uploaded the first time `id` is seen.
    pub(crate) fn draw_image(&self, id: u64, width: u32, height: u32, rgba: &[u8], dest: RectF) {
        let Some(bitmap) = self.bitmap(id, width, height, rgba) else {
            return;
        };
        // SAFETY: inside a frame; the bitmap belongs to this target.
        unsafe {
            self.rt.DrawBitmap(
                &bitmap,
                Some(&rect_f(dest)),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            )
        };
    }

    fn bitmap(&self, id: u64, width: u32, height: u32, rgba: &[u8]) -> Option<ID2D1Bitmap> {
        if let Some(bitmap) = self.bitmaps.borrow().get(id) {
            return Some(bitmap);
        }
        if rgba.len() < (width as usize) * (height as usize) * 4 {
            return None;
        }
        let mut bgra = Vec::with_capacity(rgba.len());
        let (pixels, _) = rgba.as_chunks::<4>();
        for pixel in pixels {
            let alpha = u16::from(pixel[3]);
            let premultiply = |channel: u8| ((u16::from(channel) * alpha + 127) / 255) as u8;
            bgra.extend_from_slice(&[
                premultiply(pixel[2]),
                premultiply(pixel[1]),
                premultiply(pixel[0]),
                pixel[3],
            ]);
        }
        let properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };
        // SAFETY: `bgra` holds `width * height` four-byte pixels and outlives
        // the call, which copies it.
        let bitmap = unsafe {
            self.rt.CreateBitmap(
                D2D_SIZE_U { width, height },
                Some(bgra.as_ptr() as *const _),
                width * 4,
                &properties,
            )
        }
        .ok()?;
        self.bitmaps
            .borrow_mut()
            .insert(id, bitmap.clone(), bgra.len());
        Some(bitmap)
    }
}
