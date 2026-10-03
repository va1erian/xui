#![forbid(unsafe_code)]

//! [`Edit`]: a single-line text field with CUA keyboard and selection.
//!
//! The text, caret and selection live in a pure [`model`], so every backend
//! edits the same way. A form designer draws one to capture a property; a
//! normal app maps each change to its `Msg` through [`Edit::on_change`].
//!
//! A backend that hosts a native control ([`ImplKind::Native`], as the Win32
//! backend does for a real `EDIT`) draws and edits the field itself, including
//! selection, clipboard and undo; the widget then draws nothing and syncs its
//! text from the control's change events. Otherwise
//! ([`Painted`](ImplKind::Painted)) the widget paints the field and handles its
//! own input through [`keys`]. [`Edit::password`] masks it for a secret.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{Cursor, Event, ImplKind, NodeKind, NodeSpec, Result, TextStyle};
use crate::color::Color;
use crate::geometry::Rect;
use crate::message::MouseButton;
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

mod geometry;
mod history;
mod keys;
mod mask;
mod model;
mod paint;
mod word;

use keys::{Clipboard, KeyResult};
use model::EditModel;

/// Maps new text to an optional app message.
type ChangeMapper<M> = Rc<RefCell<Option<Box<dyn Fn(&str) -> Option<M>>>>>;

/// The design size of the field text.
const TEXT_SIZE: Dip = Dip(12.0);
/// Padding inside the field border, as a design value.
const PADDING: Dip = Dip(4.0);

/// A single-line text field.
pub struct Edit<M: 'static> {
    control: Control<M>,
    model: Rc<RefCell<EditModel>>,
    /// The cue banner drawn while the text is empty (a native control shows
    /// its own).
    cue: Rc<RefCell<String>>,
    /// The horizontal scroll offset in pixels, so the caret stays visible.
    scroll: Rc<Cell<i32>>,
    /// Whether the text is masked, as a password field's is.
    masked: Rc<Cell<bool>>,
    /// Whether the backend hosts a native control that edits itself.
    native: bool,
    /// Set while the widget itself sets the native text, so the resulting
    /// change notification does not loop back through `on_change`.
    setting: Rc<Cell<bool>>,
    on_change: ChangeMapper<M>,
}

/// The [`keys::Clipboard`] over a window's backend.
struct UiClipboard<'a, M>(&'a Ui<M>);

impl<M: 'static> Clipboard for UiClipboard<'_, M> {
    fn text(&self) -> Option<String> {
        self.0.clipboard_text()
    }

    fn set(&self, text: &str) {
        self.0.set_clipboard_text(text);
    }
}

