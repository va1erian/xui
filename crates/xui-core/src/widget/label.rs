#![forbid(unsafe_code)]

//! [`Label`]: a painted, non-interactive text widget.

use std::cell::RefCell;
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::Rect;
use crate::units::Dip;

/// The design size of the label text.
const TEXT_SIZE: Dip = Dip(12.0);

/// A painted static text label.
pub struct Label<M: 'static> {
    control: Control<M>,
    text: Rc<RefCell<String>>,
}

impl<M: 'static> Label<M> {
    /// Creates a label showing `text` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<Label<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Label, bounds).text(text))?;
        let state = Rc::new(RefCell::new(text.to_string()));
        let theme = ui.theme_handle();
        let text_for_paint = Rc::clone(&state);
        control.set_painter(Rc::new(move |canvas| {
            let theme = theme.get();
            // The node is an opaque child window: paint its background first,
            // or the back buffer shows through around the text.
            canvas.clear(theme.background);
            let style = TextStyle::new(theme.text, TEXT_SIZE);
            canvas.draw_text(&text_for_paint.borrow(), canvas.bounds(), &style);
        }));
        Ok(Label {
            control,
            text: state,
        })
    }

    /// The label's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }
}

impl<M: 'static> HasText for Label<M> {
    fn text(&self) -> String {
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = text.to_string();
        self.control.invalidate();
    }
}
