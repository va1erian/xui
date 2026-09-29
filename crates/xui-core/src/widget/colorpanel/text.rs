#![forbid(unsafe_code)]

//! The editable text boxes of a [`ColorPanel`](super::ColorPanel): the HEX,
//! RGB, CMYK, HSV and HSL read-outs.
//!
//! Each box parses and formats one view of the panel's [`Hsv`]. A box updates
//! the model live while it holds valid text, and commits on Enter or focus
//! loss; invalid text is reverted there, so no half-applied state is left.

use crate::app::Ui;
use crate::backend::Event;
use crate::message::Key;
use crate::widget::HasText;

use super::Shared;
use super::model::{
    Hsv, format_cmyk, format_hex, format_hsl, format_hsv, format_rgb, parse_cmyk, parse_hex,
    parse_hsl, parse_hsv, parse_rgb,
};

/// Which view an editable box shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    /// `#rrggbb`.
    Hex,
    /// `r, g, b`.
    Rgb,
    /// `c%, m%, y%, k%`.
    Cmyk,
    /// `h°, s%, v%`.
    Hsv,
    /// `h°, s%, l%`.
    Hsl,
}

impl Field {
    /// The label shown on the box's frame.
    pub(crate) fn title(self) -> &'static str {
        match self {
            Field::Hex => "HEX",
            Field::Rgb => "RGB",
            Field::Cmyk => "CMYK",
            Field::Hsv => "HSV",
            Field::Hsl => "HSL",
        }
    }

    /// Formats `hsv` in this box's own syntax.
    pub(crate) fn format(self, hsv: Hsv) -> String {
        match self {
            Field::Hex => format_hex(hsv.to_color()),
            Field::Rgb => format_rgb(hsv.to_color()),
            Field::Cmyk => format_cmyk(hsv),
            Field::Hsv => format_hsv(hsv),
            Field::Hsl => format_hsl(hsv),
        }
    }

    /// Parses this box's syntax; `None` leaves the colour unchanged.
    pub(crate) fn parse(self, text: &str) -> Option<Hsv> {
        match self {
            Field::Hex => parse_hex(text).map(Hsv::from_color),
            Field::Rgb => parse_rgb(text).map(Hsv::from_color),
            Field::Cmyk => parse_cmyk(text),
            Field::Hsv => parse_hsv(text),
            Field::Hsl => parse_hsl(text),
        }
    }
}

/// Handles one event on an editable box, returning a mapped message.
pub(crate) fn on_event<M: 'static>(
    ui: &Ui<M>,
    shared: &std::rc::Rc<Shared<M>>,
    field: Field,
    event: &Event,
) -> Option<M> {
    match event {
        Event::KeyDown {
            key: Key::RETURN, ..
        }
        | Event::KillFocus => commit(ui, shared, field),
        Event::Char(_) | Event::TextChanged | Event::KeyDown { .. } => {
            live(ui, shared, field);
            None
        }
        _ => None,
    }
}

/// Applies a live, still-valid edit from `field`, updating the other views.
fn live<M: 'static>(ui: &Ui<M>, shared: &Shared<M>, field: Field) {
    let Some(edit) = shared.edit(field) else {
        return;
    };
    let text = edit.text();
    // A native field reports the text this panel itself just wrote (from
    // `sync`); ignore it rather than re-deriving a rounded colour from it.
    if text == field.format(shared.hsv.get()) {
        return;
    }
    let Some(hsv) = field.parse(&text) else {
        return;
    };
    if hsv == shared.hsv.get() {
        return;
    }
    shared.hsv.set(hsv);
    super::sync(ui, shared, Some(field));
    if let Some(message) = shared.change(hsv.to_color()) {
        ui.emit(message);
    }
}

/// Commits `field` on Enter or focus loss: apply valid text, else revert it.
fn commit<M: 'static>(ui: &Ui<M>, shared: &Shared<M>, field: Field) -> Option<M> {
    let edit = shared.edit(field)?;
    let Some(hsv) = field.parse(&edit.text()) else {
        // Invalid: put the box back to the model, leaving the colour untouched.
        edit.set_text(&field.format(shared.hsv.get()));
        return None;
    };
    shared.hsv.set(hsv);
    super::sync(ui, shared, None);
    shared.commit(hsv.to_color())
}
