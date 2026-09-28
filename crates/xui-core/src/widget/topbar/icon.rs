#![forbid(unsafe_code)]

//! The built-in [`Glyph`](super::Glyph) icon set: each variant resolves to a
//! generated [`Lucide`] outline (some filled), drawn with portable [`Canvas`]
//! primitives, so a media bar needs no icon font and renders the same on every
//! backend.

use crate::backend::{Canvas, TextStyle};
use crate::color::Color;
use crate::geometry::Rect;
use crate::units::Dip;
use crate::widget::lucide::{self, Lucide};

use super::Glyph;

/// The design size of a drawn glyph box.
const GLYPH: Dip = Dip(18.0);
/// The design size of a text glyph.
const TEXT_SIZE: Dip = Dip(14.0);

/// Draws `glyph` centred in `rect` in `color`.
///
/// A [`Glyph::Text`] run goes through [`Canvas::draw_text`]; every other glyph
/// is a Lucide outline (transport shapes and stars are also filled).
pub(crate) fn draw_glyph(
    canvas: &mut dyn Canvas,
    glyph: Glyph,
    rect: Rect,
    color: Color,
    dpi: u32,
) {
    let (icon, filled) = match glyph {
        Glyph::Text(text) => {
            let style = TextStyle::new(color, TEXT_SIZE).centered().middle();
            canvas.draw_text(text, rect, &style);
            return;
        }
        Glyph::Menu => (Lucide::Menu, false),
        Glyph::Search => (Lucide::Search, false),
        Glyph::Close => (Lucide::X, false),
        Glyph::More => (Lucide::Ellipsis, false),
        Glyph::Star => (Lucide::Star, false),
        Glyph::StarFilled => (Lucide::Star, true),
        Glyph::Play => (Lucide::Play, true),
        Glyph::Pause => (Lucide::Pause, true),
        Glyph::Stop => (Lucide::Square, true),
        Glyph::Previous => (Lucide::SkipBack, true),
        Glyph::Next => (Lucide::SkipForward, true),
        Glyph::Repeat => (Lucide::Repeat, false),
        Glyph::Shuffle => (Lucide::Shuffle, false),
        Glyph::Audio => (Lucide::Volume2, false),
        Glyph::Album => (Lucide::Disc, false),
        Glyph::People => (Lucide::Users, false),
        Glyph::Tag => (Lucide::Tag, false),
        Glyph::Folder => (Lucide::Folder, false),
        Glyph::History => (Lucide::History, false),
        Glyph::Monitor => (Lucide::Monitor, false),
        Glyph::Settings => (Lucide::Settings, false),
    };
    let size = GLYPH.to_px(dpi).value().max(2);
    lucide::draw_lucide(canvas, icon, filled, rect, size, color, dpi);
}
