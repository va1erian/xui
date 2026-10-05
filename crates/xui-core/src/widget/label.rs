#![forbid(unsafe_code)]

//! [`Label`]: a painted, non-interactive text widget.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::Rect;
use crate::property::{Properties, Property, Value};
use crate::theme::look::backdrop;
use crate::units::Dip;

/// The design size of the label text.
const TEXT_SIZE: Dip = Dip(12.0);
/// The design size of a [`Label::title`].
const TITLE_SIZE: Dip = Dip(20.0);
/// The design size of a [`Label::caption`].
const CAPTION_SIZE: Dip = Dip(12.0);

/// How a label sets its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Body text.
    Body,
    /// A page title: large and bold.
    Title,
    /// A section caption: small, bold, upper case, in secondary text.
    Caption,
}

impl Kind {
    /// The style the label paints (and is measured) in.
    fn style(self, theme: &crate::theme::Theme) -> TextStyle {
        match self {
            Kind::Body => TextStyle::new(theme.text, TEXT_SIZE),
            Kind::Title => TextStyle::new(theme.text, TITLE_SIZE).bold().middle(),
            Kind::Caption => TextStyle::new(theme.text_secondary, CAPTION_SIZE)
                .bold()
                .middle(),
        }
    }

    /// The text as painted: a caption is upper case.
    fn shown(self, text: &str) -> String {
        match self {
            Kind::Caption => text.to_uppercase(),
            Kind::Body | Kind::Title => text.to_string(),
        }
    }
}

/// A painted static text label.
pub struct Label<M: 'static> {
    control: Control<M>,
    text: Rc<RefCell<String>>,
    kind: Rc<Cell<Kind>>,
}

impl<M: 'static> Label<M> {
    /// Creates a label showing `text` at `bounds`.
    pub(crate) fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<Label<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Label, bounds).text(text))?;
        let state = Rc::new(RefCell::new(text.to_string()));
        let theme = ui.theme_handle();
        let selected = control.selected_handle();
        let text_for_paint = Rc::clone(&state);
        let kind = Rc::new(Cell::new(Kind::Body));
        let kind_for_paint = Rc::clone(&kind);
        control.set_painter(Rc::new(move |canvas| {
            let theme = theme.get();
            let bounds = canvas.bounds();
            // The node is an opaque child window: paint its background first,
            // or the back buffer shows through around the text.
            backdrop(canvas, theme.background);
            let kind = kind_for_paint.get();
            let text = kind.shown(&text_for_paint.borrow());
            canvas.draw_text(&text, bounds, &kind.style(&theme));
            if selected.get() {
                canvas.stroke_rect(bounds, theme.accent, 2.0);
            }
        }));
        Ok(Label {
            control,
            text: state,
            kind,
        })
    }

    /// Sets the label as a page title: large, bold, vertically centred.
    pub fn title(self) -> Label<M> {
        self.kind.set(Kind::Title);
        self.control.invalidate();
        self
    }

    /// Sets the label as a section caption: small, bold, upper case and in
    /// secondary text, the heading above a card of settings.
    pub fn caption(self) -> Label<M> {
        self.kind.set(Kind::Caption);
        self.control.invalidate();
        self
    }

    /// The size of the label's text as painted, at `dpi`: a title's larger
    /// bold face and a caption's upper case included.
    pub(super) fn text_size(&self, ui: &Ui<M>, dpi: u32) -> crate::geometry::Size {
        let kind = self.kind.get();
        let metrics = ui.measure_text(
            &kind.shown(&self.text.borrow()),
            &kind.style(&ui.theme()),
            dpi,
        );
        crate::geometry::Size::new(metrics.width, metrics.height)
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
        // Apps refresh status lines with the text they already show: an
        // unchanged text needs neither a repaint nor a re-flow.
        if *self.text.borrow() == text {
            return;
        }
        *self.text.borrow_mut() = text.to_string();
        self.control.invalidate();
        self.control.invalidate_layout();
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
