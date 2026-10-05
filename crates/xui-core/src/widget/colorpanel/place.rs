#![forbid(unsafe_code)]

//! [`Placeable`] for [`ColorPanel`].

use super::ColorPanel;
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
