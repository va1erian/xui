#![forbid(unsafe_code)]

//! [`Edit`]: a single-line text field.
//!
//! The text and the caret live in the widget, so it edits the same way on every
//! backend. A form designer draws one to capture a property; a normal app maps
//! each change to its `Msg` through [`Edit::on_change`].
//!
//! A backend that hosts a native control ([`ImplKind::Native`], as the Win32
//! backend does for a real `EDIT`) draws and edits the field itself; the widget
//! then draws nothing and syncs its text from the control's change events.
//! Otherwise ([`Painted`](ImplKind::Painted)) the widget paints the field and
//! handles its own input.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{Event, ImplKind, NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::{Point, Rect};
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

/// Maps new text to an optional app message.
type ChangeMapper<M> = Rc<RefCell<Option<Box<dyn Fn(&str) -> Option<M>>>>>;

/// The design size of the field text.
const TEXT_SIZE: Dip = Dip(12.0);
/// Padding inside the field border, as a design value.
const PADDING: Dip = Dip(4.0);

/// A single-line text field.
pub struct Edit<M: 'static> {
    control: Control<M>,
    text: Rc<RefCell<String>>,
    caret: Rc<Cell<usize>>,
    /// Whether the backend hosts a native control that edits itself.
    native: bool,
    /// Set while the widget itself sets the native text, so the resulting
    /// change notification does not loop back through `on_change`.
    setting: Rc<Cell<bool>>,
    on_change: ChangeMapper<M>,
}

impl<M: 'static> Edit<M> {
    /// Creates a field showing `text` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<Edit<M>> {
        let native = ui.supports(NodeKind::Edit) == ImplKind::Native;
        let control = Control::new(
            ui,
            &NodeSpec::new(NodeKind::Edit, bounds).text(text).tab_stop(),
        )?;
        let state = Rc::new(RefCell::new(text.to_string()));
        let caret = Rc::new(Cell::new(text.chars().count()));
        let focused = Rc::new(Cell::new(false));
        let setting = Rc::new(Cell::new(false));
        let on_change: ChangeMapper<M> = Rc::new(RefCell::new(None));

        if !native {
            let text = Rc::clone(&state);
            let caret = Rc::clone(&caret);
            let focused = Rc::clone(&focused);
            let theme = ui.theme_handle();
            let ui = ui.clone();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                canvas.clear(theme.input_background);
                let border = if focused.get() {
                    theme.border_focused
                } else {
                    theme.border
                };
                canvas.stroke_rect(bounds, border, 1.0);

                let pad = PADDING.to_px(canvas.dpi()).value();
                let inner = bounds.shrink(pad);
                let style = TextStyle::new(theme.text, TEXT_SIZE).middle();
                let value = text.borrow();
                canvas.draw_text(&value, inner, &style);

                if focused.get() {
                    let prefix: String = value.chars().take(caret.get()).collect();
                    let advance = ui.measure_text(&prefix, &style, canvas.dpi()).width;
                    let x = (inner.left + advance).min(inner.right);
                    canvas.draw_line(
                        Point::new(x, inner.top),
                        Point::new(x, inner.bottom),
                        theme.text,
                        1.0,
                    );
                }
            }));
        }

        {
            let text = Rc::clone(&state);
            let caret = Rc::clone(&caret);
            let focused = Rc::clone(&focused);
            let setting = Rc::clone(&setting);
            let on_change = Rc::clone(&on_change);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                let mut changed = false;
                match event {
                    Event::SetFocus => focused.set(true),
                    Event::KillFocus => focused.set(false),
                    Event::MouseDown {
                        button: MouseButton::Left,
                        ..
                    } => {
                        focused.set(true);
                        ui.focus(id);
                    }
                    // A native control edits itself and reports the change; read
                    // the new text back and raise it.
                    Event::TextChanged => {
                        let value = ui.text(id);
                        *text.borrow_mut() = value.clone();
                        // A change the widget itself made is not a user edit.
                        if setting.get() {
                            return None;
                        }
                        let mapper = on_change.borrow();
                        if let Some(mapper) = mapper.as_ref() {
                            return mapper(&value);
                        }
                        return None;
                    }
                    // Only the focused field edits.
                    Event::Char(character) if focused.get() && !character.is_control() => {
                        let mut chars: Vec<char> = text.borrow().chars().collect();
                        let at = caret.get().min(chars.len());
                        chars.insert(at, *character);
                        caret.set(at + 1);
                        *text.borrow_mut() = chars.into_iter().collect();
                        changed = true;
                    }
                    Event::KeyDown { key, .. } if focused.get() => match *key {
                        Key::BACK => {
                            let mut chars: Vec<char> = text.borrow().chars().collect();
                            let at = caret.get().min(chars.len());
                            if at > 0 {
                                chars.remove(at - 1);
                                caret.set(at - 1);
                                *text.borrow_mut() = chars.into_iter().collect();
                                changed = true;
                            }
                        }
                        Key::DELETE => {
                            let mut chars: Vec<char> = text.borrow().chars().collect();
                            let at = caret.get().min(chars.len());
                            if at < chars.len() {
                                chars.remove(at);
                                *text.borrow_mut() = chars.into_iter().collect();
                                changed = true;
                            }
                        }
                        Key::LEFT => caret.set(caret.get().saturating_sub(1)),
                        Key::RIGHT => {
                            let len = text.borrow().chars().count();
                            caret.set((caret.get() + 1).min(len));
                        }
                        Key::HOME => caret.set(0),
                        Key::END => caret.set(text.borrow().chars().count()),
                        _ => return None,
                    },
                    _ => return None,
                }
                ui.invalidate(id);
                if changed {
                    let mapper = on_change.borrow();
                    if let Some(mapper) = mapper.as_ref() {
                        let value = text.borrow().clone();
                        return mapper(&value);
                    }
                }
                None
            });
        }

        Ok(Edit {
            control,
            text: state,
            caret,
            native,
            setting,
            on_change,
        })
    }

    /// Maps a change to the app's message: the closure returns `Some(msg)` to
    /// raise it, or `None` to ignore the change. It receives the new text.
    pub fn on_change(self, mapper: impl Fn(&str) -> Option<M> + 'static) -> Edit<M> {
        *self.on_change.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The field's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// Gives the field the keyboard focus.
    pub fn focus(&self) {
        self.control.focus();
    }
}

impl<M: 'static> HasText for Edit<M> {
    fn text(&self) -> String {
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = text.to_string();
        self.caret.set(text.chars().count());
        if self.native {
            // Push to the control; its change notification is suppressed so
            // `on_change` fires only for real edits.
            self.setting.set(true);
            self.control.set_text(text);
            self.setting.set(false);
        } else {
            self.control.invalidate();
        }
    }
}

impl<M: 'static> Properties for Edit<M> {
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
