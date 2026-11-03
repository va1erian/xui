#![forbid(unsafe_code)]

//! [`HtmlView`]'s place in an `xui_core::arrange` layout.

use xui_core::app::Ui;
use xui_core::backend::WidgetId;
use xui_core::geometry::Size;
use xui_core::layout::Constraints;
use xui_core::widget::Placeable;

use crate::HtmlView;

/// The view fills whatever slot a layout gives it and re-renders at that
/// width; it has no natural size of its own.
impl<M: Send + 'static> Placeable<M> for HtmlView<M> {
    fn id(&self) -> WidgetId {
        HtmlView::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, _constraints: Constraints) -> Size {
        Size::new(0, 0)
    }
}
