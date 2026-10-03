#![forbid(unsafe_code)]

//! [`ProgressBar`]: a themed, read-only range indicator.

use std::cell::Cell;
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result};
use crate::geometry::Rect;
use crate::property::{Properties, Property, Value};
use crate::theme::look::{self, backdrop};

/// The corner radius of the bar.
const RADIUS: f32 = 6.0;

/// A read-only progress indicator.
pub struct ProgressBar<M: 'static> {
    control: Control<M>,
    value: Rc<Cell<i32>>,
    max: Rc<Cell<i32>>,
    enabled: Rc<Cell<bool>>,
}

impl<M: 'static> ProgressBar<M> {
    /// Creates a progress bar with no bounds of its own, for a layout to place (see
    /// [`crate::arrange`]); its size comes from [`Placeable`](super::Placeable).
    pub fn auto(ui: &Ui<M>, max: i32) -> Result<ProgressBar<M>> {
        ProgressBar::new(ui, Rect::default(), max)
    }

    /// Creates an empty bar with the range `0..=max` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, max: i32) -> Result<ProgressBar<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::ProgressBar, bounds))?;
        let value = Rc::new(Cell::new(0));
        let max = Rc::new(Cell::new(max.max(1)));

        {
            let value = Rc::clone(&value);
            let max = Rc::clone(&max);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                backdrop(canvas, theme.background);
                // A decorated theme's groove must read on its gradient
                // background, which the scroll bar's blending track does not.
                let groove = if look::decorated(&theme) {
                    theme.track
                } else {
                    theme.scrollbar_track
                };
                canvas.fill_rounded_rect(bounds, RADIUS, groove);

                let max = max.get().max(1);
                let filled =
                    (bounds.width() as i64 * value.get().clamp(0, max) as i64 / max as i64) as i32;
                if filled > 0 {
                    let fill =
                        Rect::new(bounds.left, bounds.top, bounds.left + filled, bounds.bottom);
                    look::face(canvas, fill, RADIUS, theme.accent, &theme);
                }
                if selected.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        Ok(ProgressBar {
            control,
            value,
            max,
            enabled: Rc::new(Cell::new(true)),
        })
    }

    /// The bar's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// The current value, clamped to the range.
    pub fn value(&self) -> i32 {
        self.value.get()
    }

    /// Sets the current value, clamped to the range.
    pub fn set_value(&self, value: i32) {
        self.value.set(value.clamp(0, self.max.get()));
        self.control.invalidate();
    }

    /// The range's upper bound.
    pub fn max(&self) -> i32 {
        self.max.get()
    }

    /// Sets the range's upper bound (at least one) and clamps the value.
    pub fn set_max(&self, max: i32) {
        self.max.set(max.max(1));
        self.set_value(self.value.get());
    }

    /// Whether the bar is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    /// Enables or disables the bar.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.control.set_enabled(enabled);
    }

    /// Marks the bar selected (a form editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for ProgressBar<M> {
    fn properties(&self) -> Vec<Property> {
        vec![
            Property {
                name: "value",
                value: Value::Integer(self.value() as i64),
            },
            Property {
                name: "max",
                value: Value::Integer(self.max() as i64),
            },
            Property {
                name: "enabled",
                value: Value::Bool(self.is_enabled()),
            },
        ]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("value", Value::Integer(value)) => {
                self.set_value(value.clamp(0, self.max() as i64) as i32);
                true
            }
            ("max", Value::Integer(max)) => {
                self.set_max(i32::try_from(max).unwrap_or(i32::MAX));
                true
            }
            ("enabled", Value::Bool(enabled)) => {
                self.set_enabled(enabled);
                true
            }
            _ => false,
        }
    }
}
