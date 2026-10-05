#![forbid(unsafe_code)]

//! [`MaterialStatusBar`]: a flat, material-styled status bar of text parts.
//!
//! It is the painted counterpart of the native-parts [`StatusBar`](super::StatusBar):
//! the same part model, restyled with a surface fill, inset dividers and an
//! accent-coloured leading status.

use crate::theme::look;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::Placeable;
use super::control::Control;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::geometry::{Point, Rect, Size};
use crate::layout::Constraints;
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

/// The design size of a part's text.
const TEXT_SIZE: Dip = Dip(12.0);
/// The horizontal inset of a part's text from its cell edge.
const PADDING: Dip = Dip(8.0);
/// The diameter of the leading status dot.
const DOT: Dip = Dip(6.0);
/// The thickness of the top border and the inset dividers.
const BORDER_WIDTH: f32 = 1.0;
/// The width of the designer's selection outline.
const SELECTED_WIDTH: f32 = 2.0;

/// A flat, material-styled row of text parts.
///
/// The first part is the leading status: it gets an accent dot and accent text.
/// Remaining parts are secondary text, separated by hairline dividers that are
/// inset vertically, so the bar reads as chrome rather than a set of boxes.
pub struct MaterialStatusBar<M: 'static> {
    control: Control<M>,
    parts: Rc<RefCell<Vec<String>>>,
    enabled: Rc<Cell<bool>>,
}

impl<M: 'static> MaterialStatusBar<M> {
    /// Creates a bar with `parts`, laid out left-to-right across `bounds`.
    pub(crate) fn new(ui: &Ui<M>, bounds: Rect, parts: &[&str]) -> Result<MaterialStatusBar<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::StatusBar, bounds))?;
        let parts: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(
            parts.iter().map(|part| part.to_string()).collect(),
        ));
        let enabled = Rc::new(Cell::new(true));

        {
            let parts = Rc::clone(&parts);
            let enabled = Rc::clone(&enabled);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                let dpi = canvas.dpi();
                look::band(canvas, &theme);
                canvas.draw_line(
                    Point::new(bounds.left, bounds.top),
                    Point::new(bounds.right, bounds.top),
                    theme.border,
                    BORDER_WIDTH,
                );

                let parts = parts.borrow();
                let count = parts.len() as i32;
                if count > 0 {
                    let padding = PADDING.to_px(dpi).value();
                    let dot = DOT.to_px(dpi).value();
                    let baseline = (bounds.top + bounds.bottom) / 2;
                    let enabled = enabled.get();
                    for (index, part) in parts.iter().enumerate() {
                        let index = index as i32;
                        let left = bounds.left + bounds.width() * index / count;
                        let right = bounds.left + bounds.width() * (index + 1) / count;
                        if index > 0 {
                            let inset = (bounds.height() / 4).max(1);
                            canvas.draw_line(
                                Point::new(left, bounds.top + inset),
                                Point::new(left, bounds.bottom - inset),
                                theme.border,
                                BORDER_WIDTH,
                            );
                        }
                        let leading = index == 0;
                        let mut text_left = left + padding;
                        if leading {
                            canvas.fill_ellipse(
                                Point::new(text_left + dot / 2, baseline),
                                dot as f32 / 2.0,
                                dot as f32 / 2.0,
                                if enabled {
                                    theme.accent
                                } else {
                                    theme.text_disabled
                                },
                            );
                            text_left += dot + padding / 2;
                        }
                        let color = if !enabled {
                            theme.text_disabled
                        } else if leading {
                            theme.accent
                        } else {
                            theme.text_secondary
                        };
                        let style = TextStyle::new(color, TEXT_SIZE).middle();
                        let style = if leading { style.bold() } else { style };
                        let text_rect = Rect::new(text_left, bounds.top, right, bounds.bottom);
                        canvas.draw_text(part, text_rect, &style);
                    }
                }

                if selected.get() {
                    canvas.stroke_rect(bounds, theme.accent, SELECTED_WIDTH);
                }
            }));
        }

        Ok(MaterialStatusBar {
            control,
            parts,
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

    /// Enables or disables the bar. A disabled bar dims its text and dot.
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

/// The natural size of the bar: a status line under a window's content,
/// usually stretched across it.
const NATURAL_WIDTH: Dip = Dip(200.0);
const NATURAL_HEIGHT: Dip = Dip(24.0);

impl<M: 'static> Placeable<M> for MaterialStatusBar<M> {
    fn id(&self) -> WidgetId {
        MaterialStatusBar::id(self)
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        Size::new(
            NATURAL_WIDTH.to_px(dpi).value(),
            NATURAL_HEIGHT.to_px(dpi).value(),
        )
    }
}

impl<M: 'static> Properties for MaterialStatusBar<M> {
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

    use super::MaterialStatusBar;
    use crate::app::{Core, Ui};
    use crate::backend::headless::{DrawOp, HeadlessBackend};
    use crate::backend::{Backend, PlatformSpec};
    use crate::geometry::Rect;
    use crate::property::Properties;

    fn setup() -> (Rc<HeadlessBackend>, Ui<u32>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend
            .open_window(&PlatformSpec::new("materialstatusbar"))
            .unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(Rc::clone(&core));
        (backend, ui)
    }

    #[test]
    fn every_part_is_painted() {
        let (backend, ui) = setup();
        let bar =
            MaterialStatusBar::new(&ui, Rect::new(0, 0, 240, 24), &["Ready", "3 items"]).unwrap();

        backend.render(bar.id());
        let ops = backend.ops(bar.id());
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Ready")),
            "the leading status was painted: {ops:?}"
        );
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "3 items")),
            "the second part was painted: {ops:?}"
        );
    }

    #[test]
    fn set_text_replaces_a_part_and_repaints() {
        let (backend, ui) = setup();
        let bar =
            MaterialStatusBar::new(&ui, Rect::new(0, 0, 240, 24), &["Ready", "3 items"]).unwrap();

        bar.set_text(1, "Done");
        assert_eq!(bar.text(1), Some("Done".to_string()));
        assert_eq!(
            bar.property("text"),
            Some(crate::property::Value::Text("Ready".into()))
        );

        backend.render(bar.id());
        let ops = backend.ops(bar.id());
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Done")),
            "the new text was painted: {ops:?}"
        );
    }
}
