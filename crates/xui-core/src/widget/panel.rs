#![forbid(unsafe_code)]

//! [`Panel`]: a container that owns a group of child widgets.

use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::Rect;
use crate::property::{Properties, Property, Value};

/// A container node that owns child widgets.
///
/// Widgets built through [`Panel::ui`] are parented to the panel, so they are
/// clipped to its bounds and destroyed with it. The app positions them (or a
/// layout does) in the panel's own coordinates.
pub struct Panel<M: 'static> {
    control: Control<M>,
    scoped: Ui<M>,
}

impl<M: 'static> Panel<M> {
    /// Creates a panel at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect) -> Result<Panel<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Container, bounds))?;
        let scoped = ui.with_parent(control.id());

        let theme = ui.theme_handle();
        let selected = control.selected_handle();
        control.set_painter(Rc::new(move |canvas| {
            let theme = theme.get();
            let bounds = canvas.bounds();
            canvas.clear(theme.surface);
            canvas.stroke_rect(bounds, theme.border, 1.0);
            if selected.get() {
                canvas.stroke_rect(bounds, theme.accent, 2.0);
            }
        }));

        Ok(Panel { control, scoped })
    }

    /// The handle widgets built inside this panel parent to.
    pub fn ui(&self) -> &Ui<M> {
        &self.scoped
    }

    /// The panel's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Moves/resizes the panel (children keep their own coordinates).
    pub fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
    }

    /// Shows or hides the panel and its children.
    pub fn set_visible(&self, visible: bool) {
        self.control.set_visible(visible);
    }

    /// Enables or disables the panel and its children.
    pub fn set_enabled(&self, enabled: bool) {
        self.control.set_enabled(enabled);
    }

    /// Marks the panel selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for Panel<M> {
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

    use super::Panel;
    use crate::app::{Core, Ui};
    use crate::backend::headless::HeadlessBackend;
    use crate::backend::{Backend, PlatformSpec};
    use crate::geometry::Rect;
    use crate::widget::Label;

    fn setup() -> (Rc<HeadlessBackend>, Ui<u32>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("panel")).unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(core);
        (backend, ui)
    }

    #[test]
    fn a_panel_owns_its_children() {
        let (backend, ui) = setup();
        let panel = Panel::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
        let child = Label::new(panel.ui(), Rect::new(8, 8, 100, 28), "inside").unwrap();
        let panel_id = panel.id();
        assert!(backend.has_node(panel_id));
        assert!(backend.has_node(child.id()));

        drop(panel);
        assert!(!backend.has_node(panel_id), "the panel is gone");
        assert!(
            !backend.has_node(child.id()),
            "the child was destroyed with the panel"
        );
    }
}
