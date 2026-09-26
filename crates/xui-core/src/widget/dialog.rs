#![forbid(unsafe_code)]

//! [`Dialog`]: a portable, modal in-window dialog.
//!
//! A dialog is a full-window scrim with a centred card, raised above the rest
//! of the window, so the widgets behind it cannot be reached while it is open.
//! It comes in three shapes: a message (one **OK** button), a confirm
//! (**OK**/**Cancel**) and a prompt (a confirm with an [`Edit`]).
//!
//! The scrim is a painted node, built like the tooltip popup: it is shown with
//! `set_visible`, moved with `apply_moves` and put on top with `raise`. The
//! chosen action reaches the app through [`Dialog::on_action`]; as with every
//! widget, the mapper returns the app's `Msg` and `App::update` is never
//! re-entered.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use self::events::{accept_action, cancel_action, on_key};
use self::layout::{Layout, place};
use super::button::Button;
use super::control::{Control, HasText};
use super::edit::Edit;
use crate::Color;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::geometry::Rect;
use crate::theme::Theme;
use crate::units::Dip;

mod events;
mod layout;

/// Maps the dialog's action to an optional app message.
type ActionMapper<M> = Rc<RefCell<Option<Box<dyn Fn(DialogAction) -> Option<M>>>>>;

/// The action a dialog was dismissed with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogAction {
    /// The affirmative button, Enter, or the Escape key when it accepts. For a
    /// [`Dialog::prompt`] it carries the entered text; empty otherwise.
    Accept(String),
    /// The cancel button, Escape, or a programmatic [`Dialog::close`].
    Cancel,
}

/// The shape of a dialog.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Message,
    Confirm,
    Prompt,
}

/// Padding between the card edge and its content.
const PADDING: Dip = Dip(16.0);
/// Vertical gap between the card's blocks.
const GAP: Dip = Dip(12.0);
/// The smallest and largest card widths.
const MIN_WIDTH: Dip = Dip(260.0);
const MAX_WIDTH: Dip = Dip(460.0);
/// The button size.
const BUTTON_WIDTH: Dip = Dip(88.0);
const BUTTON_HEIGHT: Dip = Dip(28.0);
/// The prompt field's height.
const FIELD_HEIGHT: Dip = Dip(28.0);
/// The margin kept between the card and the window edges.
const MARGIN: Dip = Dip(24.0);
/// The title and message text sizes.
const TITLE_SIZE: Dip = Dip(15.0);
const MESSAGE_SIZE: Dip = Dip(12.0);
/// The card's corner radius, in pixels.
const RADIUS: f32 = 8.0;

/// State the painter, the key listeners and the buttons share.
struct Shared<M: 'static> {
    ui: Ui<M>,
    /// Every node the dialog owns (the scrim and its children), shown and
    /// hidden together.
    nodes: RefCell<Vec<WidgetId>>,
    title: RefCell<String>,
    message: RefCell<String>,
    /// The prompt field's current text.
    field: RefCell<String>,
    action: ActionMapper<M>,
    open: Cell<bool>,
}

/// A modal dialog drawn in the window.
pub struct Dialog<M: 'static> {
    shared: Rc<Shared<M>>,
    layout: Rc<Cell<Layout>>,
    scrim: Control<M>,
    buttons: Vec<Button<M>>,
    edit: Option<Edit<M>>,
    accept: usize,
    cancel: Option<usize>,
}

