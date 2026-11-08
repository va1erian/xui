#![forbid(unsafe_code)]

//! How a document looks: fonts, sizes and spacing as design values, and the
//! colour roles text is painted in.
//!
//! Colours are not stored: a [`Role`] is resolved against the live theme when
//! painting, so a theme change repaints without laying anything out again.

use xui_core::backend::FontSpec;
use xui_core::{Color, Dip, Theme};

use crate::model::Inline;

/// Fonts, sizes and spacing of a [`MarkdownView`](crate::MarkdownView), in
/// design units.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkdownStyle {
    /// The body font family, or `None` for the backend's default UI font.
    pub family: Option<String>,
    /// The code font family (code blocks and inline code).
    pub code_family: Option<String>,
    /// The body text size.
    pub size: Dip,
    /// The code text size.
    pub code_size: Dip,
    /// Heading sizes as multiples of [`size`](Self::size), levels 1 to 6.
    pub heading_scale: [f32; 6],
    /// The space around the document.
    pub padding: Dip,
    /// The space between blocks.
    pub gap: Dip,
    /// How far each list level indents.
    pub list_indent: Dip,
    /// How far each quote level indents.
    pub quote_indent: Dip,
    /// The space inside a code block and around a table cell's text.
    pub inset: Dip,
}

impl Default for MarkdownStyle {
    fn default() -> MarkdownStyle {
        MarkdownStyle {
            family: None,
            code_family: Some(DEFAULT_CODE_FAMILY.to_owned()),
            size: Dip(14.0),
            code_size: Dip(13.0),
            heading_scale: [1.8, 1.45, 1.2, 1.05, 1.0, 0.9],
            padding: Dip(16.0),
            gap: Dip(10.0),
            list_indent: Dip(24.0),
            quote_indent: Dip(16.0),
            inset: Dip(8.0),
        }
    }
}

/// A monospace family the platform ships. Elsewhere `monospace` asks the
/// backend for its generic one.
#[cfg(windows)]
const DEFAULT_CODE_FAMILY: &str = "Consolas";
#[cfg(not(windows))]
const DEFAULT_CODE_FAMILY: &str = "monospace";

/// The weight of strong text and headings.
const BOLD: u16 = 700;

/// The font a piece of text is drawn in, without its family string: cheap to
/// copy, hash and compare, so it keys the shape cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FontKey {
    /// The size in hundredths of a design unit.
    size: u32,
    weight: u16,
    italic: bool,
    code: bool,
}

impl MarkdownStyle {
    /// The font for text with `style` in a block whose text is `base` size
    /// (`None` for body text), `weight` and family.
    pub(crate) fn font(&self, style: Inline, heading: Option<u8>, code_block: bool) -> FontKey {
        let code = code_block || style.contains(Inline::CODE);
        let size = match heading {
            Some(level) => self.size.0 * self.heading_scale[usize::from(level.clamp(1, 6)) - 1],
            None if code => self.code_size.0,
            None => self.size.0,
        };
        let bold = heading.is_some() || style.contains(Inline::BOLD);
        FontKey {
            size: (size * 100.0).round() as u32,
            weight: if bold { BOLD } else { 400 },
            italic: style.contains(Inline::ITALIC) || style.contains(Inline::IMAGE),
            code,
        }
    }

    /// The full font request for `key`.
    pub(crate) fn spec(&self, key: FontKey) -> FontSpec {
        let mut spec = FontSpec::new(Dip(key.size as f32 / 100.0))
            .weight(key.weight)
            .italic(key.italic);
        spec.family = if key.code {
            self.code_family.clone()
        } else {
            self.family.clone()
        };
        spec
    }
}

/// What a piece of text or a decoration is painted in, resolved against the
/// live theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    /// Body text.
    Text,
    /// De-emphasised text: image alt text, quotes, list numbers.
    Weak,
    /// A link.
    Link,
    /// Code block and inline code backgrounds, table header rows.
    Surface,
    /// Rules, quote bars, table grids, heading underlines.
    Border,
}

impl Role {
    /// The colour of this role in `theme`.
    pub(crate) fn color(self, theme: &Theme) -> Color {
        match self {
            Role::Text => theme.text,
            Role::Weak => theme.text_secondary,
            Role::Link => theme.accent,
            Role::Surface => theme.surface,
            Role::Border => theme.border,
        }
    }

    /// The text role of a span.
    pub(crate) fn of(style: Inline, link: bool, quoted: bool) -> Role {
        if link {
            Role::Link
        } else if style.contains(Inline::IMAGE) || quoted {
            Role::Weak
        } else {
            Role::Text
        }
    }
}

/// The page background of the view in `theme`.
pub(crate) fn background(theme: &Theme) -> Color {
    theme.input_background
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_are_bold_and_scaled_and_code_uses_its_own_size_and_family() {
        let style = MarkdownStyle::default();
        let h1 = style.font(Inline::NONE, Some(1), false);
        // The size goes through hundredths of a unit for the cache key.
        assert!((style.spec(h1).size.0 - 14.0 * 1.8).abs() < 0.01);
        assert_eq!(style.spec(h1).weight.value(), 700);
        let code = style.font(Inline::CODE, None, false);
        assert_eq!(style.spec(code).size, Dip(13.0));
        assert_eq!(style.spec(code).family, style.code_family);
        let alt = style.font(Inline::IMAGE, None, false);
        assert!(style.spec(alt).italic);
        assert_ne!(h1, code);
    }

    #[test]
    fn links_win_over_quotes_and_alt_text_is_weak() {
        assert_eq!(Role::of(Inline::NONE, true, true), Role::Link);
        assert_eq!(Role::of(Inline::IMAGE, false, false), Role::Weak);
        assert_eq!(Role::of(Inline::BOLD, false, false), Role::Text);
    }
}
