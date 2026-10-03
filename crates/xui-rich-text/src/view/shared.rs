#![forbid(unsafe_code)]

//! What the painter, the event mapper and the timer share: the state, the app
//! mappers and the window handle used to act on a result.

use std::cell::RefCell;

use xui_core::app::Ui;
use xui_core::backend::WidgetId;
use xui_core::geometry::Rect;

use super::events::Out;
use super::state::State;
use crate::edit::{Clipboard, EditorState};
use crate::model::{Document, StyleSummary, Tri};

type LinkMapper<M> = Box<dyn Fn(&str) -> Option<M>>;
type Mapper<T, M> = RefCell<Option<Box<dyn Fn(&T) -> Option<M>>>>;

/// The clipboard of the window `ui` belongs to.
struct WindowClipboard<M: 'static>(Ui<M>);

impl<M: 'static> Clipboard for WindowClipboard<M> {
    fn text(&self) -> Option<String> {
        self.0.clipboard_text()
    }

    fn set_text(&self, text: &str) {
        self.0.set_clipboard_text(text);
    }
}

pub(crate) struct Shared<M: 'static> {
    pub state: RefCell<State>,
    pub ui: Ui<M>,
    pub id: WidgetId,
    pub on_change: Mapper<Document, M>,
    pub on_selection: Mapper<StyleSummary, M>,
    pub on_link: RefCell<Option<LinkMapper<M>>>,
}

impl<M: 'static> Shared<M> {
    pub fn new(ui: &Ui<M>, id: WidgetId, bounds: Rect) -> Shared<M> {
        let state = State::new(
            ui.text_shaper(),
            Box::new(WindowClipboard(ui.clone())),
            bounds,
            ui.dpi(),
        );
        Shared {
            state: RefCell::new(state),
            ui: ui.clone(),
            id,
            on_change: RefCell::new(None),
            on_selection: RefCell::new(None),
            on_link: RefCell::new(None),
        }
    }

    /// Acts on a handled event outside the state borrow (focus and capture
    /// deliver events straight back to the mapper), and maps what changed to
    /// messages. The first is returned and the rest queued, in the order link,
    /// change, selection.
    pub fn deliver(&self, out: Out) -> Option<M> {
        if out.focus {
            self.ui.focus(self.id);
        }
        match out.capture {
            Some(true) => self.ui.set_capture(self.id),
            Some(false) => self.ui.release_capture(),
            None => {}
        }
        if let Some(cursor) = out.cursor {
            self.ui.set_cursor(self.id, cursor);
        }
        if out.invalidate {
            self.ui.invalidate(self.id);
        }
        let mut messages = Vec::new();
        if let Some(url) = &out.link
            && let Some(mapper) = self.on_link.borrow().as_ref()
        {
            messages.extend(mapper(url));
        }
        if out.changed || out.selection {
            let state = self.state.borrow();
            if out.changed
                && let Some(mapper) = self.on_change.borrow().as_ref()
            {
                messages.extend(mapper(&state.ed.doc));
            }
            if out.selection
                && let Some(mapper) = self.on_selection.borrow().as_ref()
            {
                messages.extend(mapper(&summary(&state.ed)));
            }
        }
        let mut messages = messages.into_iter();
        let first = messages.next();
        messages.for_each(|message| self.ui.emit(message));
        first
    }
}

/// The formatting under the selection, with a style change pending at a
/// caret already applied (what the next typed character will get).
fn summary(ed: &EditorState) -> StyleSummary {
    let mut summary = ed.doc.style_summary(&ed.selection);
    if let Some(pending) = &ed.pending {
        if let Some(on) = pending.bold {
            summary.bold = Tri::Uniform(on);
        }
        if let Some(on) = pending.italic {
            summary.italic = Tri::Uniform(on);
        }
        if let Some(on) = pending.underline {
            summary.underline = Tri::Uniform(on);
        }
        if let Some(on) = pending.strike {
            summary.strike = Tri::Uniform(on);
        }
        if let Some(size) = pending.size {
            summary.size = Tri::Uniform(size);
        }
        if let Some(color) = pending.color {
            summary.color = Tri::Uniform(color);
        }
    }
    summary
}