impl<M: 'static> Dialog<M> {
    /// A message dialog: a title, a message and one **OK** button.
    pub fn message(ui: &Ui<M>, title: &str, message: &str) -> Result<Dialog<M>> {
        Dialog::build(ui, title, message, Shape::Message, "")
    }

    /// A confirm dialog: a title, a message and **OK**/**Cancel** buttons.
    pub fn confirm(ui: &Ui<M>, title: &str, message: &str) -> Result<Dialog<M>> {
        Dialog::build(ui, title, message, Shape::Confirm, "")
    }

    /// A prompt dialog: a confirm with an [`Edit`] seeded with `initial`.
    pub fn prompt(ui: &Ui<M>, title: &str, message: &str, initial: &str) -> Result<Dialog<M>> {
        Dialog::build(ui, title, message, Shape::Prompt, initial)
    }

    fn build(
        ui: &Ui<M>,
        title: &str,
        message: &str,
        shape: Shape,
        initial: &str,
    ) -> Result<Dialog<M>> {
        let shared = Rc::new(Shared {
            ui: ui.clone(),
            nodes: RefCell::new(Vec::new()),
            title: RefCell::new(title.to_string()),
            message: RefCell::new(message.to_string()),
            field: RefCell::new(initial.to_string()),
            action: Rc::new(RefCell::new(None)),
            open: Cell::new(false),
        });
        let scrim = Control::new(ui, &NodeSpec::new(NodeKind::Custom, Rect::default()))?;
        let scrim_id = scrim.id();
        shared.nodes.borrow_mut().push(scrim_id);

        let layout = Rc::new(Cell::new(Layout::default()));
        {
            let shared = Rc::clone(&shared);
            let layout = Rc::clone(&layout);
            let theme = ui.theme_handle();
            let selected = scrim.selected_handle();
            scrim.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let layout = layout.get();
                canvas.clear(scrim_color(theme));
                if !layout.visible {
                    return;
                }
                canvas.fill_rounded_rect(layout.card, RADIUS, theme.raised);
                canvas.stroke_rounded_rect(layout.card, RADIUS, theme.border, 1.0);
                let title = shared.title.borrow();
                let style = TextStyle::new(theme.text, TITLE_SIZE).bold();
                canvas.draw_text(&title, layout.title, &style);
                let message = shared.message.borrow();
                let style = TextStyle::new(theme.text_secondary, MESSAGE_SIZE).wrapped();
                canvas.draw_text(&message, layout.message, &style);
                if selected.get() {
                    canvas.stroke_rect(layout.card, theme.accent, 2.0);
                }
            }));
        }

        {
            let shared = Rc::clone(&shared);
            let scrim_ui = ui.clone();
            scrim.on_events(move |event| {
                if scrim_ui.is_design_mode() && event.is_input() {
                    return None;
                }
                on_key(&shared, event)
            });
        }

        let child_ui = ui.clone();
        let mut buttons = Vec::new();
        let (accept, cancel) = match shape {
            Shape::Message => {
                let shared = Rc::clone(&shared);
                buttons.push(
                    Button::new(&child_ui, Rect::default(), "OK")?
                        .on_click(move || accept_action(&shared)),
                );
                (0, None)
            }
            Shape::Confirm | Shape::Prompt => {
                let cancel_shared = Rc::clone(&shared);
                buttons.push(
                    Button::new(&child_ui, Rect::default(), "Cancel")?
                        .on_click(move || cancel_action(&cancel_shared)),
                );
                let accept_shared = Rc::clone(&shared);
                buttons.push(
                    Button::new(&child_ui, Rect::default(), "OK")?
                        .on_click(move || accept_action(&accept_shared)),
                );
                (1, Some(0))
            }
        };

        let edit = if shape == Shape::Prompt {
            let shared = Rc::clone(&shared);
            Some(
                Edit::new(&child_ui, Rect::default(), initial)?.on_change(move |text| {
                    *shared.field.borrow_mut() = text.to_string();
                    None
                }),
            )
        } else {
            None
        };

        {
            let mut nodes = shared.nodes.borrow_mut();
            for button in &buttons {
                nodes.push(button.id());
            }
            if let Some(edit) = &edit {
                nodes.push(edit.id());
            }
            for id in nodes.iter() {
                ui.set_visible(*id, false);
            }
        }
        for id in shared.nodes.borrow().iter() {
            let shared = Rc::clone(&shared);
            // The scrim and its controls are siblings, not a parent and children.
            // Joining a painted child window to the scrim stops the Win32 backend
            // painting the scrim (its whole area is covered by the child), so the
            // dialog keeps one flat z-order instead.
            let child_ui = ui.clone();
            ui.add_events(*id, move |event| {
                if child_ui.is_design_mode() && event.is_input() {
                    return None;
                }
                on_key(&shared, event)
            });
        }

        Ok(Dialog {
            shared,
            layout,
            scrim,
            buttons,
            edit,
            accept,
            cancel,
        })
    }

    /// Maps a dismissal to the app's message: the closure returns `Some(msg)`
    /// to raise it, or `None` to ignore the action.
    pub fn on_action(self, mapper: impl Fn(DialogAction) -> Option<M> + 'static) -> Dialog<M> {
        *self.shared.action.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Replaces the affirmative button's label.
    pub fn accept_label(self, label: &str) -> Dialog<M> {
        if let Some(button) = self.buttons.get(self.accept) {
            button.set_text(label);
        }
        self
    }

    /// Replaces the cancel button's label.
    pub fn cancel_label(self, label: &str) -> Dialog<M> {
        if let Some(index) = self.cancel
            && let Some(button) = self.buttons.get(index)
        {
            button.set_text(label);
        }
        self
    }

    /// Opens the dialog: centre the card, show it above the rest of the window
    /// and give the field (a prompt) or the affirmative button the focus.
    pub fn open(&self) {
        let ui = &self.shared.ui;
        let title = self.shared.title.borrow().clone();
        let message = self.shared.message.borrow().clone();
        let buttons: Vec<WidgetId> = self.buttons.iter().map(Button::id).collect();
        let field = self.edit.as_ref().map(Edit::id);
        let placement = place(ui, self.scrim.id(), &buttons, field, &title, &message);

        ui.apply_moves(&placement.moves);
        self.layout.set(placement.layout);

        for id in self.shared.nodes.borrow().iter() {
            ui.set_visible(*id, true);
        }
        ui.raise(self.scrim.id());
        for button in &self.buttons {
            ui.raise(button.id());
        }
        if let Some(edit) = &self.edit {
            ui.raise(edit.id());
            edit.focus();
        } else if let Some(button) = self.buttons.get(self.accept) {
            ui.focus(button.id());
        }
        self.shared.open.set(true);
        ui.invalidate(self.scrim.id());
    }

    /// Closes the dialog without raising an action.
    pub fn close(&self) {
        if self.shared.open.replace(false) {
            self.hide();
        }
    }

    /// Whether the dialog is currently open.
    pub fn is_open(&self) -> bool {
        self.shared.open.get()
    }

    /// The scrim's node identity.
    pub fn id(&self) -> WidgetId {
        self.scrim.id()
    }

    /// The dialog's title.
    pub fn title(&self) -> String {
        self.shared.title.borrow().clone()
    }

    /// Replaces the title; a visible dialog resizes to fit.
    pub fn set_title(&self, title: &str) {
        *self.shared.title.borrow_mut() = title.to_string();
        if self.shared.open.get() {
            self.open();
        }
    }

    /// The prompt field's current text (empty for a message/confirm).
    pub fn text(&self) -> String {
        self.shared.field.borrow().clone()
    }

    fn hide(&self) {
        let mut layout = self.layout.get();
        layout.visible = false;
        self.layout.set(layout);
        for id in self.shared.nodes.borrow().iter() {
            self.shared.ui.set_visible(*id, false);
        }
        self.shared.ui.invalidate(self.scrim.id());
    }
}

/// The scrim colour: the window background darkened. It is opaque because a
/// painted child window does not blend with the widgets behind it on every
/// backend, so a translucent fill would show uninitialised pixels.
fn scrim_color(theme: Theme) -> Color {
    theme.background.lerp(Color::rgb(0, 0, 0), 0.35)
}

#[cfg(test)]
mod tests;
