#![forbid(unsafe_code)]

//! [`Placeable`] for [`ColorPanel`] and its parts, [`ColorField`] and
//! [`HueSlider`].

use super::{ColorField, ColorPanel, HueSlider};
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::{Rect, Size};
use crate::layout::Constraints;
use crate::units::Dip;
use crate::widget::Placeable;

/// The natural size of the panel: the Full tab's field, hue strip, preview
/// and value boxes side by side at a comfortable size.
const NATURAL_WIDTH: Dip = Dip(480.0);
const NATURAL_HEIGHT: Dip = Dip(320.0);

impl<M: 'static> Placeable<M> for ColorPanel<M> {
    fn id(&self) -> WidgetId {
        ColorPanel::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        Size::new(
            NATURAL_WIDTH.to_px(dpi).value(),
            NATURAL_HEIGHT.to_px(dpi).value(),
        )
    }

    fn placed(&self, _ui: &Ui<M>, rect: Rect) {
        self.arrange(rect);
    }
}

/// The natural size of a saturation/value field.
const FIELD: Dip = Dip(200.0);
/// The natural length and thickness of a hue strip.
const HUE_LENGTH: Dip = Dip(200.0);
const HUE_THICKNESS: Dip = Dip(20.0);

impl<M: 'static> Placeable<M> for ColorField<M> {
    fn id(&self) -> WidgetId {
        ColorField::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let side = FIELD.to_px(constraints.dpi).value();
        Size::new(side, side)
    }
}

impl<M: 'static> Placeable<M> for HueSlider<M> {
    fn id(&self) -> WidgetId {
        HueSlider::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        Size::new(
            HUE_LENGTH.to_px(dpi).value(),
            HUE_THICKNESS.to_px(dpi).value(),
        )
    }
}
