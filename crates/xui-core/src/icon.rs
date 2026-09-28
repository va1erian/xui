#![forbid(unsafe_code)]

//! The public icon API: one drawing entry point over the generated
//! [`Lucide`] set and the legacy [`Icon`]/[`Glyph`] vocabularies.
//!
//! An app names an icon with [`Lucide`] — a generated enum with one variant per
//! vendored SVG — and either hands it to a widget (`Button::icon`,
//! `Toolbar::item`, `TreeRow::icon`, `TopBar::icon`, …) or draws it itself with
//! [`draw_icon`]. The older [`Icon`] and [`Glyph`] sets convert into
//! [`IconRef`], so every existing caller keeps compiling and rendering the same
//! shapes.
//!
//! ```no_run
//! use xui_core::icon::{Lucide, draw_icon};
//! # fn paint(canvas: &mut dyn xui_core::backend::Canvas, rect: xui_core::Rect) {
//! draw_icon(canvas, Lucide::FolderOpen, rect, xui_core::Color::hex(0xFF_FF_FF), 96);
//! # }
//! ```

use crate::backend::Canvas;
use crate::color::Color;
use crate::geometry::Rect;
use crate::widget::{Glyph, Icon};

pub use crate::widget::Lucide;

/// A reference to any icon xui can draw.
///
/// It is the common currency of the icon-taking widget builders: a [`Lucide`]
/// outline, one of the legacy [`Icon`] shapes (which resolve to Lucide data) or
/// a built-in [`Glyph`] (some of which are filled or draw a short text run).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconRef {
    /// A generated Lucide outline.
    Lucide(Lucide),
    /// A built-in glyph from the older vocabulary.
    Glyph(Glyph),
}

impl From<Lucide> for IconRef {
    fn from(icon: Lucide) -> IconRef {
        IconRef::Lucide(icon)
    }
}

impl From<Icon> for IconRef {
    fn from(icon: Icon) -> IconRef {
        IconRef::Lucide(icon.lucide())
    }
}

impl From<Glyph> for IconRef {
    fn from(glyph: Glyph) -> IconRef {
        IconRef::Glyph(glyph)
    }
}

/// Draws `icon` centred in `rect` in `color`, scaled for a `dpi`.
///
/// A Lucide outline (and the legacy [`Icon`] shapes) fills `rect`'s smaller
/// side; a [`Glyph`] keeps its own design size. Nothing is drawn when `rect` is
/// too small to show an outline.
pub fn draw_icon(
    canvas: &mut dyn Canvas,
    icon: impl Into<IconRef>,
    rect: Rect,
    color: Color,
    dpi: u32,
) {
    match icon.into() {
        IconRef::Lucide(icon) => {
            let size = rect.width().min(rect.height());
            if size < 3 {
                return;
            }
            crate::widget::lucide::draw_lucide(canvas, icon, false, rect, size, color, dpi);
        }
        IconRef::Glyph(glyph) => crate::widget::draw_glyph(canvas, glyph, rect, color, dpi),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_icons_and_glyphs_convert_to_icon_refs() {
        assert_eq!(IconRef::from(Icon::Plus), IconRef::Lucide(Lucide::Plus));
        assert_eq!(IconRef::from(Icon::Close), IconRef::Lucide(Lucide::X));
        assert_eq!(IconRef::from(Lucide::Save), IconRef::Lucide(Lucide::Save));
        assert_eq!(IconRef::from(Glyph::Play), IconRef::Glyph(Glyph::Play));
    }
}
