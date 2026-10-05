#![forbid(unsafe_code)]

//! [`CheckBox`]: a themed check box with a label.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::{Point, Rect};
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::theme::look::{self, backdrop};
use crate::units::Dip;

/// Maps a new checked state to an optional app message.
type ToggleMapper<M> = Rc<RefCell<Option<Box<dyn Fn(bool) -> Option<M>>>>>;

/// The design size of the label text.
const TEXT_SIZE: Dip = Dip(12.0);
/// The design size of the box.
const BOX: Dip = Dip(16.0);
/// The gap between the box and the label.
const GAP: Dip = Dip(8.0);

/// A check box with a label.
pub struct CheckBox<M: 'static> {
    control: Control<M>,
    text: Rc<RefCell<String>>,
    checked: Rc<Cell<bool>>,
    enabled: Rc<Cell<bool>>,
    on_toggle: ToggleMapper<M>,
}

impl<M: 'static> CheckBox<M> {
    /// Creates a check box labelled `text`, unchecked, at `bounds`.
    pub(crate) fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<CheckBox<M>> {
        let control = Control::new(
            ui,
            &NodeSpec::new(NodeKind::CheckBox, bounds)
                .text(text)
                .tab_stop(),
        )?;
        let label = Rc::new(RefCell::new(text.to_string()));
        let checked = Rc::new(Cell::new(false));
        let enabled = Rc::new(Cell::new(true));
        let on_toggle: ToggleMapper<M> = Rc::new(RefCell::new(None));

        {
            let label = Rc::clone(&label);
            let checked = Rc::clone(&checked);
            let enabled = Rc::clone(&enabled);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                let dpi = canvas.dpi();
                let enabled = enabled.get();
                backdrop(canvas, theme.background);

                let box_size = BOX.to_px(dpi).value();
                let gap = GAP.to_px(dpi).value();
                let top = bounds.top + (bounds.height() - box_size) / 2;
                let square = Rect::new(bounds.left, top, bounds.left + box_size, top + box_size);
                let fancy = look::decorated(&theme) && enabled;
                if fancy {
                    // A filled, glowing box when checked; an inset well when not.
                    let center = Point::new(square.left + box_size / 2, square.top + box_size / 2);
                    if checked.get() {
                        look::glow(canvas, center, box_size as f32 / 2.0, &theme);
                        look::face(canvas, square, 3.0, theme.accent, &theme);
                    } else {
                        canvas.fill_rounded_rect(square, 3.0, theme.input_background);
                        canvas.stroke_rounded_rect(square, 3.0, theme.input_border, 1.0);
                    }
                } else {
                    canvas.fill_rect(square, theme.input_background);
                }
                if !fancy {
                    canvas.stroke_rect(
                        square,
                        if enabled {
                            theme.input_border
                        } else {
                            theme.text_disabled
                        },
                        1.0,
                    );
                }
                if checked.get() {
                    // A check mark drawn as two strokes.
                    let color = if fancy {
                        theme.text_on_accent
                    } else if enabled {
                        theme.accent
                    } else {
                        theme.text_disabled
                    };
                    let x = square.left as f32;
                    let y = square.top as f32;
                    let s = box_size as f32;
                    canvas.draw_line(
                        Point::new((x + s * 0.22) as i32, (y + s * 0.52) as i32),
                        Point::new((x + s * 0.42) as i32, (y + s * 0.72) as i32),
                        color,
                        2.0,
                    );
                    canvas.draw_line(
                        Point::new((x + s * 0.42) as i32, (y + s * 0.72) as i32),
                        Point::new((x + s * 0.78) as i32, (y + s * 0.28) as i32),
                        color,
                        2.0,
                    );
                }

                let text_rect =
                    Rect::new(square.right + gap, bounds.top, bounds.right, bounds.bottom);
                let text_color = if enabled {
                    theme.text
                } else {
                    theme.text_disabled
                };
                let style = TextStyle::new(text_color, TEXT_SIZE).middle();
                canvas.draw_text(&label.borrow(), text_rect, &style);

                if selected.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }

        {
            let checked = Rc::clone(&checked);
            let enabled = Rc::clone(&enabled);
            let on_toggle = Rc::clone(&on_toggle);
            let pressed = Rc::new(Cell::new(false));
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                if !enabled.get() {
                    return None;
                }
                let toggle = match event {
                    Event::MouseDown {
                        button: MouseButton::Left,
                        ..
                    } => {
                        pressed.set(true);
                        false
                    }
                    Event::MouseLeave | Event::CaptureChanged => {
                        pressed.set(false);
                        false
                    }
                    Event::MouseUp {
                        button: MouseButton::Left,
                        ..
                    } => pressed.replace(false),
                    Event::KeyDown {
                        key: Key::SPACE,
                        repeat,
                        system,
                        ..
                    } if *repeat <= 1 && !*system => true,
                    _ => return None,
                };
                if !toggle {
                    return None;
                }
                let state = !checked.get();
                checked.set(state);
                ui.invalidate(id);
                let mapper = on_toggle.borrow();
                if let Some(mapper) = mapper.as_ref() {
                    return mapper(state);
                }
                None
            });
        }

        Ok(CheckBox {
            control,
            text: label,
            checked,
            enabled,
            on_toggle,
        })
    }

    /// Maps a toggle to the app's message: the closure receives the new state
    /// and returns `Some(msg)` to raise it, or `None` to ignore it.
    pub fn on_toggle(self, mapper: impl Fn(bool) -> Option<M> + 'static) -> CheckBox<M> {
        *self.on_toggle.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The box's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// Whether the box is checked.
    pub fn is_checked(&self) -> bool {
        self.checked.get()
    }

    /// Sets the checked state without raising the toggle event.
    pub fn set_checked(&self, checked: bool) {
        self.checked.set(checked);
        self.control.invalidate();
    }

    /// Whether the box is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    /// Enables or disables the box. A disabled box is dimmed and ignores input.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the box selected (a form editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> HasText for CheckBox<M> {
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

impl<M: 'static> Properties for CheckBox<M> {
    fn properties(&self) -> Vec<Property> {
        vec![
            Property {
                name: "text",
                value: Value::Text(self.text()),
            },
            Property {
                name: "checked",
                value: Value::Bool(self.is_checked()),
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
            ("checked", Value::Bool(checked)) => {
                self.set_checked(checked);
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
