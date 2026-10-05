#![forbid(unsafe_code)]

//! Panes built from layouts, and [`Placeable`] for [`Split`].

use super::{DIVIDER, Split, relayout};
use crate::app::Ui;
use crate::arrange::Layout;
use crate::backend::{Result, WidgetId};
use crate::geometry::{Rect, Size};
use crate::layout::Constraints;
use crate::widget::{Panel, Placeable};

impl<M: 'static> Split<M> {
    /// Fills the two panes with layouts: each pane becomes a panel holding
    /// its layout, re-laid as the divider moves or the split resizes.
    pub fn set_layouts(&self, a: Layout<M>, b: Layout<M>) -> Result<()> {
        let mut layouts = Vec::with_capacity(2);
        for content in [a, b] {
            let panel = Panel::plain(&self.scoped, Rect::default())?;
            let mounted = self.scoped.mount_in(panel.id(), content)?;
            layouts.push((panel, mounted));
        }
        self.pane_a(&[layouts[0].0.id()]);
        self.pane_b(&[layouts[1].0.id()]);
        self.layouts.replace(layouts);
        Ok(())
    }
}

impl<M: 'static> Placeable<M> for Split<M> {
    fn id(&self) -> WidgetId {
        Split::id(self)
    }

    /// The two panes' layouts side by side (or stacked) with the divider
    /// between them; a split without layouts asks for nothing.
    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let layouts = self.layouts.borrow();
        let open = Constraints::unbounded(constraints.dpi);
        let sizes: Vec<Size> = layouts.iter().map(|(_, m)| m.measure(open)).collect();
        if sizes.is_empty() {
            return Size::new(0, 0);
        }
        let divider = DIVIDER.to_px(constraints.dpi).value();
        let (along, across) = sizes.iter().fold((divider, 0), |(along, across), size| {
            if self.shared.horizontal {
                (along + size.width, across.max(size.height))
            } else {
                (along + size.height, across.max(size.width))
            }
        });
        if self.shared.horizontal {
            Size::new(along, across)
        } else {
            Size::new(across, along)
        }
    }

    fn placed(&self, _ui: &Ui<M>, _rect: Rect) {
        relayout(&self.scoped, &self.shared);
        for (_, mounted) in self.layouts.borrow().iter() {
            mounted.relayout();
        }
    }
}
