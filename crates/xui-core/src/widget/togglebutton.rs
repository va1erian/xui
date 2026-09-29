#![forbid(unsafe_code)]

//! [`ToggleButton`]: a push button that latches a checked state.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::Rect;
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
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
pub struct ToggleButton<M: 'static> {
    control: Control<M>,
    text: Rc<RefCell<String>>,
    checked: Rc<Cell<bool>>,
    enabled: Rc<Cell<bool>>,
    hover: Rc<Cell<bool>>,
    pressed: Rc<Cell<bool>>,
    on_toggle: ToggleMapper<M>,
}

impl<M: 'static> ToggleButton<M> {
    /// Creates a toggle button with no bounds of its own, for a layout to place (see
    /// [`crate::arrange`]); its size comes from [`Placeable`](super::Placeable).
    pub fn auto(ui: &Ui<M>, text: &str) -> Result<ToggleButton<M>> {
        ToggleButton::new(ui, Rect::default(), text)
    }

    /// Creates a toggle button labelled `text`, unchecked, at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<ToggleButton<M>> {
        let spec = NodeSpec::new(NodeKind::Button, bounds)
            .text(text)
            .tab_stop();
        let control = Control::new(ui, &spec)?;
        let label = Rc::new(RefCell::new(text.to_string()));
        let checked = Rc::new(Cell::new(false));
        let enabled = Rc::new(Cell::new(true));
        let hover = Rc::new(Cell::new(false));
        let pressed = Rc::new(Cell::new(false));
        let on_toggle: ToggleMapper<M> = Rc::new(RefCell::new(None));

        {
            let label = Rc::clone(&label);
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
                canvas.clear(theme.background);
                let fill = if checked { theme.accent } else { fill };
                canvas.fill_rounded_rect(bounds, RADIUS, fill);
                let border = pick(hover.get(), theme.border_focused, theme.border);
                canvas.stroke_rounded_rect(bounds, RADIUS, border, 1.0);
                let base = pick(checked, theme.text_on_accent, theme.text);
                let color = pick(enabled.get(), base, theme.text_disabled);
                let style = TextStyle::new(color, TEXT_SIZE).centered().middle();
                canvas.draw_text(&label.borrow(), bounds, &style);
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
            control,
            text: label,
            checked,
            enabled,
            hover,
            pressed,
            on_toggle,
        })
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
        *self.text.borrow_mut() = text.to_string();
        self.control.invalidate();
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
    use crate::message::{Modifiers, MouseButton};

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
