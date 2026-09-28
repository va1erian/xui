#![forbid(unsafe_code)]

//! A small portable vector icon set: vendored Lucide outlines drawn with the
//! existing [`Canvas`] primitives, so it needs no image dependency and works on
//! every backend.

use crate::backend::Canvas;
use crate::color::Color;
use crate::geometry::Rect;

use super::lucide;

/// A button-appropriate icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// A plus sign (add / new).
    Plus,
    /// A minus sign (remove).
    Minus,
    /// A cross (close / dismiss).
    Close,
    /// A tick (confirm / done).
    Check,
    /// A downward chevron (expand).
    ChevronDown,
    /// An upward chevron (collapse).
    ChevronUp,
    /// A magnifier (search).
    Search,
    /// Three dots (more actions).
    More,
}

/// Draws `icon` centred in `rect` in `color`, with a stroke scaled to `dpi`.
pub fn draw_icon(canvas: &mut dyn Canvas, icon: Icon, rect: Rect, color: Color, dpi: u32) {
    let size = rect.width().min(rect.height());
    if size < 3 {
        return;
    }
    let strokes = match icon {
        Icon::Plus => lucide::PLUS,
        Icon::Minus => lucide::MINUS,
        Icon::Close => lucide::X,
        Icon::Check => lucide::CHECK,
        Icon::ChevronDown => lucide::CHEVRON_DOWN,
        Icon::ChevronUp => lucide::CHEVRON_UP,
        Icon::Search => lucide::SEARCH,
        Icon::More => lucide::ELLIPSIS,
    };
    lucide::draw(canvas, strokes, false, rect, size, color, dpi);
}
