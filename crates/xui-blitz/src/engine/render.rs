//! Drawing a document's visible part into an image with `vello_cpu`, and the
//! pointer shapes Blitz asks for as xui cursors.

use anyrender::{ImageRenderer, PaintScene};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::BaseDocument;
use cursor_icon::CursorIcon;
use kurbo::{Affine, Rect};
use peniko::{Color as PColor, Fill};
use xui_core::Color;
use xui_core::backend::Cursor;
use xui_core::image::Image;

/// Draws documents at one size, keeping the rasteriser's buffers between
/// frames.
pub(crate) struct Raster {
    renderer: VelloCpuImageRenderer,
    size: (u32, u32),
}

impl Raster {
    pub(crate) fn new() -> Raster {
        Raster {
            renderer: VelloCpuImageRenderer::new(1, 1),
            size: (1, 1),
        }
    }

    /// The document's viewport (`width`×`height` device pixels at `scale`,
    /// scrolled as the document is) over `background`.
    ///
    /// Every frame is a fresh pixel buffer: an [`Image`] owns its pixels and
    /// the UI thread keeps the last one on screen while this draws the next.
    pub(crate) fn draw(
        &mut self,
        doc: &mut BaseDocument,
        (width, height): (u32, u32),
        scale: f64,
        background: Color,
    ) -> Option<Image> {
        let (width, height) = (
            width.clamp(1, u16::MAX as u32),
            height.clamp(1, u16::MAX as u32),
        );
        if self.size != (width, height) {
            self.renderer.resize(width, height);
            self.size = (width, height);
        }
        // `render` leaves the frame's commands in the render context. Without
        // this every frame replays all the earlier ones too (slower each
        // time), and replays pictures the image cache has since evicted:
        // vello_cpu then panics with "Image ... not found in registry".
        self.renderer.reset();
        let mut pixels = Vec::new();
        self.renderer.render_to_vec(
            |scene| {
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    PColor::from_rgb8(background.r, background.g, background.b),
                    None,
                    &Rect::new(0.0, 0.0, width as f64, height as f64),
                );
                blitz_paint::paint_scene(scene, doc, scale, width, height, 0, 0);
            },
            &mut pixels,
        );
        Image::from_rgba(width, height, pixels).ok()
    }
}

/// The xui cursor for a CSS cursor. Shapes with no counterpart (`move`,
/// `crosshair`, drag and drop) are the arrow.
pub(crate) fn cursor_for(icon: Option<CursorIcon>) -> Cursor {
    match icon {
        Some(CursorIcon::Pointer) => Cursor::Hand,
        Some(CursorIcon::Text | CursorIcon::VerticalText) => Cursor::Text,
        Some(CursorIcon::Wait | CursorIcon::Progress) => Cursor::Busy,
        Some(
            CursorIcon::EwResize
            | CursorIcon::ColResize
            | CursorIcon::EResize
            | CursorIcon::WResize,
        ) => Cursor::SizeHorizontal,
        Some(
            CursorIcon::NsResize
            | CursorIcon::RowResize
            | CursorIcon::NResize
            | CursorIcon::SResize,
        ) => Cursor::SizeVertical,
        Some(CursorIcon::NwseResize | CursorIcon::NwResize | CursorIcon::SeResize) => {
            Cursor::SizeNwSe
        }
        Some(CursorIcon::NeswResize | CursorIcon::NeResize | CursorIcon::SwResize) => {
            Cursor::SizeNeSw
        }
        _ => Cursor::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_text_and_busy_map_to_their_cursors() {
        assert_eq!(cursor_for(Some(CursorIcon::Pointer)), Cursor::Hand);
        assert_eq!(cursor_for(Some(CursorIcon::Text)), Cursor::Text);
        assert_eq!(cursor_for(Some(CursorIcon::Progress)), Cursor::Busy);
        assert_eq!(
            cursor_for(Some(CursorIcon::ColResize)),
            Cursor::SizeHorizontal
        );
        assert_eq!(cursor_for(Some(CursorIcon::Move)), Cursor::Default);
        assert_eq!(cursor_for(None), Cursor::Default);
    }
}
