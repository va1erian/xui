#![forbid(unsafe_code)]

//! [`ToggleButton`]: a push button that latches a checked state.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::Tooltip;
use super::button::layout_content;
use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::Rect;
use crate::icon::{IconRef, draw_icon};
use crate::message::{Key, MouseButton};
use crate::motion::{Animated, Motion};
use crate::property::{Properties, Property, Value, node};
use crate::theme::look::{backdrop, face};
use crate::units::Dip;

/// Maps a new checked state to an optional app message.
type ToggleMapper<M> = Rc<RefCell<Option<Box<dyn Fn(bool) -> Option<M>>>>>;
/// The design size of the button text.
const TEXT_SIZE: Dip = Dip(12.0);
/// The corner radius of the button face.
const RADIUS: f32 = 4.0;

/// Returns `yes` when `cond`, else `no`.
fn pick<T>(cond: bool, yes: T, no: T) -> T {
    if cond { yes } else { no }
}

/// A push button that stays pressed: a click or Space/Return latches its
/// checked state. [`ToggleButton::set_checked`] changes it without an event.
///
/// Like [`Button`](super::Button) it can show an icon before its label, or an
/// icon alone (a formatting toolbar's bold or alignment toggle).
pub struct ToggleButton<M: 'static> {
    /// Declared first so it is dropped before the node it watches.
    tooltip: RefCell<Option<Tooltip<M>>>,
    control: Control<M>,
    text: Rc<RefCell<String>>,
    icon: Rc<Cell<Option<IconRef>>>,
    checked: Rc<Cell<bool>>,
    /// How latched the face looks: 0 unchecked, 1 checked, easing between.
    latch: Rc<Animated>,
    enabled: Rc<Cell<bool>>,
    hover: Rc<Cell<bool>>,
    pressed: Rc<Cell<bool>>,
    on_toggle: ToggleMapper<M>,
}

