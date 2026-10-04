#![forbid(unsafe_code)]

//! [`Placeable`]: the capability a layout needs from a widget.
//!
//! A layout places a widget by its node and asks how big it would like to be.
//! The natural sizes here follow the design values the painted widgets already
//! use (a 28-dip control row, 12-dip text), measured through the backend so
//! they track the font and the DPI.

use super::separator::Orientation;
use super::{
    Button, CheckBox, ComboBox, Edit, HasText, Hyperlink, Label, MultilineEdit, NumberField,
    ProgressBar, Separator, Slider, StatusBar, ToggleButton,
};
use crate::app::Ui;
use crate::backend::{TextStyle, WidgetId};
use crate::geometry::{Rect, Size};
use crate::layout::{Constraints, Insets};
use crate::units::Dip;

/// The design size of widget text.
const TEXT_SIZE: Dip = Dip(12.0);
/// The height of a single-line control.
const CONTROL_HEIGHT: Dip = Dip(28.0);
/// The width of a field with no content to size it (an edit, a slider).
const FIELD_WIDTH: Dip = Dip(160.0);
/// The horizontal padding either side of a button's label.
const BUTTON_PADDING: Dip = Dip(12.0);
/// The narrowest a button gets, however short its label.
const BUTTON_MIN_WIDTH: Dip = Dip(64.0);
/// A check box's square plus the gap before its label.
const CHECK_LEAD: Dip = Dip(24.0);
/// The height of a progress bar.
const PROGRESS_HEIGHT: Dip = Dip(8.0);
/// The vertical padding around a bare text run.
const TEXT_PADDING: Dip = Dip(4.0);
/// The natural size of a status bar with nothing to size it.
const STATUS_WIDTH: Dip = Dip(200.0);
/// The natural height of a status bar.
const STATUS_HEIGHT: Dip = Dip(24.0);

/// A widget a layout can place.
///
/// A capability trait, not a base type: it needs only the widget's node and a
/// measured size. The default size is the widget's current bounds, so a widget
/// that has not opted in keeps whatever size it was built at.
pub trait Placeable<M: 'static> {
    /// The widget's node.
    fn id(&self) -> WidgetId;

    /// The size the widget would like within `constraints`, in device pixels.
    /// A widget whose height depends on its width (wrapping text) reads
    /// [`Constraints::max_width`]; the result may exceed a bound when the
    /// widget cannot shrink to it.
    fn measure(&self, ui: &Ui<M>, constraints: Constraints) -> Size {
        let _ = constraints;
        ui.bounds(self.id()).size()
    }

    /// For a widget that frames other widgets (a group box), the insets
    /// between its edge and the content a layout places inside it.
    fn content_insets(&self, ui: &Ui<M>) -> Insets {
        let _ = ui;
        Insets::default()
    }

    /// Reacts to the layout having placed the widget's node at `rect` (device
    /// pixels). A widget that owns satellite nodes positions them here so they
    /// follow their primary — a [`ListView`](super::ListView) re-lays its
    /// scrollbar; the default does nothing.
    fn placed(&self, ui: &Ui<M>, rect: Rect) {
        let _ = (ui, rect);
    }
}

/// The measured size of `text` in the standard widget font.
fn text_size<M: 'static>(ui: &Ui<M>, text: &str, dpi: u32) -> Size {
    let style = TextStyle::new(ui.theme().text, TEXT_SIZE);
    let metrics = ui.measure_text(text, &style, dpi);
    Size::new(metrics.width, metrics.height)
}

fn px(value: Dip, dpi: u32) -> i32 {
    value.to_px(dpi).value()
}

/// A single-line control `width` wide.
fn row(width: i32, dpi: u32) -> Size {
    Size::new(width, px(CONTROL_HEIGHT, dpi))
}

impl<M: 'static> Placeable<M> for Label<M> {
    fn id(&self) -> WidgetId {
        Label::id(self)
    }

    fn measure(&self, ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        let text = text_size(ui, &self.text(), dpi);
        Size::new(text.width, text.height + 2 * px(TEXT_PADDING, dpi))
    }
}

impl<M: 'static> Placeable<M> for Hyperlink<M> {
    fn id(&self) -> WidgetId {
        Hyperlink::id(self)
    }

    fn measure(&self, ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        let text = text_size(ui, &self.text(), dpi);
        Size::new(text.width, text.height + 2 * px(TEXT_PADDING, dpi))
    }
}

/// Implements [`Placeable`] for a widget sized by its label, as a push button.
macro_rules! labelled_button {
    ($($widget:ident),*) => {$(
        impl<M: 'static> Placeable<M> for $widget<M> {
            fn id(&self) -> WidgetId {
                $widget::id(self)
            }

            fn measure(&self, ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
                let width = text_size(ui, &self.text(), dpi).width + 2 * px(BUTTON_PADDING, dpi);
                row(width.max(px(BUTTON_MIN_WIDTH, dpi)), dpi)
            }
        }
    )*};
}
labelled_button!(Button, ToggleButton);

impl<M: 'static> Placeable<M> for CheckBox<M> {
    fn id(&self) -> WidgetId {
        CheckBox::id(self)
    }

    fn measure(&self, ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        row(
            text_size(ui, &self.text(), dpi).width + px(CHECK_LEAD, dpi),
            dpi,
        )
    }
}

/// Implements [`Placeable`] for a field with no content to size it.
macro_rules! field {
    ($($widget:ident),*) => {$(
        impl<M: 'static> Placeable<M> for $widget<M> {
            fn id(&self) -> WidgetId {
                $widget::id(self)
            }

            fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
                row(px(FIELD_WIDTH, dpi), dpi)
            }
        }
    )*};
}
field!(Edit, NumberField, ComboBox, Slider);

impl<M: 'static> Placeable<M> for MultilineEdit<M> {
    fn id(&self) -> WidgetId {
        MultilineEdit::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        Size::new(px(FIELD_WIDTH, dpi), 3 * px(CONTROL_HEIGHT, dpi))
    }
}

impl<M: 'static> Placeable<M> for ProgressBar<M> {
    fn id(&self) -> WidgetId {
        ProgressBar::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        Size::new(px(FIELD_WIDTH, dpi), px(PROGRESS_HEIGHT, dpi))
    }
}

impl<M: 'static> Placeable<M> for Separator<M> {
    fn id(&self) -> WidgetId {
        Separator::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        let line = px(Dip(1.0), dpi).max(1);
        match self.orientation() {
            Orientation::Horizontal => Size::new(0, line),
            Orientation::Vertical => Size::new(line, 0),
        }
    }
}

impl<M: 'static> Placeable<M> for StatusBar<M> {
    fn id(&self) -> WidgetId {
        StatusBar::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        Size::new(px(STATUS_WIDTH, dpi), px(STATUS_HEIGHT, dpi))
    }
}
