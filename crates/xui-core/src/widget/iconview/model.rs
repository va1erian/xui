#![forbid(unsafe_code)]

//! The icon view's data model and its icon-size vocabulary.
//!
//! An [`IconModel`] supplies each tile's icon and up to three lines of text
//! lazily, so a large model paints only the tiles that are visible. [`IconSize`]
//! picks one of the three Windows XP tile sizes (16, 32 and 48 DIP icons).

use crate::icon::IconRef;
use crate::units::Dip;
use crate::widget::CellData;

/// Supplies an [`IconView`](super::IconView) with tiles without storing them in
/// the view itself.
///
/// The model is borrowed for the paint and [`line`](IconModel::line) returns a
/// borrowed `&str`, so a virtual view allocates nothing per visible tile.
/// Implement it for the struct that already owns the items.
pub trait IconModel {
    /// The number of items.
    fn items(&self) -> usize;

    /// The icon drawn in `item`'s leading slot, or `None` for an empty slot.
    /// The icon follows the tile's text colour, so it tracks the theme, the
    /// selection and the disabled state.
    fn icon(&self, item: usize) -> Option<IconRef>;

    /// The text of `line` (`0` is the name, `1` and `2` are secondary details)
    /// in `item`, or `None` when the line is absent. Lines past `2` are ignored.
    fn line(&self, item: usize, line: usize) -> Option<&str>;

    /// Whether the model holds no items.
    fn is_empty(&self) -> bool {
        self.items() == 0
    }

    /// An optional opaque payload attached to `item`, e.g. an id the app's
    /// context handler keys off. `None` by default.
    fn data(&self, _item: usize) -> Option<CellData> {
        None
    }
}

impl IconModel for Vec<String> {
    fn items(&self) -> usize {
        Vec::len(self)
    }

    fn icon(&self, _item: usize) -> Option<IconRef> {
        None
    }

    fn line(&self, item: usize, line: usize) -> Option<&str> {
        (line == 0)
            .then(|| self.get(item).map(String::as_str))
            .flatten()
    }
}

impl IconModel for Vec<Vec<String>> {
    fn items(&self) -> usize {
        Vec::len(self)
    }

    fn icon(&self, _item: usize) -> Option<IconRef> {
        None
    }

    fn line(&self, item: usize, line: usize) -> Option<&str> {
        self.get(item)
            .and_then(|lines| lines.get(line))
            .map(String::as_str)
    }
}

/// The size of a tile's icon, and the tile and text metrics that derive from
/// it.
///
/// The three sizes mirror the Windows XP "Tiles" view: [`Small`](IconSize::Small)
/// is a 16 DIP icon, [`Medium`](IconSize::Medium) 32 and [`Large`](IconSize::Large)
/// 48. [`Large`](IconSize::Large) is the default. A tile's height is chosen so
/// its text block fits three lines for the medium and large icons; the small
/// tile is only one line tall, so it draws the name alone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum IconSize {
    /// A 16 DIP icon; the tile is a single line tall.
    Small,
    /// A 32 DIP icon.
    Medium,
    /// A 48 DIP icon; the default XP Tiles look.
    #[default]
    Large,
}

impl IconSize {
    /// The icon's side as a design value: `Small` is 16, `Medium` 32 and
    /// `Large` 48 DIP.
    pub const fn icon(self) -> Dip {
        match self {
            IconSize::Small => Dip(16.0),
            IconSize::Medium => Dip(32.0),
            IconSize::Large => Dip(48.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_strings_are_a_single_line_model() {
        let model: Vec<String> = vec!["a".into(), "b".into()];
        assert_eq!(model.items(), 2);
        assert_eq!(model.line(1, 0), Some("b"));
        assert_eq!(model.line(1, 1), None);
        assert_eq!(model.line(2, 0), None);
        assert!(!model.is_empty());
    }

    #[test]
    fn nested_strings_give_lines() {
        let model: Vec<Vec<String>> = vec![vec!["n".into(), "t".into(), "s".into()]];
        assert_eq!(model.line(0, 2), Some("s"));
        assert_eq!(model.line(0, 3), None);
    }

    #[test]
    fn icon_sizes_are_the_xp_values() {
        assert_eq!(IconSize::Small.icon().value(), 16.0);
        assert_eq!(IconSize::Medium.icon().value(), 32.0);
        assert_eq!(IconSize::Large.icon().value(), 48.0);
        assert_eq!(IconSize::default(), IconSize::Large);
    }
}
