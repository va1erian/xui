#![forbid(unsafe_code)]

//! [`BlitzView`]'s place in an `xui_core::arrange` layout.

use xui_core::app::Ui;
use xui_core::backend::WidgetId;
use xui_core::geometry::{Rect, Size};
use xui_core::layout::Constraints;
use xui_core::widget::Placeable;

use crate::BlitzView;

/// The view fills whatever slot a layout gives it, re-rendering at that size,
/// and its scrollbar follows the slot's right edge; it has no natural size of
/// its own.
impl<M: Send + 'static> Placeable<M> for BlitzView<M> {
    fn id(&self) -> WidgetId {
        BlitzView::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, _constraints: Constraints) -> Size {
        Size::new(0, 0)
    }

    fn placed(&self, _ui: &Ui<M>, rect: Rect) {
        // A layout moves only the view's own node; the scrollbar is a child
        // node, so re-lay it against the view's new bounds.
        self.place_bar(rect);
    }
}
