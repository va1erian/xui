#![forbid(unsafe_code)]

//! The widget: [`RichTextEditor`], one custom node that paints a laid-out
//! [`Document`] and edits it from keyboard and mouse input.
//!
//! Input is mapped to [`Command`]s run by the editing controller
//! ([`EditorState`](crate::edit::EditorState)); the view relays out what
//! changed, keeps the caret in view and tells the app through the mappers it
//! was built with.

mod coords;
mod drag;
mod events;
mod exec;
mod keys;
mod mouse;
mod overlay;
mod paint;
mod scroll;
mod shared;
mod state;

use std::rc::Rc;

use xui_core::app::Ui;
use xui_core::backend::{NodeKind, NodeSpec, Result};
use xui_core::geometry::{Point, Rect};
use xui_core::theme::{Theme, Themed};
use xui_core::widget::Control;

use crate::edit::{Clipboard, Command};
use crate::model::{Affinity, DocPos, Document, Selection, StyleSummary};
use events::Out;
use shared::Shared;

/// How often the caret timer ticks, in milliseconds; the caret blinks every
/// [`BLINK_TICKS`] ticks and an image drag autoscrolls on each.
const TICK_MS: u32 = 50;
/// Timer ticks per caret blink phase.
const BLINK_TICKS: u32 = 10;

/// A rich-text editor on a custom xui node.
///
/// Typing, caret movement, selection, clipboard, undo, formatting commands
/// ([`exec`](RichTextEditor::exec)) and image selection, resizing and moving
/// work from keyboard and mouse. The app learns about edits through the
/// mappers given at construction.
pub struct RichTextEditor<M: 'static> {
    control: Control<M>,
    shared: Rc<Shared<M>>,
}

impl<M: 'static> RichTextEditor<M> {
    /// Creates an empty editor at `bounds` (device pixels), copying and
    /// pasting through the window's portable clipboard.
    pub fn new(ui: &Ui<M>, bounds: Rect) -> Result<RichTextEditor<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, bounds).tab_stop())?;
        let shared = Rc::new(Shared::new(ui, control.id(), bounds));
        {
            let shared = Rc::clone(&shared);
            let theme = ui.theme_handle();
            control.set_painter(Rc::new(move |canvas| {
                paint::paint(canvas, &mut shared.state.borrow_mut(), &theme.get());
            }));
        }
        {
            let shared = Rc::clone(&shared);
            control.on_events(move |event| {
                let out = {
                    let mut state = shared.state.borrow_mut();
                    if event.is_input() {
                        state.ready();
                    }
                    events::handle(&mut state, event)?
                };
                shared.deliver(out)
            });
        }
        {
            let shared = Rc::clone(&shared);
            let ticks = std::cell::Cell::new(0u32);
            // `None` when the backend has no timers (the offscreen one): the
            // caret then stays solid and drags do not autoscroll.
            let _ = control.set_timer(TICK_MS, move || {
                ticks.set(ticks.get().wrapping_add(1));
                let mut state = shared.state.borrow_mut();
                let mut repaint = state.autoscroll();
                if state.focused && ticks.get().is_multiple_of(BLINK_TICKS) {
                    state.caret_on = !state.caret_on;
                    repaint = true;
                }
                drop(state);
                if repaint {
                    shared.ui.invalidate(shared.id);
                }
                None
            });
        }
        Ok(RichTextEditor { control, shared })
    }

    /// Maps every change of the document to a message. The mapper must not
    /// call back into the editor.
    pub fn on_change(self, mapper: impl Fn(&Document) -> Option<M> + 'static) -> RichTextEditor<M> {
        *self.shared.on_change.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps every change of the selection, or of the formatting under it, to a
    /// message carrying the formatting summary a toolbar shows. The mapper
    /// must not call back into the editor.
    pub fn on_selection(
        self,
        mapper: impl Fn(&StyleSummary) -> Option<M> + 'static,
    ) -> RichTextEditor<M> {
        *self.shared.on_selection.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a Ctrl+click on a link to a message carrying its target.
    pub fn on_link(self, mapper: impl Fn(&str) -> Option<M> + 'static) -> RichTextEditor<M> {
        *self.shared.on_link.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Replaces the clipboard (the default is the window's portable one).
    pub fn with_clipboard(self, clipboard: Box<dyn Clipboard>) -> RichTextEditor<M> {
        self.shared.state.borrow_mut().clipboard = clipboard;
        self
    }

    /// Sets the document shown.
    pub fn document(self, document: Document) -> RichTextEditor<M> {
        self.set_document(document);
        self
    }

    /// Replaces the document, resetting history, selection and scroll. The
    /// mappers do not fire.
    pub fn set_document(&self, document: Document) {
        self.shared.state.borrow_mut().set_document(document);
        self.control.invalidate();
    }

    /// Reads the document.
    pub fn with_document<R>(&self, f: impl FnOnce(&Document) -> R) -> R {
        f(&self.shared.state.borrow().ed.doc)
    }

    /// The current selection.
    pub fn selection(&self) -> Selection {
        self.shared.state.borrow().ed.selection
    }

    /// Takes the keyboard focus.
    pub fn focus(&self) {
        self.control.focus();
    }

    /// Runs a command as one undoable step, as a toolbar or menu would, and
    /// fires the mappers for what it changed.
    pub fn exec(&self, command: Command) {
        let mut out = Out::default();
        {
            let mut state = self.shared.state.borrow_mut();
            out.absorb(&state.run(command));
        }
        let ui = &self.shared.ui;
        if let Some(message) = self.shared.deliver(out) {
            ui.emit(message);
        }
    }

    /// Scrolls to `offset` device pixels from the top; the next paint clamps
    /// it to the content.
    pub fn set_scroll(&self, offset: f32) {
        self.shared.state.borrow_mut().scroll = offset.max(0.0);
        self.control.invalidate();
    }

    /// The caret box at `pos` in the node's client pixels, with the text
    /// margin and scroll applied (so it can be drawn or handed to an IME).
    pub fn caret_rect(&self, pos: DocPos, affinity: Affinity) -> Rect {
        let mut state = self.shared.state.borrow_mut();
        state.ready();
        state.caret_rect_view(pos, affinity)
    }

    /// The position nearest the client pixel `point`.
    pub fn pos_at(&self, point: Point) -> DocPos {
        let mut state = self.shared.state.borrow_mut();
        state.ready();
        state.pos_at_view(point)
    }

    /// Scrolls the least that brings the caret at `pos` into view.
    pub fn ensure_caret_visible(&self, pos: DocPos) {
        let mut state = self.shared.state.borrow_mut();
        state.ready();
        let caret = state.layout.caret_rect(&state.ed.doc, pos);
        state.ensure_visible(caret);
        drop(state);
        self.control.invalidate();
    }

    /// Moves and resizes the editor (device pixels).
    pub fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
    }
}

impl<M: 'static> Themed for RichTextEditor<M> {
    fn apply_theme(&self, _theme: &Theme) {
        self.control.invalidate();
    }
}
