#![forbid(unsafe_code)]

//! [`Button`]: a painted push button that maps a click to the app's message.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::Rect;
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

/// Maps a click to an optional app message.
type ClickMapper<M> = Rc<RefCell<Option<Box<dyn Fn() -> Option<M>>>>>;

/// The design size of the button text.
const TEXT_SIZE: Dip = Dip(12.0);
/// The corner radius of the button face.
const RADIUS: f32 = 4.0;

/// The visual state of a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ButtonState {
    Normal,
    Hover,
    Pressed,
    Disabled,
}

/// A painted push button.
///
/// Its click is mapped to the app's `Msg` through [`Button::on_click`]; the
/// click is raised only once per press-and-release, and only by a left button
/// (or Space/Enter) while enabled.
pub struct Button<M: 'static> {
    control: Control<M>,
    state: Rc<Cell<ButtonState>>,
    text: Rc<RefCell<String>>,
    on_click: ClickMapper<M>,
}

impl<M: 'static> Button<M> {
    /// Creates a button labelled `text` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<Button<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Button, bounds).text(text))?;
        let state = Rc::new(Cell::new(ButtonState::Normal));
        let label = Rc::new(RefCell::new(text.to_string()));
        let on_click: ClickMapper<M> = Rc::new(RefCell::new(None));

        {
            let state = Rc::clone(&state);
            let label = Rc::clone(&label);
            let theme = ui.theme_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let state = state.get();
                // Paint the node's background so the rounded face's corners do
                // not show the uninitialised back buffer.
                canvas.clear(theme.background);
                let fill = match state {
                    ButtonState::Normal | ButtonState::Disabled => theme.surface,
                    ButtonState::Hover => theme.hover,
                    ButtonState::Pressed => theme.pressed,
                };
                let bounds = canvas.bounds();
                canvas.fill_rounded_rect(bounds, RADIUS, fill);
                canvas.stroke_rounded_rect(bounds, RADIUS, theme.border, 1.0);
                let color = if state == ButtonState::Disabled {
                    theme.text_disabled
                } else {
                    theme.text
                };
                let style = TextStyle::new(color, TEXT_SIZE).centered().middle();
                canvas.draw_text(&label.borrow(), bounds, &style);
            }));
        }

        {
            let state = Rc::clone(&state);
            let on_click = Rc::clone(&on_click);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                let mut activate = false;
                match event {
                    Event::MouseMove { .. } => {
                        // Do not disturb an active press: a pointer that
                        // jitters between press and release must still click.
                        if state.get() == ButtonState::Normal {
                            state.set(ButtonState::Hover);
                        }
                    }
                    Event::MouseLeave | Event::CaptureChanged => {
                        if state.get() != ButtonState::Disabled {
                            state.set(ButtonState::Normal);
                        }
                    }
                    Event::MouseDown {
                        button: MouseButton::Left,
                        ..
                    } => {
                        if state.get() != ButtonState::Disabled {
                            state.set(ButtonState::Pressed);
                        }
                    }
                    Event::MouseUp {
                        button: MouseButton::Left,
                        ..
                    } => {
                        if state.get() == ButtonState::Pressed {
                            state.set(ButtonState::Hover);
                            activate = true;
                        }
                    }
                    Event::KeyDown {
                        key,
                        repeat,
                        system,
                        ..
                    } if matches!(*key, Key::SPACE | Key::RETURN)
                        && *repeat <= 1
                        && !*system
                        && state.get() != ButtonState::Disabled =>
                    {
                        activate = true;
                    }
                    _ => return None,
                }
                ui.invalidate(id);
                if activate {
                    let mapper = on_click.borrow();
                    if let Some(mapper) = mapper.as_ref() {
                        return mapper();
                    }
                }
                None
            });
        }

        Ok(Button {
            control,
            state,
            text: label,
            on_click,
        })
    }

    /// Maps a click to the app's message: the closure returns `Some(msg)` to
    /// raise it, or `None` to ignore the click.
    pub fn on_click(self, mapper: impl Fn() -> Option<M> + 'static) -> Button<M> {
        *self.on_click.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The button's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// Whether the button is enabled.
    pub fn is_enabled(&self) -> bool {
        self.state.get() != ButtonState::Disabled
    }

    /// Enables or disables the button. A disabled button is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        self.state.set(if enabled {
            ButtonState::Normal
        } else {
            ButtonState::Disabled
        });
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }
}

impl<M: 'static> HasText for Button<M> {
    fn text(&self) -> String {
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = text.to_string();
        self.control.invalidate();
    }
}

impl<M: 'static> Properties for Button<M> {
    fn properties(&self) -> Vec<Property> {
        vec![
            Property {
                name: "text",
                value: Value::Text(self.text()),
            },
            Property {
                name: "enabled",
                value: Value::Bool(self.is_enabled()),
            },
        ]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("text", Value::Text(text)) => {
                self.set_text(&text);
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
