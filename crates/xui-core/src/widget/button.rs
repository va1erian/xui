#![forbid(unsafe_code)]

//! [`Button`]: a painted push button that maps a click to the app's message.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::{Control, HasText};
use crate::app::Ui;
use crate::backend::{Event, NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::Rect;
use crate::icon::{IconRef, draw_icon};
use crate::message::{Key, MouseButton};
use crate::property::{Properties, Property, Value};
use crate::units::Dip;

/// Maps a click to an optional app message.
type ClickMapper<M> = Rc<RefCell<Option<Box<dyn Fn() -> Option<M>>>>>;

/// The design size of the button text.
const TEXT_SIZE: Dip = Dip(12.0);
/// The corner radius of the button face.
const RADIUS: f32 = 4.0;
/// The gap between an icon and its label, in device pixels.
const GAP: i32 = 6;
/// The largest icon side, in device pixels.
const ICON_MAX: i32 = 20;

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
    icon: Rc<Cell<Option<IconRef>>>,
    on_click: ClickMapper<M>,
}

impl<M: 'static> Button<M> {
    /// Creates a button with no bounds of its own, for a layout to place (see
    /// [`crate::arrange`]); its size comes from [`Placeable`](super::Placeable).
    pub fn auto(ui: &Ui<M>, text: &str) -> Result<Button<M>> {
        Button::new(ui, Rect::default(), text)
    }

    /// Creates a button labelled `text` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, text: &str) -> Result<Button<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Button, bounds).text(text))?;
        let state = Rc::new(Cell::new(ButtonState::Normal));
        let label = Rc::new(RefCell::new(text.to_string()));
        let icon = Rc::new(Cell::new(None));
        let on_click: ClickMapper<M> = Rc::new(RefCell::new(None));

        {
            let state = Rc::clone(&state);
            let label = Rc::clone(&label);
            let icon = Rc::clone(&icon);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
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
                let dpi = canvas.dpi();
                canvas.fill_rounded_rect(bounds, RADIUS, fill);
                canvas.stroke_rounded_rect(bounds, RADIUS, theme.border, 1.0);
                let color = if state == ButtonState::Disabled {
                    theme.text_disabled
                } else {
                    theme.text
                };
                let text = label.borrow();
                let (icon_rect, text_rect) =
                    layout_content(bounds, icon.get().is_some(), text.is_empty());
                if let Some(icon_rect) = icon_rect
                    && let Some(icon) = icon.get()
                {
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
            let state = Rc::clone(&state);
            let on_click = Rc::clone(&on_click);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                // In design mode the editor handles input, not the widget.
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                let mut activate = false;
                match event {
                    Event::MouseMove { .. } => {
                        // Do not disturb an active press: a pointer that
                        // jitters between press and release must still click.
                        // Only the Normal -> Hover edge changes the face, so
                        // any other move repaints nothing.
                        if state.get() != ButtonState::Normal {
                            return None;
                        }
                        state.set(ButtonState::Hover);
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
            icon,
            on_click,
        })
    }

    /// Draws `icon` before the label (or centred when there is no label).
    ///
    /// Any [`IconRef`] works: a generated [`Lucide`](crate::icon::Lucide) icon,
    /// the legacy [`Icon`](super::Icon) set or a [`Glyph`](super::Glyph).
    pub fn icon(self, icon: impl Into<IconRef>) -> Button<M> {
        self.set_icon(Some(icon));
        self
    }

    /// Replaces the leading icon, or removes it with `None`.
    ///
    /// Accepts the same [`IconRef`] inputs as [`Button::icon`]; use
    /// [`Button::clear_icon`] to remove the icon without a type annotation on
    /// `None`.
    pub fn set_icon(&self, icon: Option<impl Into<IconRef>>) {
        self.icon.set(icon.map(Into::into));
        self.control.invalidate();
    }

    /// Removes the button's leading icon.
    pub fn clear_icon(&self) {
        self.icon.set(None);
        self.control.invalidate();
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

    /// Marks the button selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }

    /// Whether the button is selected.
    pub fn is_selected(&self) -> bool {
        self.control.is_selected()
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

/// The icon and text rectangles inside a button face.
///
/// With no icon the label keeps the whole face; with an icon and a label the
/// icon sits at the leading edge and the label is centred in what remains; with
/// an icon and no label the icon is centred.
fn layout_content(bounds: Rect, has_icon: bool, text_empty: bool) -> (Option<Rect>, Rect) {
    if !has_icon {
        return (None, bounds);
    }
    let side = (bounds.height() * 2 / 3).clamp(8, ICON_MAX);
    if text_empty {
        let left = bounds.left + (bounds.width() - side) / 2;
        let top = bounds.top + (bounds.height() - side) / 2;
        return (Some(Rect::new(left, top, left + side, top + side)), bounds);
    }
    let top = bounds.top + (bounds.height() - side) / 2;
    let icon_rect = Rect::new(bounds.left + GAP, top, bounds.left + GAP + side, top + side);
    let text_rect = Rect::new(
        icon_rect.right + GAP,
        bounds.top,
        bounds.right,
        bounds.bottom,
    );
    (Some(icon_rect), text_rect)
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
