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
use crate::property::{Properties, Property, Value};
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
        let enabled = Rc::new(Cell::new(true));
        let hover = Rc::new(Cell::new(false));
        let pressed = Rc::new(Cell::new(false));
        let icon = Rc::new(Cell::new(None));
        let on_toggle: ToggleMapper<M> = Rc::new(RefCell::new(None));

        {
            let label = Rc::clone(&label);
            let icon = Rc::clone(&icon);
            let checked = Rc::clone(&checked);
            let enabled = Rc::clone(&enabled);
            let hover = Rc::clone(&hover);
            let pressed = Rc::clone(&pressed);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                let checked = checked.get();
                let bg = pick(hover.get(), theme.hover, theme.surface);
                let fill = pick(pressed.get(), theme.pressed, bg);
                backdrop(canvas, theme.background);
                let fill = if checked { theme.accent } else { fill };
                face(canvas, bounds, RADIUS, fill, &theme);
                let border = pick(hover.get(), theme.border_focused, theme.border);
                canvas.stroke_rounded_rect(bounds, RADIUS, border, 1.0);
                let base = pick(checked, theme.text_on_accent, theme.text);
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
        self.checked.set(checked);
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
        ]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
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
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::ToggleButton;
    use crate::app::{App, Core, Runtime, Ui};
    use crate::backend::headless::HeadlessBackend;
    use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
    use crate::geometry::Rect;
    use crate::icon::{IconRef, Lucide};
    use crate::message::{Modifiers, MouseButton};
    use crate::widget::Glyph;

    struct TestApp {
        log: Rc<RefCell<Vec<u32>>>,
    }

    impl App for TestApp {
        type Msg = u32;
        fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
            self.log.borrow_mut().push(msg);
        }
    }
    type Harness = (Ui<u32>, Rc<Runtime<TestApp>>, Rc<RefCell<Vec<u32>>>);
    #[rustfmt::skip]
    fn setup() -> Harness {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
        let core = Core::new(backend, window);
        let ui = Ui::new(Rc::clone(&core));
        let log = Rc::new(RefCell::new(Vec::new()));
        let runtime = Runtime::primary(core, TestApp { log: Rc::clone(&log) });
        (ui, runtime, log)
    }

    #[rustfmt::skip]
    fn click(runtime: &Runtime<TestApp>, target: WidgetId) {
        let m = Modifiers::NONE;
        let down = Event::MouseDown { x: 5, y: 5, button: MouseButton::Left, modifiers: m };
        let up = Event::MouseUp { x: 5, y: 5, button: MouseButton::Left, modifiers: m };
        let mv = Event::MouseMove { x: 6, y: 5, modifiers: m };
        runtime.deliver(target, &down);
        runtime.deliver(target, &mv);
        runtime.deliver(target, &up);
        runtime.deliver(WidgetId::NONE, &Event::Wake);
    }

    #[test]
    fn a_click_toggles_and_maps_a_message() {
        let (ui, runtime, log) = setup();
        let button = ToggleButton::new(&ui, Rect::new(0, 0, 80, 28), "Bold")
            .unwrap()
            .on_toggle(|checked| Some(checked as u32));
        assert!(!button.is_checked());
        click(&runtime, button.id());
        assert!(button.is_checked());
        assert_eq!(*log.borrow(), vec![1]);
    }

    #[test]
    fn a_disabled_toggle_button_ignores_clicks() {
        let (ui, runtime, log) = setup();
        let button = ToggleButton::new(&ui, Rect::new(0, 0, 80, 28), "Bold")
            .unwrap()
            .on_toggle(|checked| Some(checked as u32));
        button.set_enabled(false);
        click(&runtime, button.id());
        assert!(!button.is_checked(), "a disabled button toggles nothing");
        assert!(log.borrow().is_empty());
    }

    #[test]
    fn set_checked_raises_nothing() {
        let (ui, _runtime, log) = setup();
        let button = ToggleButton::new(&ui, Rect::new(0, 0, 80, 28), "Bold").unwrap();
        button.set_checked(true);
        assert!(button.is_checked());
        assert!(log.borrow().is_empty(), "a programmatic set raised nothing");
    }
    #[test]
    fn icon_is_set_replaced_and_cleared_without_toggling() {
        let (ui, runtime, log) = setup();
        let button = ToggleButton::new(&ui, Rect::new(0, 0, 28, 28), "")
            .unwrap()
            .icon(Lucide::Bold)
            .on_toggle(|checked| Some(checked as u32));
        assert_eq!(button.icon.get(), Some(IconRef::Lucide(Lucide::Bold)));
        button.set_icon(Some(Glyph::Play));
        assert_eq!(button.icon.get(), Some(IconRef::Glyph(Glyph::Play)));
        button.clear_icon();
        assert_eq!(button.icon.get(), None);
        assert!(!button.is_checked(), "changing the icon toggles nothing");
        click(&runtime, button.id());
        assert!(button.is_checked(), "an icon-only button still toggles");
        assert_eq!(*log.borrow(), vec![1]);
    }

    #[test]
    fn hovering_repaints_once_then_stays_quiet() {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
        let core = Core::new(backend.clone(), window);
        let ui = Ui::new(Rc::clone(&core));
        let button = ToggleButton::new(&ui, Rect::new(0, 0, 80, 28), "Bold").unwrap();
        let runtime = Runtime::primary(
            core,
            TestApp {
                log: Rc::new(RefCell::new(Vec::new())),
            },
        );
        let mv = |x| Event::MouseMove {
            x,
            y: 5,
            modifiers: Modifiers::NONE,
        };

        let before = backend.invalidations();
        runtime.deliver(button.id(), &mv(4));
        assert_eq!(
            backend.invalidations(),
            before + 1,
            "entering hover repaints"
        );
        for x in 5..25 {
            runtime.deliver(button.id(), &mv(x));
        }
        assert_eq!(
            backend.invalidations(),
            before + 1,
            "moves while hovered repaint nothing"
        );
        runtime.deliver(button.id(), &Event::MouseLeave);
        assert_eq!(backend.invalidations(), before + 2, "leaving repaints");
    }
}
