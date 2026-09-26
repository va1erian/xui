#![forbid(unsafe_code)]

//! A tree row's leading icon: a portable [`Glyph`] or a decoded [`Image`],
//! painted before the label.

use crate::Color;
use crate::backend::Canvas;
use crate::geometry::Rect;
use crate::image::Image;
use crate::widget::Glyph;
use crate::widget::topbar::draw_glyph;

/// A leading icon painted before a [`TreeRow`](super::TreeRow)'s label.
///
/// [`Glyph`] draws from the built-in vector set, in the row's text colour, so
/// it follows the theme and the selection. [`Image`] carries a decoded bitmap
/// with its own colours, so an app can supply richer artwork.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowIcon {
    /// A built-in portable vector glyph.
    Glyph(Glyph),
    /// A decoded RGBA bitmap, scaled into the icon box.
    Image(Image),
}

impl From<Glyph> for RowIcon {
    fn from(glyph: Glyph) -> RowIcon {
        RowIcon::Glyph(glyph)
    }
}

impl From<Image> for RowIcon {
    fn from(image: Image) -> RowIcon {
        RowIcon::Image(image)
    }
}

/// Draws `icon` in `rect`; a glyph is drawn in `color`, an image in its own.
pub(crate) fn draw(canvas: &mut dyn Canvas, icon: &RowIcon, rect: Rect, color: Color, dpi: u32) {
    match icon {
        RowIcon::Glyph(glyph) => draw_glyph(canvas, *glyph, rect, color, dpi),
        RowIcon::Image(image) => canvas.draw_image(image, rect),
    }
}
