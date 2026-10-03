#![forbid(unsafe_code)]

//! The widget: [`RichTextEditor`], one custom node that paints a laid-out
//! [`Document`] through the portable canvas.

mod paint;
mod scroll;
mod state;

use std::cell::RefCell;
use std::rc::Rc;

use xui_core::app::Ui;
use xui_core::backend::{Event, NodeKind, NodeSpec, Result};
use xui_core::geometry::Rect;
use xui_core::message::MouseButton;
use xui_core::theme::{Theme, Themed};
use xui_core::widget::Control;

use crate::model::{DocPos, Document};
use state::State;

/// How far one wheel notch scrolls, in design units.
const WHEEL_DIP: f32 = 48.0;

/// A rich-text view on a custom xui node.
pub struct RichTextEditor<M: 'static> {
    control: Control<M>,
    state: Rc<RefCell<State>>,
}

impl<M: 'static> RichTextEditor<M> {
    /// Creates an empty editor at `bounds` (device pixels).
    pub fn new(ui: &Ui<M>, bounds: Rect) -> Result<RichTextEditor<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, bounds))?;
        let state = Rc::new(RefCell::new(State::new(ui.text_shaper(), bounds, ui.dpi())));
        {
            let state = Rc::clone(&state);
            let theme = ui.theme_handle();
            control.set_painter(Rc::new(move |canvas| {
                paint::paint(canvas, &mut state.borrow_mut(), &theme.get());
            }));
        }
        {
            let state = Rc::clone(&state);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                let mut state = state.borrow_mut();
                if let Event::Resize { width, height } = *event {
                    state.bounds = Rect::new(0, 0, width, height);
                }
                if event.is_input() {
                    // The first input can arrive before the first paint.
                    let (bounds, dpi) = (state.bounds, ui.dpi());
                    state.prepare(bounds, dpi);
                }
                let mut capture = None;
                let handled = match *event {
                    Event::MouseWheel {
                        delta,
                        horizontal: false,
                        ..
                    } => {
                        let step = f32::from(delta) / 120.0 * WHEEL_DIP * state.dpi as f32 / 96.0;
                        state.scroll_by(-step);
                        true
                    }
                    Event::MouseDown {
                        x,
                        y,
                        button: MouseButton::Left,
                        ..
                    } if state.on_bar(x, y) => {
                        capture = state.press_bar(y).then_some(true);
                        true
                    }
                    Event::MouseMove { y, .. } => state.drag_bar(y),
                    Event::MouseUp {
                        button: MouseButton::Left,
                        ..
                    }
                    | Event::CaptureChanged => {
                        capture = state.release_bar().then_some(false);
                        capture.is_some()
                    }
                    _ => false,
                };
                drop(state);
                match capture {
                    Some(true) => ui.set_capture(id),
                    Some(false) => ui.release_capture(),
                    None => {}
                }
                if handled {
                    ui.invalidate(id);
                }
                None
            });
        }
        Ok(RichTextEditor { control, state })
    }

    /// Sets the document shown.
    pub fn document(self, document: Document) -> RichTextEditor<M> {
        self.set_document(document);
        self
    }

    /// Replaces the document shown, resetting scroll and selection.
    pub fn set_document(&self, document: Document) {
        self.state.borrow_mut().set_document(document);
        self.control.invalidate();
    }

    /// Selects from `anchor` to `head`, or clears the selection.
    pub fn set_selection(&self, selection: Option<(DocPos, DocPos)>) {
        self.state.borrow_mut().selection = selection;
        self.control.invalidate();
    }

    /// Scrolls to `offset` device pixels from the top; the next paint clamps
    /// it to the content.
    pub fn set_scroll(&self, offset: f32) {
        self.state.borrow_mut().scroll = offset.max(0.0);
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
