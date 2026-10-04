#![forbid(unsafe_code)]

//! Pages built from a layout, and [`Placeable`] for [`Tabs`].

use super::{STRIP, Tabs, relayout};
use crate::app::Ui;
use crate::arrange::Layout;
use crate::backend::{Result, WidgetId};
use crate::geometry::{Rect, Size};
use crate::layout::Constraints;
use crate::widget::{Panel, Placeable};

impl<M: 'static> Tabs<M> {
    /// Appends `title`'s page holding `content`: its widgets are created in a
    /// panel that fills the page and kept placed as the container resizes.
    pub fn add_layout_page(&self, title: &str, content: Layout<M>) -> Result<()> {
        let panel = Panel::plain(&self.scoped, Rect::default())?;
        let mounted = self.scoped.mount_in(panel.id(), content)?;
        self.add_page(title, &[panel.id()]);
        self.layouts.borrow_mut().push((panel, mounted));
        Ok(())
    }
}

impl<M: 'static> Placeable<M> for Tabs<M> {
    fn id(&self) -> WidgetId {
        Tabs::id(self)
    }

    /// The strip above the largest page a layout fills.
    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let page = self
            .layouts
            .borrow()
            .iter()
            .map(|(_, mounted)| mounted.preferred_size())
            .fold(Size::new(0, 0), |most, size| {
                Size::new(most.width.max(size.width), most.height.max(size.height))
            });
        Size::new(
            page.width,
            page.height + STRIP.to_px(constraints.dpi).value(),
        )
    }

    fn placed(&self, _ui: &Ui<M>, _rect: Rect) {
        relayout(&self.scoped, &self.shared);
    }
}
