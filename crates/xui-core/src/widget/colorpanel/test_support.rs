#![forbid(unsafe_code)]

//! Test-only accessors into a [`ColorPanel`](super::ColorPanel)'s children, so
//! the headless event tests can target a specific node.

use std::rc::Rc;

use super::ColorPanel;
use super::model::Hsv;
use super::text::Field;
use crate::backend::WidgetId;
use crate::widget::Edit;

impl<M: 'static> ColorPanel<M> {
    /// The Simple tab's swatch grid node.
    pub(crate) fn swatch_node(&self) -> WidgetId {
        self.swatches.id()
    }

    /// The Full tab's SV field node.
    pub(crate) fn field_node(&self) -> WidgetId {
        self.field.id()
    }

    /// An editable box's widget.
    pub(crate) fn edit_handle(&self, field: Field) -> Option<Rc<Edit<M>>> {
        self.shared.edit(field)
    }

    /// The SV field's current triple.
    pub(crate) fn field_hsv(&self) -> Hsv {
        self.field.hsv()
    }

    /// The hue slider's current value.
    pub(crate) fn hue_value(&self) -> f32 {
        self.hue.hue()
    }
}
