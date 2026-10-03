#![forbid(unsafe_code)]

//! The editor's mutable state, shared by its painter and event mapper.

use std::time::Instant;

use xui_core::Dip;
use xui_core::backend::{Cursor, TextShaper};
use xui_core::geometry::{Point, Rect};
use xui_core::widget::scrollbar::THICKNESS;

use super::drag::Drag;
use super::sheets::Sheets;
use crate::edit::{Clipboard, EditorState};
use crate::layout::Layout;
use crate::model::{Document, ObjectId};

/// Documents of at most this many paragraphs are always laid out whole.
const EAGER_PARAGRAPHS: usize = 100;
/// The most paragraphs laid out at once around the view.
const WINDOW_BUDGET: usize = 128;

/// The margin between the view's edge and the text, on each side.
const PAD: Dip = Dip(8.0);

/// The text margin in device pixels at `dpi`.
pub(crate) fn pad_px(dpi: u32) -> i32 {
    PAD.to_px(dpi).value()
}

/// How the document is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    /// One continuous column as wide as the view (the default).
    #[default]
    Draft,
    /// The pages the document would print on: sheets of its
    /// [`PageSetup`](crate::model::PageSetup) on a desk, the text inside their
    /// margins, shrunk to fit when the view is narrower than a sheet.
    Page,
}

/// The previous click, for double and triple clicks.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Click {
    pub at: Instant,
    pub point: Point,
    /// 1 for a single click, 2 after a double click, 3 after a triple.
    pub count: u8,
}

/// The document being edited, its layout and the view onto it.
pub(crate) struct State {
    pub ed: EditorState,
    pub layout: Layout,
    pub shaper: Box<dyn TextShaper>,
    pub clipboard: Box<dyn Clipboard>,
    /// When the editor was made; the monotonic origin commands are timed from.
    pub epoch: Instant,
    /// Vertical scroll offset in device pixels.
    pub scroll: f32,
    /// The viewport height in device pixels, as of the last [`State::prepare`].
    pub viewport: f32,
    /// The node's size at the origin, as last reported.
    pub bounds: Rect,
    /// The scrollbar's track as of the last [`State::prepare`] (client pixels).
    pub track: Rect,
    /// The DPI of the last [`State::prepare`].
    pub dpi: u32,
    /// A thumb drag in progress: the pointer's y and the offset when it began.
    pub bar_drag: Option<(i32, i32)>,
    /// Whether the editor has the keyboard focus.
    pub focused: bool,
    /// The blink phase of the caret.
    pub caret_on: bool,
    /// The mouse drag in progress.
    pub drag: Option<Drag>,
    /// The last click.
    pub last_click: Option<Click>,
    /// The last pointer position in client pixels, for autoscroll.
    pub pointer: Point,
    /// The cursor last asked for.
    pub cursor: Cursor,
    /// The selected image and where it is drawn (layout pixels).
    pub image: Option<(ObjectId, Rect)>,
    /// The caret shown at the drop point while dragging an image (layout
    /// pixels).
    pub drop_caret: Option<Rect>,
    /// The guide shown at a table column edge while it is dragged (layout
    /// pixels).
    pub column_guide: Option<Rect>,
    /// Timer ticks so far (the blink phase counts them).
    pub ticks: u32,
    /// Draft or page view.
    pub mode: ViewMode,
    /// Where the flow's origin is in client pixels before scrolling, as of
    /// the last [`State::prepare`].
    pub origin: Point,
    /// The sheets, in page view.
    pub sheets: Option<Sheets>,
}

impl State {
    pub fn new(
        shaper: Box<dyn TextShaper>,
        clipboard: Box<dyn Clipboard>,
        bounds: Rect,
        dpi: u32,
    ) -> State {
        State {
            ed: EditorState::new(Document::new()),
            layout: Layout::new(),
            shaper,
            clipboard,
            epoch: Instant::now(),
            scroll: 0.0,
            viewport: 0.0,
            bounds: bounds.at(0, 0),
            track: Rect::default(),
            dpi,
            bar_drag: None,
            focused: false,
            caret_on: true,
            drag: None,
            last_click: None,
            pointer: Point::new(0, 0),
            cursor: Cursor::Text,
            image: None,
            drop_caret: None,
            column_guide: None,
            ticks: 0,
            mode: ViewMode::Draft,
            origin: Point::new(pad_px(dpi), pad_px(dpi)),
            sheets: None,
        }
    }

