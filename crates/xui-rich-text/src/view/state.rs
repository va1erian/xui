#![forbid(unsafe_code)]

//! The editor's mutable state, shared by its painter and event mapper.

use xui_core::backend::TextShaper;

use crate::layout::Layout;
use crate::model::{DocPos, Document};

/// A selection as `(anchor, head)`.
pub(crate) type Selection = (DocPos, DocPos);

/// The document, its layout and the view onto it.
pub(crate) struct State {
    pub doc: Document,
    pub layout: Layout,
    pub shaper: Box<dyn TextShaper>,
    /// Vertical scroll offset in device pixels.
    pub scroll: f32,
    /// The viewport height in device pixels, as of the last paint.
    pub viewport: f32,
    pub selection: Option<Selection>,
}

impl State {
    pub fn new(shaper: Box<dyn TextShaper>) -> State {
        State {
            doc: Document::new(),
            layout: Layout::new(),
            shaper,
            scroll: 0.0,
            viewport: 0.0,
            selection: None,
        }
    }

    /// Replaces the document and everything derived from it.
    pub fn set_document(&mut self, doc: Document) {
        self.doc = doc;
        self.layout = Layout::new();
        self.scroll = 0.0;
        self.selection = None;
    }

    /// Brings the layout up to date for an area `width` pixels wide at `dpi`.
    pub fn sync(&mut self, width: f32, viewport: f32, dpi: u32) {
        self.layout.set_metrics(width, dpi);
        self.layout.update(&self.doc, self.shaper.as_ref());
        self.viewport = viewport;
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    /// The largest scroll offset.
    pub fn max_scroll(&self) -> f32 {
        (self.layout.height() - self.viewport).max(0.0)
    }
}
