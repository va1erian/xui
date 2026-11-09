#![forbid(unsafe_code)]

//! The widget: [`MarkdownView`], one custom node that paints a laid-out
//! [`Document`], scrolls it and follows its links.

mod events;
mod paint;
mod pointer;
mod selection;
mod shared;
mod state;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use xui_core::app::Ui;
use xui_core::backend::{NodeKind, NodeSpec, Result, WidgetId};
use xui_core::geometry::{Point, Rect, Size};
use xui_core::image::Image;
use xui_core::layout::Constraints;
use xui_core::theme::{Theme, Themed};
use xui_core::widget::{Control, Placeable};

use crate::highlight::Highlighter;
use crate::layout::Layout;
use crate::model::Document;
use crate::style::{CodePalette, MarkdownStyle};
use selection::Selection;
use shared::Shared;
use state::State;
/// A read-only view of a Markdown document: styled text, lists, quotes, code
/// blocks and tables, scrolled with the wheel, the keyboard and its own bar.
///
/// Text is shaped through the backend's [`TextShaper`](xui_core::backend::TextShaper)
/// and painted in the live theme's colours. Blocks on screen are laid out
/// first and the rest a slice at a time on a timer, so a long document opens
/// at once.
///
/// ```ignore
/// let view = MarkdownView::new(ui, bounds)?
///     .markdown("# Hello\n\nSome *styled* text and a [link](next.md).")
///     .on_link(|href| Some(Msg::Open(href.to_owned())));
/// ```
pub struct MarkdownView<M: 'static> {
    shared: Rc<Shared<M>>,
}

