#![forbid(unsafe_code)]

//! The [`Editor`]'s place in an `xui_core::arrange` layout.

use xui_core::app::Ui;
use xui_core::backend::WidgetId;
use xui_core::geometry::Size;
use xui_core::layout::Constraints;
use xui_core::widget::Placeable;

use crate::Editor;

/// The editor fills whatever slot a layout gives it and scrolls its text
/// inside it, so it has no natural size of its own.
impl<M: 'static> Placeable<M> for Editor<M> {
    fn id(&self) -> WidgetId {
        Editor::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, _constraints: Constraints) -> Size {
        Size::new(0, 0)
    }
}
