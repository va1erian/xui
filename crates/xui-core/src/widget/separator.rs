#![forbid(unsafe_code)]

//! [`Separator`]: a themed horizontal or vertical divider line.

use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::{Point, Rect};
use crate::property::{Properties, Property, Value};

/// The thickness of the divider line, in device pixels.
const LINE_WIDTH: f32 = 1.0;
/// The width of the designer's selection outline, in device pixels.
const SELECTED_WIDTH: f32 = 2.0;

/// The direction a [`Separator`] runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    /// A line across the width, at the vertical middle (the default).
    #[default]
    Horizontal,
    /// A line down the height, at the horizontal middle.
    Vertical,
}

/// A themed divider line.
pub struct Separator<M: 'static> {
    control: Control<M>,
    orientation: Orientation,
}

impl<M: 'static> Separator<M> {
    /// Creates a horizontal divider with no bounds of its own, for a layout to
    /// place (see [`crate::arrange`]).
    pub fn auto(ui: &Ui<M>) -> Result<Separator<M>> {
        Separator::new(ui, Rect::default())
    }

    /// Creates a vertical divider with no bounds of its own, for a layout to
    /// place (see [`crate::arrange`]).
    pub fn auto_vertical(ui: &Ui<M>) -> Result<Separator<M>> {
        Separator::vertical(ui, Rect::default())
    }

    /// Creates a horizontal divider at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect) -> Result<Separator<M>> {
        Separator::create(ui, bounds, Orientation::Horizontal)
    }

    /// Creates a vertical divider at `bounds`.
    pub fn vertical(ui: &Ui<M>, bounds: Rect) -> Result<Separator<M>> {
        Separator::create(ui, bounds, Orientation::Vertical)
    }

    fn create(ui: &Ui<M>, bounds: Rect, orientation: Orientation) -> Result<Separator<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Separator, bounds))?;
        let theme = ui.theme_handle();
        let selected = control.selected_handle();
        control.set_painter(Rc::new(move |canvas| {
            let theme = theme.get();
            let bounds = canvas.bounds();
            canvas.clear(theme.background);
            let (from, to) = match orientation {
                Orientation::Horizontal => {
                    let y = bounds.top + bounds.height() / 2;
                    (Point::new(bounds.left, y), Point::new(bounds.right, y))
                }
                Orientation::Vertical => {
                    let x = bounds.left + bounds.width() / 2;
                    (Point::new(x, bounds.top), Point::new(x, bounds.bottom))
                }
            };
            canvas.draw_line(from, to, theme.border, LINE_WIDTH);
            if selected.get() {
                canvas.stroke_rect(bounds, theme.accent, SELECTED_WIDTH);
            }
        }));
        Ok(Separator {
            control,
            orientation,
        })
    }

    /// The separator's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// The direction the line runs.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Marks the separator selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for Separator<M> {
    fn properties(&self) -> Vec<Property> {
        Vec::new()
    }

    fn set_property(&self, _name: &str, _value: Value) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::{Orientation, Separator};
    use crate::app::{Core, Ui};
    use crate::backend::headless::{DrawOp, HeadlessBackend};
    use crate::backend::{Backend, PlatformSpec};
    use crate::geometry::Rect;

    fn setup() -> (Rc<HeadlessBackend>, Rc<Core<u32>>, Ui<u32>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend
            .open_window(&PlatformSpec::new("separator"))
            .unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(Rc::clone(&core));
        (backend, core, ui)
    }

    #[test]
    fn a_horizontal_separator_paints_a_line() {
        let (backend, _core, ui) = setup();
        let separator = Separator::new(&ui, Rect::new(0, 0, 120, 12)).unwrap();
        assert_eq!(separator.orientation(), Orientation::Horizontal);

        backend.render(separator.id());
        let ops = backend.ops(separator.id());
        assert!(
            ops.iter().any(|op| matches!(op, DrawOp::Line(..))),
            "the separator painted a line: {ops:?}"
        );
    }
}
