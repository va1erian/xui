#![forbid(unsafe_code)]

//! [`MultilineEdit`]: a multi-line text area.
//!
//! The text and the caret live in the widget, so it edits the same way on every
//! backend. It paints its own field and handles its own input: a form designer
//! draws one to capture a property, and a normal app maps each change to its
//! `Msg` through [`MultilineEdit::on_change`].

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, TextStyle};
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
/// The design height of one line.
const LINE: Dip = Dip(18.0);

/// The (line index, start, end) char bounds of the line holding `at`.
fn line_span(text: &str, at: usize) -> (usize, usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let at = at.min(chars.len());
    let mut line = 0;
    let mut start = 0;
    for (i, c) in chars.iter().enumerate() {
        if i >= at {
            break;
        }
        if *c == '\n' {
            line += 1;
            start = i + 1;
        }
    }
    let end = chars[start..]
        .iter()
        .position(|c| *c == '\n')
        .map_or(chars.len(), |i| start + i);
    (line, start, end)
}

/// Inserts `character` at the caret and advances past it.
fn insert(text: &RefCell<String>, caret: &Cell<usize>, character: char) {
    let mut chars: Vec<char> = text.borrow().chars().collect();
    let at = caret.get().min(chars.len());
    chars.insert(at, character);
    caret.set(at + 1);
    *text.borrow_mut() = chars.into_iter().collect();
}

/// A multi-line text area.
pub struct MultilineEdit<M: 'static> {
    control: Control<M>,
    text: Rc<RefCell<String>>,
    caret: Rc<Cell<usize>>,
    enabled: Rc<Cell<bool>>,
    on_change: ChangeMapper<M>,
}