    /// Replaces the document, resetting history, selection, scroll and layout.
    pub fn set_document(&mut self, doc: Document) {
        self.ed = EditorState::new(doc);
        self.layout = Layout::new();
        self.scroll = 0.0;
        self.drag = None;
        self.image = None;
        self.drop_caret = None;
        self.column_guide = None;
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
        match self.mode {
            ViewMode::Draft => {
                let width = (text_area.width() - 2 * pad_px(dpi)).max(0) as f32;
                self.layout.set_pages(None);
                self.layout.set_metrics(width, dpi);
                self.origin = Point::new(text_area.left + pad_px(dpi), pad_px(dpi));
                self.sheets = None;
            }
            ViewMode::Page => self.prepare_sheets(text_area, dpi),
        }
        self.viewport = bounds.height() as f32;
        self.layout_window();
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
        text_area
    }

    /// Switches between draft and page view, keeping the caret in view.
    pub fn set_mode(&mut self, mode: ViewMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.ready();
        if let crate::model::Selection::Text { head, .. } = self.ed.selection {
            self.ensure_para(head.para);
            let caret = self.layout.caret_rect(&self.ed.doc, head);
            self.ensure_visible(caret);
        }
    }

    /// Brings the layout up to date: all of it for a short document, else the
    /// paragraphs around the view (the timer finishes the rest).
    fn layout_window(&mut self) {
        let (doc, shaper) = (&self.ed.doc, self.shaper.as_ref());
        if doc.paragraph_count() <= EAGER_PARAGRAPHS {
            self.layout.update(doc, shaper);
        } else {
            let top = self.shift() as f32;
            let reach = self.viewport;
            let (from, to) = (top - reach, top + 2.0 * reach);
            self.layout
                .update_around(doc, shaper, from, to, WINDOW_BUDGET);
        }
    }

    /// Makes sure paragraph `para` and its neighbourhood are laid out (a
    /// caret or an image far from the view needs real lines).
    pub fn ensure_para(&mut self, para: usize) {
        let Some(p) = self.layout.paragraphs().get(para) else {
            return;
        };
        if p.dirty {
            let (top, bottom) = (p.y - self.viewport, p.bottom() + self.viewport);
            let (doc, shaper) = (&self.ed.doc, self.shaper.as_ref());
            self.layout
                .update_around(doc, shaper, top, bottom, WINDOW_BUDGET);
        }
    }

    /// The offset from view y to layout y: the scroll less the top margin.
    pub fn shift(&self) -> i32 {
        self.scroll.round() as i32 - self.origin.y
    }

    /// The scrollable height: the flow and a margin above and below, or the
    /// sheets and the desk around them.
    pub fn content_height(&self) -> f32 {
        match self.sheets {
            Some(s) => s.top + self.layout.page_count() as f32 * s.pitch,
            None => self.layout.height() + 2.0 * pad_px(self.dpi) as f32,
        }
    }

    /// The page the caret is on (from 0) and the number of pages; one page
    /// in draft view.
    pub fn page_info(&mut self) -> (usize, usize) {
        let page = match self.ed.selection {
            crate::model::Selection::Text { head, .. } => {
                self.ensure_para(head.para);
                let caret = self.layout.caret_rect(&self.ed.doc, head);
                self.layout.page_at(caret.top as f32)
            }
            crate::model::Selection::Object(id) => self
                .ed
                .doc
                .object_pos(id)
                .and_then(|pos| self.layout.paragraphs().get(pos.para))
                .map_or(0, |p| self.layout.page_at(p.y)),
        };
        let count = self.layout.page_count();
        (page.min(count - 1), count)
    }

    /// The largest scroll offset.
    pub fn max_scroll(&self) -> f32 {
        (self.content_height() - self.viewport).max(0.0)
    }
}