impl<M: 'static> ToggleButton<M> {
    /// Creates a toggle button labelled `text`, unchecked, at `bounds`.
    pub(crate) fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<ToggleButton<M>> {
        let spec = NodeSpec::new(NodeKind::Button, bounds)
            .text(text)
            .tab_stop();
        let control = Control::new(ui, &spec)?;
        let label = Rc::new(RefCell::new(text.to_string()));
        let checked = Rc::new(Cell::new(false));
        let latch = Rc::new(Animated::new(0.0));
        let enabled = Rc::new(Cell::new(true));
        let hover = Rc::new(Cell::new(false));
        let pressed = Rc::new(Cell::new(false));
        let icon = Rc::new(Cell::new(None));
        let on_toggle: ToggleMapper<M> = Rc::new(RefCell::new(None));

        {
            let label = Rc::clone(&label);
            let icon = Rc::clone(&icon);
            let latch = Rc::clone(&latch);
            let time = ui.frame_time_handle();
            let enabled = Rc::clone(&enabled);
            let hover = Rc::clone(&hover);
            let pressed = Rc::clone(&pressed);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                let latched = latch.value(time.get());
                let bg = pick(hover.get(), theme.hover, theme.surface);
                let fill = pick(pressed.get(), theme.pressed, bg);
                backdrop(canvas, theme.background);
                let fill = fill.lerp(theme.accent, latched);
                face(canvas, bounds, RADIUS, fill, &theme);
                let border = pick(hover.get(), theme.border_focused, theme.border);
                canvas.stroke_rounded_rect(bounds, RADIUS, border, 1.0);
                let base = theme.text.lerp(theme.text_on_accent, latched);
                let color = pick(enabled.get(), base, theme.text_disabled);
                let text = label.borrow();
                let (icon_rect, text_rect) =
                    layout_content(bounds, icon.get().is_some(), text.is_empty());
                if let Some(icon_rect) = icon_rect
                    && let Some(icon) = icon.get()
                {
                    let dpi = canvas.dpi();
                    draw_icon(canvas, icon, icon_rect, color, dpi);
                }
                let style = TextStyle::new(color, TEXT_SIZE).centered().middle();
                canvas.draw_text(&text, text_rect, &style);
                if selected.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        {
            let checked = Rc::clone(&checked);
            let latch = Rc::clone(&latch);
            let enabled = Rc::clone(&enabled);
            let hover = Rc::clone(&hover);
            let pressed = Rc::clone(&pressed);
            let on_toggle = Rc::clone(&on_toggle);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                if !enabled.get() {
                    return None;
                }
                let mut toggle = false;
                match event {
                    Event::MouseDown {
                        button: MouseButton::Left,
                        ..
                    } => pressed.set(true),
                    // Only entering hover changes the face; a move while
                    // already hovered repaints nothing.
                    Event::MouseMove { .. } => {
                        if hover.replace(true) {
                            return None;
                        }
                    }
                    Event::MouseLeave | Event::CaptureChanged => {
                        hover.set(false);
                        pressed.set(false);
                    }
                    Event::MouseUp { .. } => toggle = pressed.replace(false),
                    Event::KeyDown {
                        key,
                        repeat,
                        system,
                        ..
                    } if matches!(*key, Key::SPACE | Key::RETURN) && *repeat <= 1 && !*system => {
                        toggle = true
                    }
                    _ => return None,
                }
                ui.invalidate(id);
                if !toggle {
                    return None;
                }
                let state = !checked.get();
                checked.set(state);
                ui.animate(id, &latch, f32::from(u8::from(state)), Motion::QUICK);
                let mapper = on_toggle.borrow();
                if let Some(mapper) = mapper.as_ref() {
                    return mapper(state);
                }
                None
            });
        }

        Ok(ToggleButton {
            tooltip: RefCell::new(None),
            control,
            text: label,
            icon,
            checked,
            latch,
            enabled,
            hover,
            pressed,
            on_toggle,
        })
    }

    /// Draws `icon` before the label (or centred when there is no label), in
    /// the label's colour: on-accent while checked, dimmed while disabled.
    ///
    /// Any [`IconRef`] works: a generated [`Lucide`](crate::icon::Lucide) icon,
    /// the legacy [`Icon`](super::Icon) set or a [`Glyph`](super::Glyph).
    pub fn icon(self, icon: impl Into<IconRef>) -> ToggleButton<M> {
        self.set_icon(Some(icon));
        self
    }

    /// Replaces the leading icon, or removes it with `None`.
    ///
    /// Accepts the same [`IconRef`] inputs as [`ToggleButton::icon`]; use
    /// [`ToggleButton::clear_icon`] to remove the icon without a type
    /// annotation on `None`.
    pub fn set_icon(&self, icon: Option<impl Into<IconRef>>) {
        self.icon.set(icon.map(Into::into));
        self.control.invalidate();
        self.control.invalidate_layout();
    }

    /// Whether the button draws an icon, for its natural size.
    pub(super) fn has_icon(&self) -> bool {
        self.icon.get().is_some()
    }

    /// Shows `text` in a tooltip while the pointer rests on the button,
    /// replacing any earlier one; an empty `text` removes it.
    pub fn set_tooltip(&self, text: &str) -> Result<()> {
        let tip = if text.is_empty() {
            None
        } else {
            Some(Tooltip::attach(self.control.ui(), self.control.id(), text)?)
        };
        self.tooltip.replace(tip);
        Ok(())
    }

    /// Removes the button's leading icon.
    pub fn clear_icon(&self) {
        self.icon.set(None);
        self.control.invalidate();
        self.control.invalidate_layout();
    }

    /// Maps a toggle to the app's message: the closure receives the new state
    /// and returns `Some(msg)` to raise it, or `None` to ignore it.
    pub fn on_toggle(self, mapper: impl Fn(bool) -> Option<M> + 'static) -> ToggleButton<M> {
        *self.on_toggle.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The button's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// Whether the button is checked.
    pub fn is_checked(&self) -> bool {
        self.checked.get()
    }

    /// Sets the checked state without raising the toggle event.
    pub fn set_checked(&self, checked: bool) {
        if self.checked.replace(checked) != checked {
            let target = f32::from(u8::from(checked));
            self.control
                .ui()
                .animate(self.id(), &self.latch, target, Motion::QUICK);
        }
        self.control.invalidate();
    }

    /// Enables or disables the button. A disabled button is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        if !enabled {
            self.hover.set(false);
            self.pressed.set(false);
        }
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the button selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> HasText for ToggleButton<M> {
    fn text(&self) -> String {
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        // Apps refresh status lines with the text they already show: an
        // unchanged text needs neither a repaint nor a re-flow.
        if *self.text.borrow() == text {
            return;
        }
        *self.text.borrow_mut() = text.to_string();
        self.control.invalidate();
        self.control.invalidate_layout();
    }
}

impl<M: 'static> Properties for ToggleButton<M> {
    fn properties(&self) -> Vec<Property> {
        vec![
            property("text", Value::Text(self.text())),
            property("checked", Value::Bool(self.is_checked())),
            node::enabled(self.control.ui(), self.id()),
            node::visible(self.control.ui(), self.id()),
        ]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        if node::set_visible(self.control.ui(), self.id(), name, &value) {
            return true;
        }
        match (name, value) {
            ("enabled", Value::Bool(enabled)) => self.set_enabled(enabled),
            ("text", Value::Text(text)) => self.set_text(&text),
            ("checked", Value::Bool(checked)) => self.set_checked(checked),
            _ => return false,
        }
        true
    }
}

/// Wraps a name and value as a [`Property`].
fn property(name: &'static str, value: Value) -> Property {
    Property { name, value }
}
#[cfg(test)]
mod tests;
