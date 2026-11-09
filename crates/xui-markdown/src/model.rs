#![forbid(unsafe_code)]

//! The parsed document: a flat list of blocks over one text buffer.
//!
//! Nesting (a paragraph in a list item in a quote) is kept as depths on each
//! block rather than as a tree, so document order is block order: selection,
//! copy and painting walk one slice. Every block's text is a range of
//! [`Document::text`], and its [`Span`]s partition that range in order, so a
//! layout never has to look anywhere else.

use std::ops::Range;

/// Inline formatting of a [`Span`], as a small set of flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Inline(u8);

impl Inline {
    /// Plain text.
    pub const NONE: Inline = Inline(0);
    /// `**strong**`.
    pub const BOLD: Inline = Inline(1);
    /// `*emphasis*`.
    pub const ITALIC: Inline = Inline(1 << 1);
    /// `~~strike-through~~`.
    pub const STRIKE: Inline = Inline(1 << 2);
    /// `` `code` ``.
    pub const CODE: Inline = Inline(1 << 3);
    /// An image's alt text, which stands in for it until the app supplies its
    /// pixels (see [`Span::image`]).
    pub const IMAGE: Inline = Inline(1 << 4);

    /// Whether every flag of `other` is set here.
    pub const fn contains(self, other: Inline) -> bool {
        self.0 & other.0 == other.0
    }

    /// These flags with those of `other` added.
    pub const fn with(self, other: Inline) -> Inline {
        Inline(self.0 | other.0)
    }

    /// These flags with those of `other` removed.
    pub const fn without(self, other: Inline) -> Inline {
        Inline(self.0 & !other.0)
    }
}

/// A run of a block's text with one formatting and at most one link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    /// Byte range in [`Document::text`]; never empty.
    pub range: Range<u32>,
    /// Its formatting.
    pub style: Inline,
    /// An index into [`Document::link`], when the run is (part of) a link.
    pub link: Option<u32>,
    /// An index into [`Document::image`], when the run is (part of) an
    /// image's alt text. An image's spans are contiguous, and one with no alt
    /// text has a single span of [`OBJECT`].
    pub image: Option<u32>,
}

/// The text standing for an image with no alt text (U+FFFC, the object
/// replacement character), so every image has a place in
/// [`Document::text`]. Copied text leaves out every U+FFFC, these and any
/// the document itself holds.
pub const OBJECT: char = '\u{FFFC}';

/// An image as the document writes it. Nothing is loaded: the app reads and
/// decodes the source and hands the pixels to the view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    /// The destination as CommonMark reads it, like [`Document::link`]:
    /// backslash escapes and entities resolved, but percent-escapes
    /// (`my%20pic.png`) kept and a relative path left relative, for the app
    /// to resolve.
    pub src: Box<str>,
    /// The title (`![alt](src "title")`), or empty.
    pub title: Box<str>,
}

/// What starts a list item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    /// A bullet (`-`, `*`, `+`).
    Bullet,
    /// A number (`1.`, `2)`), as written for the first item and counted on.
    Number(u64),
    /// A GFM task box, checked or not.
    Task(bool),
}

/// How a table column's cells are aligned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// No alignment given (laid out as left).
    #[default]
    None,
    /// `:---`.
    Left,
    /// `:---:`.
    Center,
    /// `---:`.
    Right,
}

/// What a block is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    /// A heading, level 1 to 6.
    Heading(u8),
    /// A paragraph, or a list item's second or later paragraph (its
    /// [`Block::list`] depth indents it under the marker).
    Paragraph,
    /// The first paragraph of a list item, drawn after its marker.
    ListItem(Marker),
    /// A fenced or indented code block. Its text is the code without the
    /// final newline; lines are separated by `\n`.
    Code {
        /// The info string's first word (`rust` in ```` ```rust ````), or empty.
        lang: Box<str>,
    },
    /// One row of a GFM table.
    TableRow {
        /// The table, an index into [`Document::table`].
        table: u32,
        /// Whether this is the header row.
        header: bool,
        /// The row's cells, a range of [`Document::cells`]. Cells are
        /// contiguous ranges of the block's text, and no span crosses one.
        cells: Range<u32>,
    },
    /// A thematic break (`---`); its text is empty.
    Rule,
}

