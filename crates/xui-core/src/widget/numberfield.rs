#![forbid(unsafe_code)]
//! [`NumberField`]: a numeric text field with steppers and a range.
use super::control::Control;
use crate::app::Ui;
use crate::backend::{Canvas, Event, NodeKind, NodeSpec, Result, TextStyle};
use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::units::Dip;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
type MapperCell<M> = Rc<RefCell<Option<Box<dyn Fn(f64) -> Option<M>>>>>;
const TEXT_SIZE: Dip = Dip(12.0);
const PADDING: Dip = Dip(4.0);
const STEP_W: Dip = Dip(18.0);
const DEFAULT_STEP: f64 = 1.0;
fn show_value(value: f64, text: &RefCell<String>, caret: &Cell<usize>) {
    let formatted = value.to_string();
    caret.set(formatted.chars().count());
    *text.borrow_mut() = formatted;
}
fn insert_char(text: &RefCell<String>, caret: &Cell<usize>, character: char) -> bool {
    if !matches!(character, '0'..='9' | '.' | '-') {
        return false;
    }
    let mut chars: Vec<char> = text.borrow().chars().collect();
    let at = caret.get().min(chars.len());
    chars.insert(at, character);
    caret.set(at + 1);
    *text.borrow_mut() = chars.into_iter().collect();
    true
}
fn edit_key(text: &RefCell<String>, caret: &Cell<usize>, key: Key) -> bool {
    let mut chars: Vec<char> = text.borrow().chars().collect();
    let at = caret.get().min(chars.len());
    match key {
        Key::BACK if at > 0 => {
            chars.remove(at - 1);
            caret.set(at - 1);
        }
        Key::DELETE if at < chars.len() => {
            chars.remove(at);
        }
        Key::LEFT => caret.set(at.saturating_sub(1)),
        Key::RIGHT => caret.set((at + 1).min(chars.len())),
        Key::HOME => caret.set(0),
        Key::END => caret.set(chars.len()),
        _ => return false,
    }
    *text.borrow_mut() = chars.into_iter().collect();
    true
}
fn draw_chevron(canvas: &mut dyn Canvas, rect: Rect, up: bool, color: Color) {
    let (cx, half) = (rect.left + rect.width() / 2, (rect.width() / 4).max(2));
    let rise = (rect.height() / 4).max(2);
    let (apex, base) = if up {
        (rect.top + rise, rect.top + rise * 2)
    } else {
        (rect.bottom - rise, rect.bottom - rise * 2)
    };
    let a = Point::new(cx, apex);
    canvas.draw_line(a, Point::new(cx - half, base), color, 1.0);
    canvas.draw_line(a, Point::new(cx + half, base), color, 1.0);
}
struct State {
    value: Cell<f64>,
    min: Cell<f64>,
    max: Cell<f64>,
    step: Cell<f64>,
    text: RefCell<String>,
    caret: Cell<usize>,
    focused: Cell<bool>,
    enabled: Cell<bool>,
}
impl State {
    fn clamp(&self, value: f64) -> f64 {
        let (min, max) = (self.min.get(), self.max.get());
        if value.is_finite() {
            value.max(min).min(max)
        } else {
            min
        }
    }
}
/// A numeric text field with a range, an editable value and −/+ steppers.
pub struct NumberField<M: 'static> {
    control: Control<M>,
    state: Rc<State>,
    on_change: MapperCell<M>,
    on_commit: MapperCell<M>,
}
impl<M: 'static> NumberField<M> {
    /// Creates a field for `min..=max` at its minimum; a non-positive `step` is `1`.
    pub fn new(ui: &Ui<M>, bounds: Rect, min: f64, max: f64, step: f64) -> Result<NumberField<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Edit, bounds).tab_stop())?;
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        let s = Rc::new(State {
            value: Cell::new(min),
            min: Cell::new(min),
            max: Cell::new(max),
            step: Cell::new(if step > 0.0 { step } else { DEFAULT_STEP }),
            text: RefCell::new(min.to_string()),
            caret: Cell::new(min.to_string().chars().count()),
            focused: Cell::new(false),
            enabled: Cell::new(true),
        });
        let on_change: MapperCell<M> = Rc::new(RefCell::new(None));
        let on_commit: MapperCell<M> = Rc::new(RefCell::new(None));
        {
            let s = Rc::clone(&s);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            let ui = ui.clone();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let b = canvas.bounds();
                let dpi = canvas.dpi();
                canvas.clear(theme.input_background);
                let sl = b.right - STEP_W.to_px(dpi).value().max(1);
                let mid = b.top + b.height() / 2;
                let border = if s.focused.get() {
                    theme.border_focused
                } else {
                    theme.border
                };
                canvas.stroke_rect(b, border, 1.0);
                canvas.draw_line(Point::new(sl, b.top), Point::new(sl, b.bottom), border, 1.0);
                canvas.draw_line(Point::new(sl, mid), Point::new(b.right, mid), border, 1.0);
                let color = if s.enabled.get() {
                    theme.text
                } else {
                    theme.text_disabled
                };
                let top_btn = Rect::new(sl, b.top, b.right, mid);
                let bot_btn = Rect::new(sl, mid, b.right, b.bottom);
                draw_chevron(&mut *canvas, top_btn, true, color);
                draw_chevron(&mut *canvas, bot_btn, false, color);
                let pad = PADDING.to_px(dpi).value();
                let tr = Rect::new(b.left + pad, b.top, (sl - pad).max(b.left + pad), b.bottom);
                let style = TextStyle::new(color, TEXT_SIZE).middle();
                let value = s.text.borrow();
                canvas.draw_text(&value, tr, &style);
                if s.focused.get() {
                    let prefix: String = value.chars().take(s.caret.get()).collect();
                    let x = (tr.left + ui.measure_text(&prefix, &style, dpi).width).min(tr.right);
                    canvas.draw_line(Point::new(x, tr.top), Point::new(x, tr.bottom), color, 1.0);
                }
                if selected.get() {
                    canvas.stroke_rect(b, theme.accent, 2.0);
                }
            }));
        }
        {
            let s = Rc::clone(&s);
            let on_change = Rc::clone(&on_change);
            let on_commit = Rc::clone(&on_commit);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                if event.is_input() && (ui.is_design_mode() || !s.enabled.get()) {
                    return None;
                }
                let (mut redraw, mut changed, mut commit) = (false, false, false);
                let apply = |new: f64| {
                    let changed = (new - s.value.get()).abs() > f64::EPSILON;
                    s.value.set(new);
                    show_value(new, &s.text, &s.caret);
                    changed
                };
                let commit_text = || {
                    let raw = s.text.borrow().trim().to_string();
                    let parsed = raw.parse::<f64>().ok().filter(|v| v.is_finite());
                    apply(parsed.map_or(s.value.get(), |v| s.clamp(v)))
                };
                let step_by = |delta: f64| apply(s.clamp(s.value.get() + delta * s.step.get()));
                match event {
                    Event::SetFocus => s.focused.set(true),
                    Event::KillFocus => {
                        s.focused.set(false);
                        changed = commit_text();
                        commit = true;
                        redraw = true;
                    }
                    Event::MouseDown {
                        x,
                        y,
                        button: MouseButton::Left,
                        ..
                    } => {
                        s.focused.set(true);
                        ui.focus(id);
                        redraw = true;
                        let b = ui.bounds(id);
                        let sw = STEP_W.to_px(ui.dpi()).value().max(1);
                        if *x >= b.right - sw {
                            let top = b.top + b.height() / 2;
                            changed = step_by(if *y < top { 1.0 } else { -1.0 });
                            commit = true;
                        }
                    }
                    Event::Char(c) if s.focused.get() => {
                        redraw = insert_char(&s.text, &s.caret, *c);
                    }
                    Event::KeyDown {
                        key,
                        repeat,
                        system,
                        ..
                    } => {
                        if !s.focused.get() || *repeat > 1 || *system {
                            return None;
                        }
                        match *key {
                            Key::UP => (changed, commit, redraw) = (step_by(1.0), true, true),
                            Key::DOWN => (changed, commit, redraw) = (step_by(-1.0), true, true),
                            Key::RETURN => (changed, commit, redraw) = (commit_text(), true, true),
                            _ => redraw = edit_key(&s.text, &s.caret, *key),
                        }
                    }
                    _ => return None,
                }
                if redraw {
                    ui.invalidate(id);
                }
                if commit && let Some(f) = on_commit.borrow().as_ref() {
                    return f(s.value.get());
                }
                if changed && let Some(f) = on_change.borrow().as_ref() {
                    return f(s.value.get());
                }
                None
            });
        }
        Ok(NumberField {
            control,
            state: s,
            on_change,
            on_commit,
        })
    }
    /// Maps a value change to the app's message (the new value).
    pub fn on_change(self, mapper: impl Fn(f64) -> Option<M> + 'static) -> NumberField<M> {
        *self.on_change.borrow_mut() = Some(Box::new(mapper));
        self
    }
    /// Maps a committed value to the app's message (on Enter, blur or a step).
    pub fn on_commit(self, mapper: impl Fn(f64) -> Option<M> + 'static) -> NumberField<M> {
        *self.on_commit.borrow_mut() = Some(Box::new(mapper));
        self
    }
    /// The field's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }
    /// The current value.
    pub fn value(&self) -> f64 {
        self.state.value.get()
    }
    /// Sets the value, clamped to the range, without raising an event.
    pub fn set_value(&self, value: f64) {
        let value = self.state.clamp(value);
        self.state.value.set(value);
        show_value(value, &self.state.text, &self.state.caret);
        self.control.invalidate();
    }
    /// Gives the field the keyboard focus.
    pub fn focus(&self) {
        self.control.focus();
    }
    /// Enables or disables the field. A disabled field is dimmed and ignores input.
    pub fn set_enabled(&self, enabled: bool) {
        self.state.enabled.set(enabled);
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }
    /// Marks the field selected (a form editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
    fn set_range(&self, min: f64, max: f64) {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.state.min.set(min);
        self.state.max.set(max);
        self.set_value(self.state.value.get());
    }
}
impl<M: 'static> Properties for NumberField<M> {
    fn properties(&self) -> Vec<Property> {
        let s = &self.state;
        [
            ("value", self.value()),
            ("min", s.min.get()),
            ("max", s.max.get()),
        ]
        .into_iter()
        .map(|(name, value)| Property {
            name,
            value: Value::Float(value),
        })
        .collect()
    }
    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("value", Value::Float(v)) => self.set_value(v),
            ("min", Value::Float(v)) => self.set_range(v, self.state.max.get()),
            ("max", Value::Float(v)) => self.set_range(self.state.min.get(), v),
            _ => return false,
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::NumberField;
    use crate::app::{Core, Ui};
    use crate::backend::headless::HeadlessBackend;
    use crate::backend::{Backend, Event, PlatformSpec};
    use crate::geometry::Rect;
    use crate::message::{Key, Modifiers, MouseButton};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn harness() -> (Rc<Core<()>>, Ui<()>) {
        let backend = Rc::new(HeadlessBackend::new());
        let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
        let core = Core::new(backend, window);
        (Rc::clone(&core), Ui::new(Rc::clone(&core)))
    }

    fn number(ui: &Ui<()>, max: f64) -> NumberField<()> {
        NumberField::new(ui, Rect::new(0, 0, 160, 28), 0.0, max, 5.0).unwrap()
    }
    fn record(log: &Rc<RefCell<Vec<f64>>>, value: f64) -> Option<()> {
        log.borrow_mut().push(value);
        None
    }
    fn key(k: Key) -> Event {
        Event::KeyDown {
            key: k,
            modifiers: Modifiers::NONE,
            repeat: 1,
            system: false,
        }
    }
    fn down(x: i32, y: i32) -> Event {
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn clicking_the_plus_stepper_commits_the_next_value() {
        let (core, ui) = harness();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&seen);
        let field = number(&ui, 100.0).on_commit(move |v| record(&log, v));
        core.router().dispatch(field.id(), &down(150, 5));
        assert_eq!((field.value(), seen.borrow().clone()), (5.0, vec![5.0]));
    }

    #[test]
    fn typing_then_return_commits_and_clamps_to_max() {
        let (core, ui) = harness();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&seen);
        let field = number(&ui, 50.0).on_commit(move |v| record(&log, v));
        core.router().dispatch(field.id(), &Event::SetFocus);
        for c in ['9', '9'] {
            core.router().dispatch(field.id(), &Event::Char(c));
        }
        core.router().dispatch(field.id(), &key(Key::RETURN));
        assert_eq!((field.value(), seen.borrow().clone()), (50.0, vec![50.0]));
    }

    #[test]
    fn set_value_raises_nothing() {
        let (_core, ui) = harness();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (a, b) = (Rc::clone(&seen), Rc::clone(&seen));
        let field = number(&ui, 100.0)
            .on_change(move |v| record(&a, v))
            .on_commit(move |v| record(&b, v));
        field.set_value(42.0);
        assert_eq!(field.value(), 42.0);
        assert!(seen.borrow().is_empty(), "set_value raised nothing");
    }
}
