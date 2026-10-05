#![forbid(unsafe_code)]

//! [`Placeable`] for [`FlowText`]: its runs wrapped at the width the layout
//! offers, so a column gives it the height its text needs at that width.

use super::FlowText;
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::Size;
use crate::layout::Constraints;
use crate::widget::Placeable;

impl<M: 'static> Placeable<M> for FlowText<M> {
    fn id(&self) -> WidgetId {
        FlowText::id(self)
    }

    /// One line with no width bound; wrapped lines within one.
    fn measure(&self, ui: &Ui<M>, constraints: Constraints) -> Size {
        let width = constraints.max_width.unwrap_or(i32::MAX / 2);
        let block = self.wrapped(ui, constraints.dpi, width);
        Size::new(block.width, block.height.max(1))
    }
}
