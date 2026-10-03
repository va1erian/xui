#![forbid(unsafe_code)]

//! Style patches: the attributes a command changes, leaving the rest alone, so
//! "toggle bold" over mixed text keeps every other attribute of each run.

use xui_core::backend::TextWeight;
use xui_core::{Color, Dip};

use super::style::{
    Align, Baseline, BlockKind, CharStyle, LineSpacing, ListItem, ParaStyle, TextColor,
};

/// A partial [`CharStyle`]: `None` leaves an attribute as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharStylePatch {
    /// Bold (weight 700) or regular (400).
    pub bold: Option<bool>,
    /// Italic on or off.
    pub italic: Option<bool>,
    /// Underline on or off.
    pub underline: Option<bool>,
    /// Strike-through on or off.
    pub strike: Option<bool>,
    /// The text colour.
    pub color: Option<TextColor>,
    /// The highlight, where `Some(None)` removes it.
    pub highlight: Option<Option<Color>>,
    /// The link target, where `Some(None)` removes the link.
    pub link: Option<Option<String>>,
    /// The font size.
    pub size: Option<Dip>,
    /// The font family, where `Some(None)` selects the default.
    pub family: Option<Option<String>>,
    /// The baseline position.
    pub baseline: Option<Baseline>,
}

impl CharStylePatch {
    /// A patch that sets bold on or off.
    pub fn bold(on: bool) -> CharStylePatch {
        CharStylePatch {
            bold: Some(on),
            ..CharStylePatch::default()
        }
    }

    /// A patch that sets italic on or off.
    pub fn italic(on: bool) -> CharStylePatch {
        CharStylePatch {
            italic: Some(on),
            ..CharStylePatch::default()
        }
    }

    /// A patch that sets underline on or off.
    pub fn underline(on: bool) -> CharStylePatch {
        CharStylePatch {
            underline: Some(on),
            ..CharStylePatch::default()
        }
    }

    /// A patch that sets strike-through on or off.
    pub fn strike(on: bool) -> CharStylePatch {
        CharStylePatch {
            strike: Some(on),
            ..CharStylePatch::default()
        }
    }

    /// Whether the patch changes nothing.
    pub fn is_empty(&self) -> bool {
        *self == CharStylePatch::default()
    }

    /// `style` with the patch's attributes replaced.
    pub fn apply(&self, style: &CharStyle) -> CharStyle {
        let mut out = style.clone();
        if let Some(bold) = self.bold {
            out.weight = if bold {
                TextWeight::BOLD
            } else {
                TextWeight::REGULAR
            };
        }
        if let Some(v) = self.italic {
            out.italic = v;
        }
        if let Some(v) = self.underline {
            out.underline = v;
        }
        if let Some(v) = self.strike {
            out.strike = v;
        }
        if let Some(v) = self.color {
            out.color = v;
        }
        if let Some(v) = self.highlight {
            out.highlight = v;
        }
        if let Some(v) = &self.link {
            out.link = v.clone();
        }
        if let Some(v) = self.size {
            out.size = v;
        }
        if let Some(v) = &self.family {
            out.family = v.clone();
        }
        if let Some(v) = self.baseline {
            out.baseline = v;
        }
        out
    }
}

/// A partial [`ParaStyle`]: `None` leaves an attribute as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParaStylePatch {
    /// Horizontal alignment.
    pub align: Option<Align>,
    /// Indent from the left edge.
    pub indent_left: Option<Dip>,
    /// Indent from the right edge.
    pub indent_right: Option<Dip>,
    /// Extra indent of the first line.
    pub indent_first: Option<Dip>,
    /// Space above the paragraph.
    pub space_before: Option<Dip>,
    /// Space below the paragraph.
    pub space_after: Option<Dip>,
    /// The distance between lines.
    pub line_spacing: Option<LineSpacing>,
    /// The list membership, where `Some(None)` takes the paragraph out of its
    /// list.
    pub list: Option<Option<ListItem>>,
    /// The structural role.
    pub kind: Option<BlockKind>,
}

impl ParaStylePatch {
    /// A patch that sets the alignment.
    pub fn align(align: Align) -> ParaStylePatch {
        ParaStylePatch {
            align: Some(align),
            ..ParaStylePatch::default()
        }
    }

    /// A patch that sets the list membership.
    pub fn list(list: Option<ListItem>) -> ParaStylePatch {
        ParaStylePatch {
            list: Some(list),
            ..ParaStylePatch::default()
        }
    }

    /// A patch that sets the structural role.
    pub fn kind(kind: BlockKind) -> ParaStylePatch {
        ParaStylePatch {
            kind: Some(kind),
            ..ParaStylePatch::default()
        }
    }

    /// Whether the patch changes nothing.
    pub fn is_empty(&self) -> bool {
        *self == ParaStylePatch::default()
    }

    /// `style` with the patch's attributes replaced.
    pub fn apply(&self, style: &ParaStyle) -> ParaStyle {
        let mut out = style.clone();
        if let Some(v) = self.align {
            out.align = v;
        }
        if let Some(v) = self.indent_left {
            out.indent_left = v;
        }
        if let Some(v) = self.indent_right {
            out.indent_right = v;
        }
        if let Some(v) = self.indent_first {
            out.indent_first = v;
        }
        if let Some(v) = self.space_before {
            out.space_before = v;
        }
        if let Some(v) = self.space_after {
            out.space_after = v;
        }
        if let Some(v) = self.line_spacing {
            out.line_spacing = v;
        }
        if let Some(v) = self.list {
            out.list = v;
        }
        if let Some(v) = self.kind {
            out.kind = v;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_changes_only_its_attributes() {
        let base = CharStyle {
            italic: true,
            size: Dip(20.0),
            ..CharStyle::default()
        };
        let bold = CharStylePatch::bold(true).apply(&base);
        assert_eq!(bold.weight, TextWeight::BOLD);
        assert!(bold.italic);
        assert_eq!(bold.size, Dip(20.0));
        let plain = CharStylePatch::bold(false).apply(&bold);
        assert_eq!(plain.weight, TextWeight::REGULAR);
        assert!(CharStylePatch::default().is_empty());
        assert!(!CharStylePatch::bold(true).is_empty());
    }

    #[test]
    fn nested_options_clear_attributes() {
        let linked = CharStyle {
            link: Some("u".into()),
            ..CharStyle::default()
        };
        let patch = CharStylePatch {
            link: Some(None),
            ..CharStylePatch::default()
        };
        assert_eq!(patch.apply(&linked).link, None);
        let para = ParaStylePatch::list(None).apply(&ParaStyle::default());
        assert_eq!(para.list, None);
    }
}
