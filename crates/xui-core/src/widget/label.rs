#![forbid(unsafe_code)]

//! [`Label`]: a painted, non-interactive text widget.

use std::cell::RefCell;
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::Rect;
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

/// The design size of the label text.
const TEXT_SIZE: Dip = Dip(12.0);

/// A painted static text label.
pub struct Label<M: 'static> {
    control: Control<M>,
    text: Rc<RefCell<String>>,
}

impl<M: 'static> Label<M> {
    /// Creates a label with no bounds of its own, for a layout to place (see
    /// [`crate::arrange`]); its size comes from [`Placeable`](super::Placeable).
    pub fn auto(ui: &Ui<M>, text: &str) -> Result<Label<M>> {
        Label::new(ui, Rect::default(), text)
    }

    /// Creates a label showing `text` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<Label<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Label, bounds).text(text))?;
        let state = Rc::new(RefCell::new(text.to_string()));
        let theme = ui.theme_handle();
        let selected = control.selected_handle();
        let text_for_paint = Rc::clone(&state);
        control.set_painter(Rc::new(move |canvas| {
            let theme = theme.get();
            let bounds = canvas.bounds();
            // The node is an opaque child window: paint its background first,
            // or the back buffer shows through around the text.
            canvas.clear(theme.background);
            let style = TextStyle::new(theme.text, TEXT_SIZE);
            canvas.draw_text(&text_for_paint.borrow(), bounds, &style);
            if selected.get() {
                canvas.stroke_rect(bounds, theme.accent, 2.0);
            }
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

    /// Marks the label selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }

    /// Whether the label is selected.
    pub fn is_selected(&self) -> bool {
        self.control.is_selected()
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

impl<M: 'static> Properties for Label<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "text",
            value: Value::Text(self.text()),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("text", Value::Text(text)) => {
                self.set_text(&text);
                true
            }
            _ => false,
        }
    }
}
