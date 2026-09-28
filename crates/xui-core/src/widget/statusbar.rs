#![forbid(unsafe_code)]

//! [`StatusBar`]: a horizontal row of non-interactive text parts.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use super::ellipsis;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::geometry::{Point, Rect};
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

/// The design size of a part's text.
const TEXT_SIZE: Dip = Dip(12.0);
/// The horizontal inset of a part's text from its cell edge.
const PADDING: Dip = Dip(6.0);
/// The thickness of the top border and the part dividers.
const BORDER_WIDTH: f32 = 1.0;
/// The width of the designer's selection outline.
const SELECTED_WIDTH: f32 = 2.0;

/// A horizontal bar of text parts. The parts share the width equally.
pub struct StatusBar<M: 'static> {
    control: Control<M>,
    parts: Rc<RefCell<Vec<String>>>,
    enabled: Rc<Cell<bool>>,
}

impl<M: 'static> StatusBar<M> {
    /// Creates a bar with no bounds of its own, for a layout to place (see
    /// [`crate::arrange`]); its size comes from
    /// [`Placeable`](super::Placeable).
    pub fn auto(ui: &Ui<M>, parts: &[&str]) -> Result<StatusBar<M>> {
        StatusBar::new(ui, Rect::default(), parts)
    }

    /// Creates a bar with `parts`, laid out left-to-right across `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, parts: &[&str]) -> Result<StatusBar<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::StatusBar, bounds))?;
        let state: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(
            parts.iter().map(|part| part.to_string()).collect(),
        ));
        let enabled = Rc::new(Cell::new(true));
        let theme = ui.theme_handle();
        let selected = control.selected_handle();
        let parts_for_paint = Rc::clone(&state);
        let enabled_for_paint = Rc::clone(&enabled);
        control.set_painter(Rc::new(move |canvas| {
            let theme = theme.get();
            let bounds = canvas.bounds();
            let dpi = canvas.dpi();
            let enabled = enabled_for_paint.get();
            canvas.clear(theme.background);
            canvas.draw_line(
                Point::new(bounds.left, bounds.top),
                Point::new(bounds.right, bounds.top),
                theme.border,
                BORDER_WIDTH,
            );

            let parts = parts_for_paint.borrow();
            let count = parts.len() as i32;
            if count > 0 {
                let padding = PADDING.to_px(dpi).value();
                let width = bounds.width();
                for (index, part) in parts.iter().enumerate() {
                    let index = index as i32;
                    let left = bounds.left + width * index / count;
                    let right = bounds.left + width * (index + 1) / count;
                    if index > 0 {
                        canvas.draw_line(
                            Point::new(left, bounds.top),
                            Point::new(left, bounds.bottom),
                            theme.border,
                            BORDER_WIDTH,
                        );
                    }
                    let color = if !enabled {
                        theme.text_disabled
                    } else if index == 0 {
                        theme.text
                    } else {
                        theme.text_secondary
                    };
                    let cell = Rect::new(left, bounds.top, right, bounds.bottom);
                    let text_rect =
                        Rect::new(left + padding, bounds.top, right - padding, bounds.bottom);
                    let style = TextStyle::new(color, TEXT_SIZE).middle();
                    // A part wider than its share of the bar must not run
                    // into the next part: ellipsize it, and clip the cell
                    // too in case the measurer and rasterizer disagree.
                    canvas.push_clip(cell);
                    let fitted = ellipsis::truncate(part, text_rect.width(), &mut |candidate| {
                        canvas.measure_text(candidate, &style).width
                    });
                    canvas.draw_text(&fitted, text_rect, &style);
                    canvas.pop_clip();
                }
            }

            if selected.get() {
                canvas.stroke_rect(bounds, theme.accent, SELECTED_WIDTH);
            }
        }));

        Ok(StatusBar {
            control,
            parts: state,
            enabled,
        })
    }

    /// The bar's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// The number of parts.
    pub fn len(&self) -> usize {
        self.parts.borrow().len()
    }

    /// Whether the bar has no parts.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The text of part `part`, if it exists.
    pub fn text(&self, part: usize) -> Option<String> {
        self.parts.borrow().get(part).cloned()
    }

    /// Replaces part `part`'s text and repaints; an out-of-range part is
    /// ignored.
    pub fn set_text(&self, part: usize, text: &str) {
        if let Some(slot) = self.parts.borrow_mut().get_mut(part) {
            *slot = text.to_string();
            self.control.invalidate();
        }
    }

    /// Replaces every part.
    pub fn set_parts(&self, parts: &[&str]) {
        *self.parts.borrow_mut() = parts.iter().map(|part| part.to_string()).collect();
        self.control.invalidate();
    }

    /// Enables or disables the bar. A disabled bar dims its text.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the bar selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for StatusBar<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "text",
            value: Value::Text(self.text(0).unwrap_or_default()),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("text", Value::Text(text)) => {
                let mut parts = self.parts.borrow_mut();
                if parts.is_empty() {
                    parts.push(text);
                } else {
                    parts[0] = text;
                }
                drop(parts);
                self.control.invalidate();
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::StatusBar;
    use crate::app::{Core, Ui};
    use crate::backend::headless::{DrawOp, HeadlessBackend};
    use crate::backend::{Backend, PlatformSpec};
    use crate::geometry::Rect;

    fn setup() -> (Rc<HeadlessBackend>, Rc<Core<u32>>, Ui<u32>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend
            .open_window(&PlatformSpec::new("statusbar"))
            .unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(Rc::clone(&core));
        (backend, core, ui)
    }

    #[test]
    fn a_status_bar_paints_every_part() {
        let (backend, _core, ui) = setup();
        let bar = StatusBar::new(&ui, Rect::new(0, 0, 240, 24), &["Ready", "3 items"]).unwrap();

        backend.render(bar.id());
        let ops = backend.ops(bar.id());
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Ready")),
            "the first part was painted: {ops:?}"
        );
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "3 items")),
            "the second part was painted: {ops:?}"
        );
    }

    #[test]
    fn a_part_too_narrow_for_its_text_is_ellipsized_not_overflowed() {
        let (backend, _core, ui) = setup();
        let long = "C:\\Users\\hadri\\AppData\\Local\\Microsoft\\WinGet\\Packages";
        let bar = StatusBar::new(&ui, Rect::new(0, 0, 200, 24), &[long, "30 targets"]).unwrap();

        backend.render(bar.id());
        let ops = backend.ops(bar.id());
        let drawn: Vec<&String> = ops
            .iter()
            .filter_map(|op| match op {
                DrawOp::Text(_, text, _) => Some(text),
                _ => None,
            })
            .collect();
        assert!(
            drawn.iter().any(|text| text.ends_with('\u{2026}')),
            "expected the long part to be ellipsized among {drawn:?}"
        );
        assert!(
            !drawn.iter().any(|text| text.as_str() == long),
            "the overflowing text must not be drawn in full: {drawn:?}"
        );

        // The bar's two parts split its 200px width evenly at x=100; the
        // first part's (ellipsized) text must not cross into the second.
        let first_right = ops
            .iter()
            .find_map(|op| match op {
                DrawOp::Text(rect, text, _) if text.ends_with('\u{2026}') => Some(rect.right),
                _ => None,
            })
            .expect("the first part's ellipsized text is drawn");
        assert!(
            first_right <= 100,
            "the first part's text must stay within its half of the bar: {first_right}"
        );
    }

    #[test]
    fn set_text_replaces_a_part_and_repaints() {
        let (backend, _core, ui) = setup();
        let bar = StatusBar::new(&ui, Rect::new(0, 0, 240, 24), &["Ready", "3 items"]).unwrap();

        bar.set_text(0, "Done");
        assert_eq!(bar.text(0), Some("Done".to_string()));

        backend.render(bar.id());
        let ops = backend.ops(bar.id());
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Done")),
            "the new text was painted: {ops:?}"
        );
    }
}
