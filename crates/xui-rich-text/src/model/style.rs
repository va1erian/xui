#![forbid(unsafe_code)]

//! Character and paragraph styles, interned in a [`StyleTable`].
//!
//! A paragraph's spans refer to styles by id, so a span is two words, style
//! equality is an id compare and the layout's shape cache can key by id.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use xui_core::backend::TextWeight;
use xui_core::{Color, Dip};

/// The colour of a run of text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextColor {
    /// Follows the window theme's text colour, so a default document reads
    /// correctly in light and dark mode.
    #[default]
    Auto,
    /// A colour chosen by the author; part of the document.
    Fixed(Color),
}

/// Where a run sits relative to the line's baseline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Baseline {
    /// On the baseline.
    #[default]
    Normal,
    /// Raised and smaller.
    Superscript,
    /// Lowered and smaller.
    Subscript,
}

/// How a run of characters looks.
#[derive(Clone, Debug, PartialEq)]
pub struct CharStyle {
    /// The font family, or `None` for the backend's default UI font.
    pub family: Option<String>,
    /// The font size.
    pub size: Dip,
    /// The face weight.
    pub weight: TextWeight,
    /// Whether the face is slanted.
    pub italic: bool,
    /// Whether the run is underlined.
    pub underline: bool,
    /// Whether the run is struck through.
    pub strike: bool,
    /// The text colour.
    pub color: TextColor,
    /// A background highlight behind the run.
    pub highlight: Option<Color>,
    /// The target of a link, when the run is one.
    pub link: Option<String>,
    /// The vertical position relative to the baseline.
    pub baseline: Baseline,
}

impl Default for CharStyle {
    fn default() -> CharStyle {
        CharStyle {
            family: None,
            size: Dip(14.0),
            weight: TextWeight::REGULAR,
            italic: false,
            underline: false,
            strike: false,
            color: TextColor::Auto,
            highlight: None,
            link: None,
            baseline: Baseline::Normal,
        }
    }
}

impl Eq for CharStyle {}

impl Hash for CharStyle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.family.hash(state);
        self.size.0.to_bits().hash(state);
        self.weight.hash(state);
        self.italic.hash(state);
        self.underline.hash(state);
        self.strike.hash(state);
        self.color.hash(state);
        self.highlight.hash(state);
        self.link.hash(state);
        self.baseline.hash(state);
    }
}

/// Horizontal alignment of a paragraph's lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
    /// Flush left.
    #[default]
    Left,
    /// Centred.
    Center,
    /// Flush right.
    Right,
    /// Stretched to both edges, except the last line.
    Justify,
}

/// The distance between a paragraph's lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineSpacing {
    /// A multiple of the natural line height (`1.0` is single spacing).
    Multiple(f32),
    /// An exact line height.
    Exactly(Dip),
}

impl Default for LineSpacing {
    fn default() -> LineSpacing {
        LineSpacing::Multiple(1.0)
    }
}

/// The marker of a list item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ListKind {
    /// A bullet.
    Bullet,
    /// A number, counted among consecutive items of the same level.
    Numbered,
}

/// A paragraph's place in a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListItem {
    /// The marker kind.
    pub kind: ListKind,
    /// The nesting level, from 0.
    pub level: u8,
}

/// The structural role of a paragraph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlockKind {
    /// Body text.
    #[default]
    Body,
    /// A heading of level 1 to 3.
    Heading(u8),
    /// A block quotation.
    Quote,
}

/// How a paragraph is laid out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParaStyle {
    /// Horizontal alignment.
    pub align: Align,
    /// Indent from the left edge.
    pub indent_left: Dip,
    /// Indent from the right edge.
    pub indent_right: Dip,
    /// Extra indent of the first line (negative for a hanging indent).
    pub indent_first: Dip,
    /// Space above the paragraph.
    pub space_before: Dip,
    /// Space below the paragraph.
    pub space_after: Dip,
    /// The distance between lines.
    pub line_spacing: LineSpacing,
    /// The list the paragraph belongs to, if any.
    pub list: Option<ListItem>,
    /// The structural role.
    pub kind: BlockKind,
    /// Whether the paragraph starts a new page in page view.
    pub page_break_before: bool,
}

impl Eq for ParaStyle {}

impl Hash for ParaStyle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.align.hash(state);
        for dip in [
            self.indent_left,
            self.indent_right,
            self.indent_first,
            self.space_before,
            self.space_after,
        ] {
            dip.0.to_bits().hash(state);
        }
        match self.line_spacing {
            LineSpacing::Multiple(m) => (0u8, m.to_bits()).hash(state),
            LineSpacing::Exactly(d) => (1u8, d.0.to_bits()).hash(state),
        }
        self.list.hash(state);
        self.kind.hash(state);
        self.page_break_before.hash(state);
    }
}

/// The id of an interned [`CharStyle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CharStyleId(pub(crate) u32);

/// The id of an interned [`ParaStyle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ParaStyleId(pub(crate) u32);

/// Interned styles. Interning the same style twice returns the same id, and
/// ids are never invalidated.
#[derive(Clone, Debug)]
pub struct StyleTable {
    chars: Vec<CharStyle>,
    char_ids: HashMap<CharStyle, CharStyleId>,
    paras: Vec<ParaStyle>,
    para_ids: HashMap<ParaStyle, ParaStyleId>,
}

impl StyleTable {
    /// A table holding only the default character and paragraph styles, as
    /// [`CharStyleId`]/[`ParaStyleId`] `DEFAULT`.
    pub fn new() -> StyleTable {
        let mut table = StyleTable {
            chars: Vec::new(),
            char_ids: HashMap::new(),
            paras: Vec::new(),
            para_ids: HashMap::new(),
        };
        table.intern_char(CharStyle::default());
        table.intern_para(ParaStyle::default());
        table
    }

    /// The id of `style`, adding it if it is new.
    pub fn intern_char(&mut self, style: CharStyle) -> CharStyleId {
        if let Some(&id) = self.char_ids.get(&style) {
            return id;
        }
        let id = CharStyleId(self.chars.len() as u32);
        self.chars.push(style.clone());
        self.char_ids.insert(style, id);
        id
    }

    /// The id of `style`, adding it if it is new.
    pub fn intern_para(&mut self, style: ParaStyle) -> ParaStyleId {
        if let Some(&id) = self.para_ids.get(&style) {
            return id;
        }
        let id = ParaStyleId(self.paras.len() as u32);
        self.paras.push(style.clone());
        self.para_ids.insert(style, id);
        id
    }

    /// The character style `id` names.
    pub fn char(&self, id: CharStyleId) -> &CharStyle {
        &self.chars[id.0 as usize]
    }

    /// The paragraph style `id` names.
    pub fn para(&self, id: ParaStyleId) -> &ParaStyle {
        &self.paras[id.0 as usize]
    }

    /// Every character style, indexed by id.
    pub fn chars(&self) -> &[CharStyle] {
        &self.chars
    }

    /// Every paragraph style, indexed by id.
    pub fn paras(&self) -> &[ParaStyle] {
        &self.paras
    }
}

impl Default for StyleTable {
    fn default() -> StyleTable {
        StyleTable::new()
    }
}

impl CharStyleId {
    /// The default character style every table starts with.
    pub const DEFAULT: CharStyleId = CharStyleId(0);
}

impl ParaStyleId {
    /// The default paragraph style every table starts with.
    pub const DEFAULT: ParaStyleId = ParaStyleId(0);
}