impl<M: 'static> Edit<M> {
    /// Creates an edit with no bounds of its own, for a layout to place (see
    /// [`crate::arrange`]); its size comes from [`Placeable`](super::Placeable).
    pub fn auto(ui: &Ui<M>, text: &str) -> Result<Edit<M>> {
        Edit::new(ui, Rect::default(), text)
    }

    /// Creates a field showing `text` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<Edit<M>> {
        let native = ui.supports(NodeKind::Edit) == ImplKind::Native;
        let control = Control::new(
            ui,
            &NodeSpec::new(NodeKind::Edit, bounds).text(text).tab_stop(),
        )?;
        ui.set_cursor(control.id(), Cursor::Text);
        let model = Rc::new(RefCell::new(EditModel::new(text)));
        let cue = Rc::new(RefCell::new(String::new()));
        let scroll = Rc::new(Cell::new(0));
        let focused = Rc::new(Cell::new(false));
        let setting = Rc::new(Cell::new(false));
        let masked = Rc::new(Cell::new(false));
        let on_change: ChangeMapper<M> = Rc::new(RefCell::new(None));

        if !native {
            let model = Rc::clone(&model);
            let cue = Rc::clone(&cue);
            let scroll = Rc::clone(&scroll);
            let focused = Rc::clone(&focused);
            let masked = Rc::clone(&masked);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let state = paint::PaintState {
                    model: model.as_ref(),
                    cue: cue.as_ref(),
                    scroll: scroll.as_ref(),
                    focused: focused.as_ref(),
                    selected: selected.as_ref(),
                    masked: masked.as_ref(),
                };
                paint::paint(canvas, &theme.get(), &state);
            }));
        }

        {
            let model = Rc::clone(&model);
            let focused = Rc::clone(&focused);
            let scroll = Rc::clone(&scroll);
            let setting = Rc::clone(&setting);
            let masked = Rc::clone(&masked);
            let on_change = Rc::clone(&on_change);
            let dragging = Rc::new(Cell::new(false));
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                // In design mode the editor handles input, not the field; a
                // native control's change notification still syncs the text.
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                let dpi = ui.dpi();
                let style = TextStyle::new(Color::rgb(0, 0, 0), TEXT_SIZE).middle();
                // Maps a node-local x to the nearest caret position in what is
                // painted. The model is borrowed only for the measurement,
                // never across a call that can deliver an event.
                let char_at = |x: i32| -> usize {
                    let target = geometry::pointer_text_x(x, dpi, scroll.get());
                    let model = model.borrow();
                    let value = mask::display(model.text(), masked.get());
                    let total = value.chars().count();
                    let mut best = 0;
                    let mut best_distance = i32::MAX;
                    let mut byte = 0;
                    for chars in 0..=total {
                        let width = ui.measure_text(&value[..byte], &style, dpi).width;
                        let distance = (width - target).abs();
                        if distance < best_distance {
                            best_distance = distance;
                            best = chars;
                        }
                        if chars < total {
                            byte += value[byte..].chars().next().map_or(0, char::len_utf8);
                        }
                    }
                    best
                };

                let mut changed = false;
                let mut redraw = false;
                match event {
                    Event::SetFocus => focused.set(true),
                    Event::KillFocus => focused.set(false),
                    Event::MouseDown {
                        x,
                        button: MouseButton::Left,
                        modifiers,
                        ..
                    } => {
                        focused.set(true);
                        ui.focus(id);
                        if !native {
                            let pos = char_at(*x);
                            let mut model = model.borrow_mut();
                            model.move_to(pos, modifiers.shift);
                            drop(model);
                            dragging.set(true);
                            ui.set_capture(id);
                        }
                        redraw = true;
                    }
                    Event::MouseDoubleClick {
                        x,
                        button: MouseButton::Left,
                        ..
                    } if !native => {
                        let pos = char_at(*x);
                        // A run would reveal where a masked field's spaces are.
                        if masked.get() {
                            model.borrow_mut().select_all();
                        } else {
                            model.borrow_mut().select_word_at(pos);
                        }
                        dragging.set(false);
                        ui.release_capture();
                        redraw = true;
                    }
                    Event::MouseMove { x, .. } if !native && dragging.get() => {
                        let pos = char_at(*x);
                        model.borrow_mut().move_to(pos, true);
                        redraw = true;
                    }
                    Event::MouseUp {
                        button: MouseButton::Left,
                        ..
                    } if !native => {
                        dragging.set(false);
                        ui.release_capture();
                    }
                    Event::MouseLeave if !native && dragging.get() => {
                        dragging.set(false);
                        ui.release_capture();
                    }
                    // A native control edits itself and reports the change; read
                    // the new text back and raise it.
                    Event::TextChanged if native => {
                        let value = ui.text(id);
                        model.borrow_mut().set_text(&value);
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
                    Event::Char(character)
                        if !native && focused.get() && !character.is_control() =>
                    {
                        model.borrow_mut().insert_char(*character);
                        changed = true;
                        redraw = true;
                    }
                    Event::KeyDown {
                        key,
                        modifiers,
                        system,
                        ..
                    } if !native && focused.get() && !*system => {
                        let clipboard = UiClipboard(&ui);
                        let result = {
                            let mut model = model.borrow_mut();
                            keys::apply(&mut model, *key, *modifiers, &clipboard, masked.get())
                        };
                        match result {
                            KeyResult::Ignored => return None,
                            KeyResult::Redraw => redraw = true,
                            KeyResult::Changed => {
                                changed = true;
                                redraw = true;
                            }
                        }
                    }
                    _ => return None,
                }
                if redraw {
                    ui.invalidate(id);
                }
                if changed {
                    let value = model.borrow().text().to_string();
                    let mapper = on_change.borrow();
                    if let Some(mapper) = mapper.as_ref() {
                        return mapper(&value);
                    }
                }
                None
            });
        }

        Ok(Edit {
            control,
            model,
            cue,
            scroll,
            masked,
            native,
            setting,
            on_change,
        })
    }

    /// Shows `cue` as a placeholder while the field is empty (a native cue
    /// banner where the backend hosts one). Chainable.
    pub fn cue(self, cue: &str) -> Edit<M> {
        *self.cue.borrow_mut() = cue.to_string();
        if self.native {
            self.control.ui().set_cue(self.control.id(), cue);
        } else {
            self.control.invalidate();
        }
        self
    }

    /// Masks the field for a secret such as a password (`true`), or shows its
    /// text again (`false`). Chainable.
    ///
    /// It shows one bullet (U+2022) per character, caret and clicks measured
    /// on the bullets; refuses copy and cut (paste and undo work); is one word
    /// to Ctrl+arrows, Ctrl+Backspace/Delete and double-click; and reports its
    /// `text` property as bullets. The cue still shows while it is empty, and
    /// [`HasText::text`] and [`on_change`](Edit::on_change) still deliver the
    /// real text. A native Win32 `EDIT` switches to its own password style,
    /// which masks and refuses copy and cut the same way.
    pub fn password(self, password: bool) -> Edit<M> {
        self.masked.set(password);
        if self.native {
            self.control.ui().set_password(self.control.id(), password);
        } else {
            self.control.invalidate();
        }
        self
    }

    /// Whether the field masks its text (see [`Edit::password`]).
    pub fn is_password(&self) -> bool {
        self.masked.get()
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

    /// Marks the field selected, so its painter draws an outline (a form
    /// editor's selection). A native field draws no outline.
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }

    /// Whether the field is selected.
    pub fn is_selected(&self) -> bool {
        self.control.is_selected()
    }
}

impl<M: 'static> HasText for Edit<M> {
    fn text(&self) -> String {
        self.model.borrow().text().to_string()
    }

    fn set_text(&self, text: &str) {
        self.model.borrow_mut().set_text(text);
        if self.native {
            // Push to the control; its change notification is suppressed so
            // `on_change` fires only for real edits.
            self.setting.set(true);
            self.control.set_text(text);
            self.setting.set(false);
        } else {
            self.scroll.set(0);
            self.control.invalidate();
        }
    }
}

impl<M: 'static> Properties for Edit<M> {
    fn properties(&self) -> Vec<Property> {
        // A masked field never hands its secret to a form file or an
        // automation client; it reports what it shows.
        let text = self.text();
        let value = mask::display(&text, self.is_password()).into_owned();
        vec![Property {
            name: "text",
            value: Value::Text(value),
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
