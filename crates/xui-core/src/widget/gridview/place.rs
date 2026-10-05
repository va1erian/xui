#![forbid(unsafe_code)]

//! [`Placeable`] for [`GridView`].

use super::GridView;
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::Size;
use crate::layout::Constraints;
use crate::units::Dip;
use crate::widget::Placeable;

/// The natural size of a tile grid: a few tiles across and down. A grid
/// usually fills its area; this is what it asks for when it does not.
const NATURAL_WIDTH: Dip = Dip(360.0);
const NATURAL_HEIGHT: Dip = Dip(240.0);

impl<M: 'static> Placeable<M> for GridView<M> {
    fn id(&self) -> WidgetId {
        self.control.id()
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        Size::new(
            NATURAL_WIDTH.to_px(dpi).value(),
            NATURAL_HEIGHT.to_px(dpi).value(),
        )
    }
}