impl<M: 'static> MarkdownView<M> {
    /// Creates an empty view at `bounds` (device pixels).
    pub fn new(ui: &Ui<M>, bounds: Rect) -> Result<MarkdownView<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, bounds).tab_stop())?;
        let style = MarkdownStyle::default();
        let padding = style.padding;
        let layout = Layout::new(
            Arc::new(Document::default()),
            style,
            ui.text_shaper(),
            ui.dpi(),
        );
        let theme = ui.theme_handle();
        let shared = Rc::new_cyclic(|me: &Weak<Shared<M>>| {
            let painter = me.clone();
            control.set_painter(Rc::new(move |canvas| {
                if let Some(shared) = painter.upgrade() {
                    paint::paint(canvas, &mut shared.state.borrow_mut(), &theme.get());
                    // A paint may have sized the layout for the first time or
                    // reset it for a new DPI: lay out the rest in the background.
                    shared.kick();
                    shared.request_images();
                    shared.painted();
                }
            }));
            let mapper = me.clone();
            control.on_events(move |event| {
                let shared = mapper.upgrade()?;
                let out = events::handle(&mut shared.state.borrow_mut(), event)?;
                if out.relayout {
                    shared.kick();
                }
                shared.request_images();
                shared.deliver(out)
            });
            Shared {
                id: control.id(),
                control,
                me: me.clone(),
                state: RefCell::new(State::new(layout, padding, ui.dpi())),
                ui: ui.clone(),
                on_link: RefCell::new(None),
                on_context: RefCell::new(None),
                on_painted: RefCell::new(None),
                on_image: RefCell::new(None),
                unpainted: Cell::new(false),
                timer: Cell::new(None),
                drag_timer: Cell::new(None),
            }
        });
        Ok(MarkdownView { shared })
    }

    /// Shows `markdown`, parsed here.
    pub fn markdown(self, markdown: &str) -> MarkdownView<M> {
        self.set_markdown(markdown);
        self
    }

    /// Parses and shows `markdown`, scrolled to the top.
    pub fn set_markdown(&self, markdown: &str) {
        self.set_document(Arc::new(Document::parse(markdown)));
    }

    /// Shows a document parsed elsewhere (on a worker thread, say).
    pub fn document(self, document: Arc<Document>) -> MarkdownView<M> {
        self.set_document(document);
        self
    }

    /// Shows `document`, scrolled to the top.
    pub fn set_document(&self, document: Arc<Document>) {
        let mut state = self.shared.state.borrow_mut();
        state.layout.set_document(document);
        let captured = state.dragging || state.bar_drag.is_some();
        state.reset_scroll();
        drop(state);
        self.shared.unpainted.set(true);
        // A drag over the old text ends with it.
        if captured {
            self.shared.ui.release_capture();
        }
        self.shared.control.invalidate();
        self.shared.kick();
    }

    /// The document shown.
    pub fn current_document(&self) -> Arc<Document> {
        Arc::clone(self.shared.state.borrow().layout.doc())
    }

    /// Sets the fonts, sizes and spacing.
    pub fn style(self, style: MarkdownStyle) -> MarkdownView<M> {
        self.set_style(style);
        self
    }

    /// Changes the fonts, sizes and spacing, laying the document out again.
    pub fn set_style(&self, style: MarkdownStyle) {
        let mut state = self.shared.state.borrow_mut();
        state.padding = style.padding;
        state.layout.set_style(style);
        let captured = state.dragging || state.bar_drag.is_some();
        state.reset_scroll();
        drop(state);
        // A drag over the old text ends with it.
        if captured {
            self.shared.ui.release_capture();
        }
        self.shared.control.invalidate();
        self.shared.kick();
    }

    /// Colours code blocks with `highlighter`.
    pub fn highlighter(self, highlighter: impl Highlighter + 'static) -> MarkdownView<M> {
        self.set_highlighter(Some(Box::new(highlighter)));
        self
    }

    /// Colours code blocks with `highlighter`, or stops colouring them, laying
    /// the document out again.
    pub fn set_highlighter(&self, highlighter: Option<Box<dyn Highlighter>>) {
        self.shared
            .state
            .borrow_mut()
            .layout
            .set_highlighter(highlighter);
        self.shared.control.invalidate();
        self.shared.kick();
    }

    /// Colours highlighted code with `palette` instead of the theme's tokens.
    pub fn code_palette(self, palette: CodePalette) -> MarkdownView<M> {
        self.set_code_palette(Some(palette));
        self
    }

    /// Changes the code colours (`None` for the theme's); only repaints.
    pub fn set_code_palette(&self, palette: Option<CodePalette>) {
        self.shared.state.borrow_mut().palette = palette;
        self.shared.control.invalidate();
    }

    /// Maps a click on a link to a message, given its destination as
    /// [`Document::link`] gives it (escapes and entities resolved).
    /// A `#slug` link to one of the document's headings scrolls there and is
    /// not reported. The mapper must not call back into the view.
    pub fn on_link(self, mapper: impl Fn(&str) -> Option<M> + 'static) -> MarkdownView<M> {
        *self.shared.on_link.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps the first paint of each document shown to a message, queued
    /// once a paint with room for text has drawn the document's visible
    /// blocks (for an app that reports when a page is on screen; the frame
    /// is presented after the paint). A paint of a view with no room waits
    /// for the next. Only a new document re-arms it, not a new style or
    /// size. A document set before this is called counts too, if it has not
    /// been painted yet. The mapper must not call back into the view.
    pub fn on_painted(self, mapper: impl Fn() -> Option<M> + 'static) -> MarkdownView<M> {
        *self.shared.on_painted.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps each image the document shows to a message carrying its source
    /// as [`Document::images`] gives it (a relative path stays relative),
    /// for the app to load and decode, then hand over with
    /// [`set_image`](Self::set_image).
    ///
    /// The view loads nothing itself. A source is reported once per
    /// document, when the first block showing it is laid out: the visible
    /// blocks first, the rest as the background layout reaches them (a
    /// table's rows are measured together, so all its images come at once
    /// when any row shows). Until
    /// its pixels arrive, an image shows as its alt text. The mapper must
    /// not call back into the view.
    pub fn on_image(self, mapper: impl Fn(&str) -> Option<M> + 'static) -> MarkdownView<M> {
        *self.shared.on_image.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Shows `image` for every image of the current document whose source
    /// is `src`, in place of its alt text. One image pixel is one design
    /// unit, scaled down to fit the line. Returns whether the document has
    /// such an image; pixels for any other source are dropped, so a stale
    /// load for a document no longer shown does nothing.
    pub fn set_image(&self, src: &str, image: Image) -> bool {
        let found = self
            .shared
            .state
            .borrow_mut()
            .layout
            .set_image(src, &Arc::new(image));
        if found {
            self.shared.control.invalidate();
            self.shared.kick();
        }
        found
    }

    /// Maps a right-click to a message carrying the client point, for the
    /// app to show its context menu (with [`copy`](Self::copy) and
    /// [`select_all`](Self::select_all), say). The mapper must not call back
    /// into the view.
    pub fn on_context(self, mapper: impl Fn(Point) -> Option<M> + 'static) -> MarkdownView<M> {
        *self.shared.on_context.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The selected text, as [`copy`](Self::copy) puts it on the clipboard;
    /// empty when nothing is selected.
    pub fn selected_text(&self) -> String {
        let state = self.shared.state.borrow();
        state.layout.doc().plain_text(state.selection.range())
    }

    /// Whether any text is selected.
    pub fn has_selection(&self) -> bool {
        !self.shared.state.borrow().selection.is_empty()
    }

    /// Puts the selected text on the clipboard, as Ctrl+C does. Returns
    /// whether anything was selected.
    pub fn copy(&self) -> bool {
        self.shared.copy()
    }

    /// Selects the whole document, as Ctrl+A does.
    pub fn select_all(&self) {
        let mut state = self.shared.state.borrow_mut();
        let end = state.layout.doc().text().len() as u32;
        state.selection = Selection::of(0..end);
        state.anchored = true;
        drop(state);
        self.shared.control.invalidate();
    }

    /// Clears the selection.
    pub fn clear_selection(&self) {
        self.shared.state.borrow_mut().clear_selection();
        self.shared.control.invalidate();
    }

    /// Scrolls the heading whose slug is `slug` (without `#`) to the top.
    /// Returns whether there is one.
    pub fn scroll_to_anchor(&self, slug: &str) -> bool {
        let mut state = self.shared.state.borrow_mut();
        let Some(block) = state.layout.doc().anchor(slug) else {
            return false;
        };
        state.scroll_to_block(block);
        drop(state);
        self.shared.control.invalidate();
        true
    }

    /// Scrolls to `offset` device pixels from the top, clamped to the content.
    pub fn set_scroll(&self, offset: f32) {
        self.shared.state.borrow_mut().set_scroll(offset);
        self.shared.control.invalidate();
    }

    /// The scroll offset in device pixels.
    pub fn scroll(&self) -> f32 {
        self.shared.state.borrow().scroll
    }

    /// Takes the keyboard focus.
    pub fn focus(&self) {
        self.shared.control.focus();
    }

    /// Moves and resizes the view (device pixels).
    pub fn set_bounds(&self, bounds: Rect) {
        self.shared.control.set_bounds(bounds);
        self.shared.kick();
    }

    /// The view's node.
    pub fn id(&self) -> WidgetId {
        self.shared.control.id()
    }
}

/// The view fills whatever slot a layout gives it; it scrolls, so it has no
/// natural size of its own.
impl<M: 'static> Placeable<M> for MarkdownView<M> {
    fn id(&self) -> WidgetId {
        self.shared.control.id()
    }

    fn measure(&self, _ui: &Ui<M>, _constraints: Constraints) -> Size {
        Size::new(0, 0)
    }
}

/// Colours are resolved from the live theme when painting, so a theme change
/// only repaints.
impl<M: 'static> Themed for MarkdownView<M> {
    fn apply_theme(&self, _theme: &Theme) {
        self.shared.control.invalidate();
    }
}
