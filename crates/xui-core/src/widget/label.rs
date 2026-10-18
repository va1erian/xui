#![forbid(unsafe_code)]

//! [`Label`]: a painted, non-interactive text widget.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::Rect;
use crate::property::{Font, Properties, Property, PropertyInfo, PropertyType, Value};
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
    /// The style the label paints (and is measured) in: the kind's theme
    /// style with the label's [`Font`] override, if any, applied.
    fn style(self, theme: &crate::theme::Theme, font: Option<&Font>) -> TextStyle {
        let base = self.font();
        let color = match self {
            Kind::Body | Kind::Title => theme.text,
            Kind::Caption => theme.text_secondary,
        };
        let size = font.map_or(base.size, |font| font.size);
        let bold = base.bold || font.is_some_and(|font| font.bold);
        let mut style = TextStyle::new(color, size);
        if bold {
            style = style.bold();
        }
        if let Some(font) = font {
            style = style.italic(font.italic);
            if let Some(family) = &font.family {
                style = style.family(family.clone());
            }
        }
        match self {
            Kind::Body => style,
            Kind::Title | Kind::Caption => style.middle(),
        }
    }

    /// The font the kind paints in when the label has no override.
    fn font(self) -> Font {
        match self {
            Kind::Body => Font::new(TEXT_SIZE),
            Kind::Title => Font::new(TITLE_SIZE).bold(),
            Kind::Caption => Font::new(CAPTION_SIZE).bold(),
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
    /// A per-label font override; `None` follows the kind's theme style.
    font: Rc<RefCell<Option<Font>>>,
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
        let font: Rc<RefCell<Option<Font>>> = Rc::new(RefCell::new(None));
        let font_for_paint = Rc::clone(&font);
        control.set_painter(Rc::new(move |canvas| {
            let theme = theme.get();
            let bounds = canvas.bounds();
            // The node is an opaque child window: paint its background first,
            // or the back buffer shows through around the text.
            backdrop(canvas, theme.background);
            let kind = kind_for_paint.get();
            let text = kind.shown(&text_for_paint.borrow());
            let style = kind.style(&theme, font_for_paint.borrow().as_ref());
            canvas.draw_text(&text, bounds, &style);
            if selected.get() {
                canvas.stroke_rect(bounds, theme.accent, 2.0);
            }
        }));
        Ok(Label {
            control,
            text: state,
            kind,
            font,
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

    /// Paints the label in `font` instead of the kind's theme style.
    pub fn font(self, font: Font) -> Label<M> {
        self.set_font(Some(font));
        self
    }

    /// Sets (`Some`) or clears (`None`) the label's font override. A change
    /// repaints and re-flows, since the text's measured size may change.
    pub fn set_font(&self, font: Option<Font>) {
        if *self.font.borrow() == font {
            return;
        }
        *self.font.borrow_mut() = font;
        self.control.invalidate();
        self.control.invalidate_layout();
    }

    /// The font the label paints in: its override, merged with the kind's
    /// weight (a title or a caption stays bold), or the kind's theme font.
    pub fn current_font(&self) -> Font {
        let kind = self.kind.get();
        match &*self.font.borrow() {
            Some(font) => Font {
                bold: font.bold || kind.font().bold,
                ..font.clone()
            },
            None => kind.font(),
        }
    }

    /// The size of the label's text as painted, at `dpi`: a title's larger
    /// bold face and a caption's upper case included.
    pub(super) fn text_size(&self, ui: &Ui<M>, dpi: u32) -> crate::geometry::Size {
        let kind = self.kind.get();
        let metrics = ui.measure_text(
            &kind.shown(&self.text.borrow()),
            &kind.style(&ui.theme(), self.font.borrow().as_ref()),
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
        vec![
            Property {
                name: "text",
                value: Value::Text(self.text()),
            },
            Property {
                name: "font",
                value: Value::Font(self.current_font()),
            },
        ]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("text", Value::Text(text)) => {
                self.set_text(&text);
                true
            }
            ("font", Value::Font(font)) => {
                self.set_font(Some(font));
                true
            }
            _ => false,
        }
    }

    fn schema(&self) -> Vec<PropertyInfo> {
        vec![
            PropertyInfo::new("text", PropertyType::Text)
                .described("Appearance", "The label's text."),
            PropertyInfo::new("font", PropertyType::Font)
                .described("Appearance", "The font the label paints in."),
        ]
    }
}
