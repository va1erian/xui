#![forbid(unsafe_code)]

//! A tree row's leading icon: any [`IconRef`] or a decoded [`Image`], painted
//! before the label.

use crate::Color;
use crate::backend::Canvas;
use crate::geometry::Rect;
use crate::icon::{IconRef, draw_icon};
use crate::image::Image;
use crate::widget::{Glyph, Icon, Lucide};

/// A leading icon painted before a [`TreeRow`](super::TreeRow)'s label.
///
/// [`Self::Icon`] draws any [`IconRef`] — a generated [`Lucide`] outline, the
/// legacy [`Glyph`]/[`Icon`] sets, or a filled glyph — in the row's text
/// colour, so it follows the theme and the selection. [`Image`] carries a
/// decoded bitmap with its own colours, so an app can supply richer artwork.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowIcon {
    /// A built-in portable vector glyph.
    Glyph(Glyph),
    /// Any icon the crate can draw: a Lucide outline or a legacy glyph.
    Icon(IconRef),
    /// A decoded RGBA bitmap, scaled into the icon box.
    Image(Image),
}

impl From<Glyph> for RowIcon {
    fn from(glyph: Glyph) -> RowIcon {
        RowIcon::Glyph(glyph)
    }
}

impl From<IconRef> for RowIcon {
    fn from(icon: IconRef) -> RowIcon {
        RowIcon::Icon(icon)
    }
}

impl From<Lucide> for RowIcon {
    fn from(icon: Lucide) -> RowIcon {
        RowIcon::Icon(IconRef::Lucide(icon))
    }
}

impl From<Icon> for RowIcon {
    fn from(icon: Icon) -> RowIcon {
        RowIcon::Icon(IconRef::from(icon))
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
        RowIcon::Glyph(glyph) => draw_icon(canvas, *glyph, rect, color, dpi),
        RowIcon::Icon(icon) => draw_icon(canvas, *icon, rect, color, dpi),
        RowIcon::Image(image) => canvas.draw_image(image, rect),
    }
}
