#![forbid(unsafe_code)]

//! [`Placeable`] for a menu bar: its titles side by side, one row tall. A
//! context menu has no node and takes no space.

use super::Menu;
use super::layout::each_title;
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::{Point, Size};
use crate::layout::Constraints;
use crate::units::Dip;
use crate::widget::Placeable;

/// The height of a menu bar.
const HEIGHT: Dip = Dip(28.0);

impl<M: 'static> Placeable<M> for Menu<M> {
    fn id(&self) -> WidgetId {
        Menu::id(self).unwrap_or(WidgetId::NONE)
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        if self.bar.is_none() {
            return Size::new(0, 0);
        }
        let dpi = constraints.dpi;
        let height = HEIGHT.to_px(dpi).value();
        let mut width = 0;
        each_title(
            &self.rt.model.borrow(),
            Point::new(0, 0),
            height,
            dpi,
            |_, rect| width = rect.right,
        );
        Size::new(width, height)
    }
}
