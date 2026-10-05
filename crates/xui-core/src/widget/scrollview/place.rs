#![forbid(unsafe_code)]

//! [`Placeable`] for [`ScrollView`].

use super::ScrollView;
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::{Rect, Size};
use crate::layout::Constraints;
use crate::units::Dip;
use crate::widget::Placeable;

/// The natural width of a scroll view; its rows stretch to whatever it gets.
const NATURAL_WIDTH: Dip = Dip(240.0);

impl<M: 'static> Placeable<M> for ScrollView<M> {
    fn id(&self) -> WidgetId {
        ScrollView::id(self)
    }

    /// Its rows' total height: give it `fill` (or a fixed height) to take
    /// less room than its content and scroll the rest.
    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        let height = self
            .shared
            .rows
            .borrow()
            .iter()
            .map(|row| row.height.to_px(dpi).value().max(0))
            .sum();
        Size::new(NATURAL_WIDTH.to_px(dpi).value(), height)
    }

    fn placed(&self, _ui: &Ui<M>, _rect: Rect) {
        self.relayout();
    }
}