impl<M: 'static> MultilineEdit<M> {
    /// Creates a text area showing `text` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<MultilineEdit<M>> {
        let control = Control::new(
            ui,
            &NodeSpec::new(NodeKind::MultilineEdit, bounds).tab_stop(),
        )?;
        let state = Rc::new(RefCell::new(text.to_string()));
        let caret = Rc::new(Cell::new(text.chars().count()));
        let focused = Rc::new(Cell::new(false));
        let enabled = Rc::new(Cell::new(true));
        let on_change: ChangeMapper<M> = Rc::new(RefCell::new(None));

        {
            let text = Rc::clone(&state);
            let caret = Rc::clone(&caret);
            let focused = Rc::clone(&focused);
            let enabled = Rc::clone(&enabled);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            let ui = ui.clone();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                let dpi = canvas.dpi();
                canvas.clear(theme.input_background);
                let border = if focused.get() {
                    theme.border_focused
                } else {
                    theme.border
                };
                canvas.stroke_rect(bounds, border, 1.0);

                let pad = PADDING.to_px(dpi).value();
                let line_height = LINE.to_px(dpi).value().max(1);
                let inner = bounds.shrink(pad);
                let color = if enabled.get() {
                    theme.text
                } else {
                    theme.text_disabled
                };
                let style = TextStyle::new(color, TEXT_SIZE).middle();
                let value = text.borrow();
                for (i, line) in value.split('\n').enumerate() {
                    let top = inner.top + i as i32 * line_height;
                    let row = Rect::new(inner.left, top, inner.right, top + line_height);
                    canvas.draw_text(line, row, &style);
                }

                if focused.get() {
                    let (line, start, _) = line_span(&value, caret.get());
                    let count = value.chars().count();
                    let column = caret.get().min(count) - start;
                    let prefix: String = value.chars().skip(start).take(column).collect();
                    let advance = ui.measure_text(&prefix, &style, dpi).width;
                    let x = (inner.left + advance).min(inner.right);
                    let top = inner.top + line as i32 * line_height;
                    canvas.draw_line(
                        Point::new(x, top),
                        Point::new(x, top + line_height),
                        theme.text,
                        1.0,
                    );
                }
                if selected.get() {
                    canvas.stroke_rect(bounds, theme.accent, 2.0);
                }
            }));
        }
        {
            let text = Rc::clone(&state);
            let caret = Rc::clone(&caret);
            let focused = Rc::clone(&focused);
            let enabled = Rc::clone(&enabled);
            let on_change = Rc::clone(&on_change);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                // In design mode the editor handles input, not the field.
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                if !enabled.get() {
                    return None;
                }
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
                    Event::Char(character) if focused.get() => {
                        let character = *character;
                        if character == '\n' || !character.is_control() {
                            insert(&text, &caret, character);
                            changed = true;
                        } else {
                            return None;
                        }
                    }
                    Event::KeyDown {
                        key,
                        repeat,
                        system,
                        ..
                    } if focused.get() && *repeat <= 1 && !*system => match *key {
                        Key::RETURN => {
                            insert(&text, &caret, '\n');
                            changed = true;
                        }
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
                        Key::HOME => caret.set(line_span(&text.borrow(), caret.get()).1),
                        Key::END => caret.set(line_span(&text.borrow(), caret.get()).2),
                        Key::UP => {
                            let (_, start, _) = line_span(&text.borrow(), caret.get());
                            if start > 0 {
                                let (_, from, to) = line_span(&text.borrow(), start - 1);
                                caret.set(from + (caret.get() - start).min(to - from));
                            }
                        }
                        Key::DOWN => {
                            let value = text.borrow();
                            let (_, start, end) = line_span(&value, caret.get());
                            if end < value.chars().count() {
                                let (_, from, to) = line_span(&value, end + 1);
                                caret.set(from + (caret.get() - start).min(to - from));
                            }
                        }
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

        Ok(MultilineEdit {
            control,
            text: state,
            caret,
            enabled,
            on_change,
        })
    }

    /// Maps a change to the app's message: the closure returns `Some(msg)` to
    /// raise it, or `None` to ignore the change. It receives the new text.
    pub fn on_change(self, mapper: impl Fn(&str) -> Option<M> + 'static) -> MultilineEdit<M> {
        *self.on_change.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The text area's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// Gives the text area the keyboard focus.
    pub fn focus(&self) {
        self.control.focus();
    }

    /// Enables or disables the text area. A disabled area is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the text area selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> HasText for MultilineEdit<M> {
    fn text(&self) -> String {
        self.text.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = text.to_string();
        self.caret.set(text.chars().count());
        self.control.invalidate();
    }
}

impl<M: 'static> Properties for MultilineEdit<M> {
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
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::MultilineEdit;
    use crate::app::{App, Core, Runtime, Ui};
    use crate::backend::headless::HeadlessBackend;
    use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
    use crate::geometry::Rect;
    use crate::message::{Key, Modifiers};
    use crate::widget::HasText;

    struct TestApp(Rc<RefCell<Vec<u32>>>);

    impl App for TestApp {
        type Msg = u32;
        fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
            self.0.borrow_mut().push(msg);
        }
    }

    fn setup() -> (Rc<Core<u32>>, Ui<u32>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
        let core = Core::new(backend, window);
        let ui = Ui::new(Rc::clone(&core));
        (core, ui)
    }

    fn area(ui: &Ui<u32>, text: &str) -> MultilineEdit<u32> {
        MultilineEdit::new(ui, Rect::new(0, 0, 200, 100), text).unwrap()
    }

    fn key(key: Key) -> Event {
        Event::KeyDown {
            key,
            modifiers: Modifiers::NONE,
            repeat: 1,
            system: false,
        }
    }

    #[test]
    fn typing_a_newline_changes_the_text_and_raises_messages() {
        let (core, ui) = setup();
        let log = Rc::new(RefCell::new(Vec::new()));
        let edit = area(&ui, "").on_change(|text| Some(text.chars().count() as u32));
        let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

        runtime.deliver(edit.id(), &Event::SetFocus);
        runtime.deliver(edit.id(), &Event::Char('a'));
        runtime.deliver(edit.id(), &key(Key::RETURN));
        runtime.deliver(edit.id(), &Event::Char('b'));
        runtime.deliver(WidgetId::NONE, &Event::Wake);

        assert_eq!(edit.text(), "a\nb");
        assert_eq!(*log.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn backspace_removes_the_newline_before_the_caret() {
        let (core, ui) = setup();
        let edit = area(&ui, "a\n");
        let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

        runtime.deliver(edit.id(), &Event::SetFocus);
        runtime.deliver(edit.id(), &key(Key::BACK));

        assert_eq!(edit.text(), "a");
    }

    #[test]
    fn set_text_changes_the_text_without_raising_a_message() {
        let (_core, ui) = setup();
        let log = Rc::new(RefCell::new(Vec::new()));
        let captured = Rc::clone(&log);
        let edit = area(&ui, "one").on_change(move |text| {
            captured.borrow_mut().push(text.to_string());
            None
        });

        edit.set_text("two\nlines");

        assert_eq!(edit.text(), "two\nlines");
        assert!(log.borrow().is_empty(), "set_text raises nothing");
    }
}
