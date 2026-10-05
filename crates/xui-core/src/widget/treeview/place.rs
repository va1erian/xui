#![forbid(unsafe_code)]

//! [`Placeable`] for [`TreeView`].

use super::{TreeView, bar};
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::{Rect, Size};
use crate::layout::Constraints;
use crate::units::Dip;
use crate::widget::Placeable;

/// The natural size of a tree: room for a few levels and a dozen rows. A tree
/// usually fills its pane; this is what it asks for when it does not.
const NATURAL_WIDTH: Dip = Dip(240.0);
const NATURAL_HEIGHT: Dip = Dip(240.0);

impl<M: 'static> Placeable<M> for TreeView<M> {
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

    fn placed(&self, ui: &Ui<M>, _rect: Rect) {
        // The scrollbar is a child node: re-lay it against the new bounds.
        bar::layout(ui, self.control.id(), &self.bar, &self.state.borrow());
    }
}
