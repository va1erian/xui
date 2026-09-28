#![forbid(unsafe_code)]

//! The built-in [`Glyph`](super::Glyph) icon set: vendored Lucide outlines drawn
//! with portable [`Canvas`] primitives, so a media bar needs no icon font and
//! renders the same on every backend.

use crate::backend::{Canvas, TextStyle};
use crate::color::Color;
use crate::geometry::Rect;
use crate::units::Dip;
use crate::widget::lucide;

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
    if let Glyph::Text(text) = glyph {
        let style = TextStyle::new(color, TEXT_SIZE).centered().middle();
        canvas.draw_text(text, rect, &style);
        return;
    }
    let (strokes, filled) = match glyph {
        Glyph::Menu => (lucide::MENU, false),
        Glyph::Search => (lucide::SEARCH, false),
        Glyph::Close => (lucide::X, false),
        Glyph::More => (lucide::ELLIPSIS, false),
        Glyph::Star => (lucide::STAR, false),
        Glyph::StarFilled => (lucide::STAR, true),
        Glyph::Play => (lucide::PLAY, true),
        Glyph::Pause => (lucide::PAUSE, true),
        Glyph::Stop => (lucide::SQUARE, true),
        Glyph::Previous => (lucide::SKIP_BACK, true),
        Glyph::Next => (lucide::SKIP_FORWARD, true),
        Glyph::Repeat => (lucide::REPEAT, false),
        Glyph::Shuffle => (lucide::SHUFFLE, false),
        Glyph::Audio => (lucide::VOLUME_2, false),
        Glyph::Album => (lucide::DISC, false),
        Glyph::People => (lucide::USERS, false),
        Glyph::Tag => (lucide::TAG, false),
        Glyph::Folder => (lucide::FOLDER, false),
        Glyph::History => (lucide::HISTORY, false),
        Glyph::Monitor => (lucide::MONITOR, false),
        Glyph::Settings => (lucide::SETTINGS, false),
        Glyph::Text(_) => return,
    };
    let size = GLYPH.to_px(dpi).value().max(2);
    lucide::draw(canvas, strokes, filled, rect, size, color, dpi);
}
