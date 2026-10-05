#![forbid(unsafe_code)]

//! [`Placeable`] for [`Toolbar`]: as wide as its packed items, one strip tall.

use super::Toolbar;
use super::layout::{self, Mode};
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::Size;
use crate::layout::Constraints;
use crate::units::Dip;
use crate::widget::Placeable;

/// The height of a toolbar strip.
const HEIGHT: Dip = Dip(32.0);

impl<M: 'static> Placeable<M> for Toolbar<M> {
    fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Every item at its content width, as the compact strip packs them
    /// (a strip that splits its width equally asks for the same).
    fn measure(&self, ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        let height = HEIGHT.to_px(dpi).value();
        let style = layout::label_style(ui.theme().text);
        let entries = self.state.entries.borrow();
        let packed = layout::compute(
            &entries,
            Mode::Compact,
            (i32::MAX / 2, height),
            dpi,
            &mut |text| ui.measure_text(text, &style, dpi).width,
        );
        let items = packed.items.iter().map(|&(_, end)| end);
        let width = items
            .chain(packed.separators.iter().copied())
            .max()
            .unwrap_or(0);
        Size::new(width, height)
    }
}
