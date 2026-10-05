#![forbid(unsafe_code)]

//! [`Placeable`] for [`TopBar`]: its items at their natural widths, one item
//! tall. Spacers and expanding items take whatever the layout gives beyond.

use super::TopBar;
use super::items::{ITEM, bar_width};
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::Size;
use crate::layout::Constraints;
use crate::widget::Placeable;

impl<M: 'static> Placeable<M> for TopBar<M> {
    fn id(&self) -> WidgetId {
        self.control.id()
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        Size::new(
            bar_width(&self.items.borrow(), dpi),
            ITEM.to_px(dpi).value(),
        )
    }
}
