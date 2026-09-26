#![forbid(unsafe_code)]

//! [`GroupBox`]: a titled frame that groups related widgets.

use std::cell::RefCell;
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::Rect;
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

/// The design size of the title text.
const TEXT_SIZE: Dip = Dip(12.0);
/// How far below the top edge the frame's top border sits, leaving room for
/// the title to straddle it.
const INSET: Dip = Dip(8.0);
/// The horizontal padding between the frame's left edge and the title.
const PAD: Dip = Dip(8.0);

/// A titled frame that visually groups related widgets.
///
/// A group box is non-interactive. Every node is a sibling under the window
/// and a child created after the box paints above it, so the caller positions
/// and owns any widgets placed inside the frame's bounds.
pub struct GroupBox<M: 'static> {
    control: Control<M>,
    title: Rc<RefCell<String>>,
}

impl<M: 'static> GroupBox<M> {
    /// Creates an empty frame titled `title` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, title: &str) -> Result<GroupBox<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::GroupBox, bounds).text(title))?;
        let state = Rc::new(RefCell::new(title.to_string()));

        {
            let state = Rc::clone(&state);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                let dpi = canvas.dpi();
                canvas.clear(theme.background);

                let inset = INSET.to_px(dpi).value();
                let pad = PAD.to_px(dpi).value();
                let frame = Rect::new(bounds.left, bounds.top + inset, bounds.right, bounds.bottom);
                canvas.stroke_rect(frame, theme.border, 1.0);

                let size = TEXT_SIZE.to_px(dpi).value() as f32;
                let title = state.borrow();
                let width = (title.chars().count() as f32 * size * 0.5).round() as i32;
                let gap = Rect::new(
                    frame.left + pad,
                    bounds.top,
                    frame.left + pad + width,
                    bounds.top + inset * 2,
                );
                canvas.fill_rect(gap, theme.background);
                let style = TextStyle::new(theme.text, TEXT_SIZE).middle();
                canvas.draw_text(&title, gap, &style);

                if selected.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        Ok(GroupBox {
            control,
            title: state,
        })
    }

    /// The frame's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// Replaces the frame's title.
    pub fn set_title(&self, title: &str) {
        *self.title.borrow_mut() = title.to_string();
        self.control.invalidate();
    }

    /// Marks the frame selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }

    /// Whether the frame is selected.
    pub fn is_selected(&self) -> bool {
        self.control.is_selected()
    }
}

impl<M: 'static> HasText for GroupBox<M> {
    fn text(&self) -> String {
        self.title.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        self.set_title(text);
    }
}

impl<M: 'static> Properties for GroupBox<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "text",
            value: Value::Text(self.text()),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("text", Value::Text(text)) => {
                self.set_text(&text);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::GroupBox;
    use crate::app::{Core, Ui};
    use crate::backend::headless::{DrawOp, HeadlessBackend};
    use crate::backend::{Backend, PlatformSpec};
    use crate::geometry::Rect;
    use crate::widget::HasText;

    fn setup() -> (Rc<HeadlessBackend>, Ui<u32>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(core);
        (backend, ui)
    }

    #[test]
    fn a_group_box_paints_a_frame_and_its_title() {
        let (backend, ui) = setup();
        let group = GroupBox::new(&ui, Rect::new(0, 0, 200, 120), "Options").unwrap();

        backend.render(group.id());
        let ops = backend.ops(group.id());
        assert!(
            ops.iter().any(|op| matches!(op, DrawOp::Stroke(..))),
            "the frame was painted: {ops:?}"
        );
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Options")),
            "the title was painted: {ops:?}"
        );
    }

    #[test]
    fn changing_a_group_boxs_title_repaints_it() {
        let (backend, ui) = setup();
        let group = GroupBox::new(&ui, Rect::new(0, 0, 200, 120), "One").unwrap();
        group.set_text("Two");

        backend.render(group.id());
        let ops = backend.ops(group.id());
        assert_eq!(group.text(), "Two");
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Two")),
            "the new title was painted: {ops:?}"
        );
    }
}
