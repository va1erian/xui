#![forbid(unsafe_code)]

//! The editor's mutable state, shared by its painter and event mapper.

use xui_core::Dip;
use xui_core::backend::TextShaper;
use xui_core::geometry::Rect;
use xui_core::widget::scrollbar::THICKNESS;

use crate::layout::Layout;
use crate::model::{DocPos, Document};

/// The margin between the view's edge and the text, on each side.
const PAD: Dip = Dip(8.0);

/// The text margin in device pixels at `dpi`.
pub(crate) fn pad_px(dpi: u32) -> i32 {
    PAD.to_px(dpi).value()
}

/// A selection as `(anchor, head)`.
pub(crate) type Selection = (DocPos, DocPos);

/// The document, its layout and the view onto it.
pub(crate) struct State {
    pub doc: Document,
    pub layout: Layout,
    pub shaper: Box<dyn TextShaper>,
    /// Vertical scroll offset in device pixels.
    pub scroll: f32,
    /// The viewport height in device pixels, as of the last [`State::prepare`].
    pub viewport: f32,
    pub selection: Option<Selection>,
    /// The node's size at the origin, as last reported.
    pub bounds: Rect,
    /// The scrollbar's track as of the last [`State::prepare`] (client pixels).
    pub track: Rect,
    /// The DPI of the last [`State::prepare`].
    pub dpi: u32,
    /// A thumb drag in progress: the pointer's y and the offset when it began.
    pub bar_drag: Option<(i32, i32)>,
}

impl State {
    pub fn new(shaper: Box<dyn TextShaper>, bounds: Rect, dpi: u32) -> State {
        State {
            doc: Document::new(),
            layout: Layout::new(),
            shaper,
            scroll: 0.0,
            viewport: 0.0,
            selection: None,
            bounds: bounds.at(0, 0),
            track: Rect::default(),
            dpi,
            bar_drag: None,
        }
    }

    /// Replaces the document and everything derived from it.
    pub fn set_document(&mut self, doc: Document) {
        self.doc = doc;
        self.layout = Layout::new();
        self.scroll = 0.0;
        self.selection = None;
    }

    /// Lays the document out for a view of `bounds` at `dpi`, places the bar
    /// and clamps the scroll offset. Returns the text area (the view without
    /// the bar). Cheap when nothing changed.
    pub fn prepare(&mut self, bounds: Rect, dpi: u32) -> Rect {
        self.bounds = bounds;
        self.dpi = dpi;
        let bar = THICKNESS.to_px(dpi).value().min(bounds.width());
        let text_area = Rect::new(bounds.left, bounds.top, bounds.right - bar, bounds.bottom);
        self.track = Rect::new(bounds.right - bar, bounds.top, bounds.right, bounds.bottom);
        let width = (text_area.width() - 2 * pad_px(dpi)).max(0) as f32;
        self.layout.set_metrics(width, dpi);
        self.layout.update(&self.doc, self.shaper.as_ref());
        self.viewport = bounds.height() as f32;
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
        text_area
    }

    /// The largest scroll offset.
    pub fn max_scroll(&self) -> f32 {
        (self.layout.height() - self.viewport).max(0.0)
    }
}
