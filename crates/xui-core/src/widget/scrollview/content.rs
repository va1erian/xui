#![forbid(unsafe_code)]

//! A scroll view whose content is a layout: measured at the viewport's width
//! with no bound on its height, and placed at the scroll offset.

use super::{ScrollView, Shared, scrollbar};
use crate::app::Ui;
use crate::arrange::Layout;
use crate::backend::{Result, WidgetId};
use crate::geometry::{Rect, Size};
use crate::layout::Constraints;
use crate::units::Dip;
use crate::widget::Placeable;

/// The natural width of a scroll view of rows; the rows stretch to whatever
/// it gets.
const NATURAL_WIDTH: Dip = Dip(240.0);

impl<M: 'static> ScrollView<M> {
    /// Replaces the view's content with `layout`: its widgets are created
    /// inside the view, measured at the view's width and scrolled as one.
    /// The rows added with [`ScrollView::add`] are not laid out while a layout
    /// is set.
    pub fn set_layout(&self, layout: Layout<M>) -> Result<()> {
        let mounted = self.control.ui().mount_driven(self.id(), layout)?;
        self.shared.layout.replace(Some(mounted));
        super::relayout(&self.scoped, &self.shared);
        self.scoped.raise(self.shared.bar.id());
        Ok(())
    }
}

/// Lays a layout content out inside `bounds` (the view's own extent) at the
/// current offset. Returns false when the view has no layout content.
pub(super) fn relayout<M>(ui: &Ui<M>, s: &Shared<M>, bounds: Rect) -> bool {
    let content = s.layout.borrow();
    let Some(content) = content.as_ref() else {
        return false;
    };
    let dpi = ui.dpi();
    let height_at = |width: i32| {
        content
            .measure(Constraints::unbounded(dpi).with_width(width))
            .height
    };
    let mut height = height_at(bounds.width());
    let overflows = height > bounds.height();
    let bar_width = if overflows {
        scrollbar::THICKNESS.to_px(dpi).value()
    } else {
        0
    };
    let viewport = Rect::new(
        bounds.left,
        bounds.top,
        (bounds.right - bar_width).max(bounds.left),
        bounds.bottom,
    );
    if overflows {
        height = height_at(viewport.width());
    }
    let offset = s.offset.get().clamp(0, (height - viewport.height()).max(0));
    s.content.set(height);
    s.viewport.set(viewport);
    s.offset.set(offset);
    let bar = Rect::new(viewport.right, bounds.top, bounds.right, bounds.bottom);
    s.bar.set_track(bar.width(), bar.height());
    ui.set_visible(s.bar.id(), overflows);
    if overflows {
        ui.apply_moves(&[(s.bar.id(), bar)]);
    }
    content.place(Rect::new(
        viewport.left,
        viewport.top - offset,
        viewport.right,
        viewport.top - offset + height.max(viewport.height()),
    ));
    ui.invalidate(s.bar.id());
    true
}

impl<M: 'static> Placeable<M> for ScrollView<M> {
    fn id(&self) -> WidgetId {
        ScrollView::id(self)
    }

    /// A layout content's natural size, or the rows' total height; give the
    /// view `fill` or a fixed height for it to scroll.
    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        match self.shared.layout.borrow().as_ref() {
            Some(content) => content.measure(Constraints {
                max_height: None,
                ..constraints
            }),
            None => {
                let height = self
                    .shared
                    .rows
                    .borrow()
                    .iter()
                    .map(|row| row.height.to_px(dpi).value().max(0))
                    .sum();
                Size::new(NATURAL_WIDTH.to_px(dpi).value(), height)
            }
        }
    }

    fn placed(&self, _ui: &Ui<M>, _rect: Rect) {
        self.relayout();
    }
}