/// One block of the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// What it is.
    pub kind: BlockKind,
    /// Byte range in [`Document::text`]. A hard line break inside it is `\n`.
    pub text: Range<u32>,
    /// Its spans, a range of [`Document::spans`]. They partition `text` in
    /// order (none when `text` is empty).
    pub spans: Range<u32>,
    /// How many block quotes it sits in.
    pub quote: u8,
    /// How deeply it is nested in lists (1 inside a top-level list).
    pub list: u8,
}

/// A table's column alignments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    /// One entry per column.
    pub columns: Vec<Align>,
}

/// A parsed Markdown document.
///
/// Built by [`Document::parse`]. It holds no UI types and is `Send`, so it
/// can be parsed on a worker thread.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    pub(crate) text: String,
    pub(crate) blocks: Vec<Block>,
    pub(crate) spans: Vec<Span>,
    pub(crate) cells: Vec<Range<u32>>,
    pub(crate) tables: Vec<Table>,
    pub(crate) links: Vec<Box<str>>,
    pub(crate) images: Vec<ImageRef>,
    /// Heading slugs and the block each names, in document order.
    pub(crate) anchors: Vec<(Box<str>, u32)>,
}

impl Document {
    /// All the document's text; blocks and spans index into it.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The blocks, in document order.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// The text of `block`.
    pub fn block_text(&self, block: &Block) -> &str {
        &self.text[range(&block.text)]
    }

    /// The spans of `block`, in order.
    pub fn spans(&self, block: &Block) -> &[Span] {
        &self.spans[range(&block.spans)]
    }

    /// The cells of a table row (empty for any other block).
    pub fn cells(&self, block: &Block) -> &[Range<u32>] {
        match &block.kind {
            BlockKind::TableRow { cells, .. } => &self.cells[range(cells)],
            _ => &[],
        }
    }

    /// A table's columns.
    pub fn table(&self, index: u32) -> &Table {
        &self.tables[index as usize]
    }

    /// A link's destination as CommonMark reads it: backslash escapes and
    /// entities resolved, nothing else changed (a relative path stays
    /// relative).
    pub fn link(&self, index: u32) -> &str {
        &self.links[index as usize]
    }

    /// The document's images in order, which [`Span::image`] indexes. A
    /// source may appear more than once.
    pub fn images(&self) -> &[ImageRef] {
        &self.images
    }

    /// One image, by its [`Span::image`] index.
    pub fn image(&self, index: u32) -> &ImageRef {
        &self.images[index as usize]
    }

    /// The block of the heading whose GitHub-style slug is `slug` (without
    /// the `#`).
    pub fn anchor(&self, slug: &str) -> Option<usize> {
        self.anchors
            .iter()
            .find(|(name, _)| &**name == slug)
            .map(|&(_, block)| block as usize)
    }

    /// The heading block a `#slug` link points at, the slug as written or
    /// percent-decoded (`#caf%C3%A9` for `café`).
    pub(crate) fn anchor_of_link(&self, href: &str) -> Option<usize> {
        let slug = href.strip_prefix('#')?;
        self.anchor(slug)
            .or_else(|| percent_decode(slug).and_then(|slug| self.anchor(&slug)))
    }
}

/// `text` with its `%XX` escapes decoded, or `None` when it has none or they
/// do not decode to UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    if !text.contains('%') {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .filter(|h| h.iter().all(u8::is_ascii_hexdigit))
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// A `u32` range as a `usize` one, for slicing.
pub(crate) fn range(r: &Range<u32>) -> Range<usize> {
    r.start as usize..r.end as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_escapes_decode_and_bad_ones_stay_or_fail() {
        assert_eq!(percent_decode("caf%C3%A9").as_deref(), Some("café"));
        assert_eq!(percent_decode("plain"), None);
        assert_eq!(percent_decode("50%-off").as_deref(), Some("50%-off"));
        assert_eq!(percent_decode("%FF"), None);
        assert_eq!(percent_decode("%"), Some("%".to_owned()));
        assert_eq!(percent_decode("%+1").as_deref(), Some("%+1"));
    }

    #[test]
    fn a_link_finds_its_heading_as_written_or_decoded() {
        let doc = Document::parse(
            "# Café au lait

## Two words
",
        );
        assert_eq!(doc.anchor_of_link("#café-au-lait"), Some(0));
        assert_eq!(doc.anchor_of_link("#caf%C3%A9-au-lait"), Some(0));
        assert_eq!(doc.anchor_of_link("#two-words"), Some(1));
        assert_eq!(doc.anchor_of_link("two-words"), None);
        assert_eq!(doc.anchor_of_link("#nowhere"), None);
    }
}
