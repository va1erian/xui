#![forbid(unsafe_code)]

//! The document model: paragraphs of styled text with anchored images.
//!
//! Nothing here shapes text or touches a backend.

mod compare;
mod edit;
pub mod fragment;
pub mod grapheme;
pub mod history;
pub mod object;
pub mod ops;
pub mod page;
pub mod paragraph;
pub mod patch;
mod pieces;
pub mod selection;
pub mod style;
pub mod summary;

pub use fragment::Fragment;
pub use history::{COALESCE_GAP, EditContext, History, Transaction};
pub use object::{InlineImage, OBJECT_CHAR, ObjectId, ObjectTable, Side, Wrap};
pub use ops::{EditError, EditOp, Slice};
pub use page::{PageSetup, mm};
pub use paragraph::{Paragraph, Span};
pub use patch::{CharStylePatch, ParaStylePatch};
pub use selection::{DocRange, Selection};
pub use style::{
    Align, Baseline, BlockKind, CharStyle, CharStyleId, LineSpacing, ListItem, ListKind, ParaStyle,
    ParaStyleId, StyleTable, TextColor,
};
pub use summary::{StyleSummary, Tri};

/// Which way a caret leans at a position shared by two lines (a soft wrap):
/// `Upstream` draws it at the end of the earlier line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Affinity {
    /// At the end of the earlier line.
    Upstream,
    /// At the start of the later line.
    #[default]
    Downstream,
}

/// A caret position: a byte offset in a paragraph, always on a grapheme
/// boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocPos {
    /// The paragraph index.
    pub para: usize,
    /// The byte offset in the paragraph's text.
    pub byte: usize,
}

impl DocPos {
    /// The position `byte` bytes into paragraph `para`.
    pub const fn new(para: usize, byte: usize) -> DocPos {
        DocPos { para, byte }
    }
}

/// A rich-text document: paragraphs, the styles they use and the objects they
/// anchor. It always has at least one paragraph.
#[derive(Clone, Debug)]
pub struct Document {
    pub(crate) paragraphs: Vec<Paragraph>,
    pub(crate) styles: StyleTable,
    pub(crate) objects: ObjectTable,
    pub(crate) page: PageSetup,
}

impl Document {
    /// A document of one empty paragraph in the default styles.
    pub fn new() -> Document {
        Document {
            paragraphs: vec![Paragraph::new(
                "",
                ParaStyleId::DEFAULT,
                CharStyleId::DEFAULT,
            )],
            styles: StyleTable::new(),
            objects: ObjectTable::new(),
            page: PageSetup::default(),
        }
    }

    /// A document of `text` in the default styles, one paragraph per line.
    pub fn from_plain_text(text: &str) -> Document {
        let mut doc = Document::new();
        doc.paragraphs = text
            .split('\n')
            .map(|line| {
                let line = line
                    .strip_suffix('\r')
                    .unwrap_or(line)
                    .replace(OBJECT_CHAR, "");
                Paragraph::new(line, ParaStyleId::DEFAULT, CharStyleId::DEFAULT)
            })
            .collect();
        doc
    }

    /// Builds a document from parts, checking every invariant.
    pub fn from_parts(
        paragraphs: Vec<Paragraph>,
        styles: StyleTable,
        objects: ObjectTable,
    ) -> Result<Document, String> {
        let doc = Document {
            paragraphs,
            styles,
            objects,
            page: PageSetup::default(),
        };
        doc.check()?;
        Ok(doc)
    }

    /// The paragraphs, in order.
    pub fn paragraphs(&self) -> &[Paragraph] {
        &self.paragraphs
    }

    /// The style table.
    pub fn styles(&self) -> &StyleTable {
        &self.styles
    }

    /// The style table, mutably (to intern new styles).
    pub fn styles_mut(&mut self) -> &mut StyleTable {
        &mut self.styles
    }

    /// The object table.
    pub fn objects(&self) -> &ObjectTable {
        &self.objects
    }

    /// The page the document is laid out on in page view.
    pub fn page(&self) -> &PageSetup {
        &self.page
    }

    /// The same document on `page` (for building one; an editor changes the
    /// page through [`EditOp::SetPage`], so it can be undone).
    pub fn with_page(mut self, page: PageSetup) -> Result<Document, String> {
        page.check()?;
        self.page = page;
        Ok(self)
    }

    /// Checks every invariant, describing the first one broken.
    pub fn check(&self) -> Result<(), String> {
        if self.paragraphs.is_empty() {
            return Err("a document needs a paragraph".into());
        }
        self.page.check()?;
        let mut anchored = std::collections::HashSet::new();
        for (i, para) in self.paragraphs.iter().enumerate() {
            para.check().map_err(|e| format!("paragraph {i}: {e}"))?;
            if para.style.0 as usize >= self.styles.paras().len() {
                return Err(format!("paragraph {i}: unknown paragraph style"));
            }
            for (_, style) in para.runs() {
                if style.0 as usize >= self.styles.chars().len() {
                    return Err(format!("paragraph {i}: unknown character style"));
                }
            }
            for &id in &para.anchors {
                if self.objects.get(id).is_none() {
                    return Err(format!("paragraph {i}: unknown object {id:?}"));
                }
                if !anchored.insert(id) {
                    return Err(format!("paragraph {i}: object {id:?} is anchored twice"));
                }
            }
        }
        Ok(())
    }
}

impl Default for Document {
    fn default() -> Document {
        Document::new()
    }
}
