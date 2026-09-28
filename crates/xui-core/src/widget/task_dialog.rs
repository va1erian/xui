#![forbid(unsafe_code)]

//! [`TaskDialog`]: a portable, modal card with an icon, command buttons and an
//! optional verification checkbox.
//!
//! It is the richer sibling of [`Dialog`](super::Dialog): same full-window
//! scrim and centred card, but every dismissal is one of the app's own command
//! buttons, and the caller can add a checkbox whose state is read after the
//! action. Build one with [`TaskDialog::new`], add commands with
//! [`TaskDialog::command`], then [`TaskDialog::open`] it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use self::events::{dismiss, on_key};
use self::icon::draw_icon;
use self::layout::{Buttons, Layout, place};
use super::button::Button;
use super::checkbox::CheckBox;
use super::control::Control;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, TextStyle, WidgetId};
use crate::color::Color;
use crate::geometry::Rect;
use crate::theme::Theme;
use crate::units::Dip;

mod events;
mod icon;
mod layout;

pub use self::icon::TaskDialogIcon;

/// Maps the dialog's action to an optional app message.
type ActionMapper<M> = Rc<RefCell<Option<Box<dyn Fn(TaskDialogAction) -> Option<M>>>>>;

/// The action a [`TaskDialog`] was dismissed with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskDialogAction {
    /// A command button, by the index it was added with.
    Command(usize),
    /// The cancel button or Escape.
    Cancel,
}

/// Padding between the card edge and its content.
const PADDING: Dip = Dip(16.0);
/// Vertical gap between the card's blocks.
const GAP: Dip = Dip(12.0);
/// The smallest and largest card widths.
const MIN_WIDTH: Dip = Dip(300.0);
const MAX_WIDTH: Dip = Dip(520.0);
/// The command/cancel button size.
const BUTTON_WIDTH: Dip = Dip(92.0);
const BUTTON_HEIGHT: Dip = Dip(28.0);
/// The verification checkbox row's height.
const CHECK_HEIGHT: Dip = Dip(24.0);
/// The icon box's side.
const ICON: Dip = Dip(32.0);
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
    action: ActionMapper<M>,
    /// How many command buttons were added; Enter picks the first.
    command_count: Cell<usize>,
    open: Cell<bool>,
}

/// A modal card with an icon, command buttons and an optional checkbox.
pub struct TaskDialog<M: 'static> {
    shared: Rc<Shared<M>>,
    layout: Rc<Cell<Layout>>,
    icon: Rc<Cell<TaskDialogIcon>>,
    scrim: Control<M>,
    commands: RefCell<Vec<Button<M>>>,
    cancel: Button<M>,
    verification: Option<CheckBox<M>>,
}

impl<M: 'static> TaskDialog<M> {
    /// Creates a task dialog with no command buttons yet.
    pub fn new(ui: &Ui<M>, title: &str, message: &str) -> Result<TaskDialog<M>> {
        let shared = Rc::new(Shared {
            ui: ui.clone(),
            nodes: RefCell::new(Vec::new()),
            title: RefCell::new(title.to_string()),
            message: RefCell::new(message.to_string()),
            action: Rc::new(RefCell::new(None)),
            command_count: Cell::new(0),
            open: Cell::new(false),
        });
        let icon = Rc::new(Cell::new(TaskDialogIcon::None));
        let scrim = Control::new(ui, &NodeSpec::new(NodeKind::Custom, Rect::default()))?;
        let layout = Rc::new(Cell::new(Layout::default()));

        {
            let shared = Rc::clone(&shared);
            let layout = Rc::clone(&layout);
            let icon = Rc::clone(&icon);
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
                // A message taller than the card is clipped to it, so it never
                // spills over the command row or outside the client.
                canvas.push_clip(layout.card);
                if layout.icon_visible {
                    let dpi = canvas.dpi();
                    draw_icon(canvas, icon.get(), layout.icon, theme, dpi);
                }
                let title = shared.title.borrow();
                let style = TextStyle::new(theme.text, TITLE_SIZE).bold();
                canvas.draw_text(&title, layout.title, &style);
                let message = shared.message.borrow();
                let style = TextStyle::new(theme.text_secondary, MESSAGE_SIZE).wrapped();
                canvas.draw_text(&message, layout.message, &style);
                canvas.pop_clip();
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

        let cancel = {
            let shared = Rc::clone(&shared);
            Button::new(ui, Rect::default(), "Cancel")?
                .on_click(move || dismiss(&shared, TaskDialogAction::Cancel))
        };

        let dialog = TaskDialog {
            shared,
            layout,
            icon,
            scrim,
            commands: RefCell::new(Vec::new()),
            cancel,
            verification: None,
        };
        dialog.register(dialog.scrim.id());
        dialog.register(dialog.cancel.id());
        Ok(dialog)
    }

    /// Selects the icon drawn beside the title.
    pub fn icon(self, icon: TaskDialogIcon) -> TaskDialog<M> {
        self.icon.set(icon);
        self
    }

    /// Adds a command button; its position is the index an
    /// [`TaskDialogAction::Command`] reports.
    pub fn command(self, label: &str) -> Result<TaskDialog<M>> {
        let index = self.commands.borrow().len();
        let shared = Rc::clone(&self.shared);
        let button = Button::new(&self.shared.ui, Rect::default(), label)?
            .on_click(move || dismiss(&shared, TaskDialogAction::Command(index)));
        self.register(button.id());
        self.commands.borrow_mut().push(button);
        self.shared.command_count.set(index + 1);
        Ok(self)
    }

    /// Adds a verification checkbox below the message. Its state is read with
    /// [`TaskDialog::is_checked`] after the dialog closes.
    pub fn verification(mut self, label: &str) -> Result<TaskDialog<M>> {
        let check = CheckBox::new(&self.shared.ui, Rect::default(), label)?;
        self.register(check.id());
        self.verification = Some(check);
        Ok(self)
    }

    /// Maps a dismissal to the app's message: the closure returns `Some(msg)`
    /// to raise it, or `None` to ignore the action.
    pub fn on_action(
        self,
        mapper: impl Fn(TaskDialogAction) -> Option<M> + 'static,
    ) -> TaskDialog<M> {
        *self.shared.action.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The verification checkbox's state (`false` when there is none).
    pub fn is_checked(&self) -> bool {
        self.verification.as_ref().is_some_and(CheckBox::is_checked)
    }

    /// Sets the verification checkbox's state.
    pub fn set_checked(&self, checked: bool) {
        if let Some(check) = &self.verification {
            check.set_checked(checked);
        }
    }

    /// Opens the dialog: centre the card, show it above the rest of the window
    /// and focus the first command (or the cancel button).
    pub fn open(&self) {
        let ui = &self.shared.ui;
        let title = self.shared.title.borrow().clone();
        let message = self.shared.message.borrow().clone();
        let commands = self.commands.borrow();
        let command_ids: Vec<WidgetId> = commands.iter().map(Button::id).collect();
        let verification = self.verification.as_ref().map(CheckBox::id);
        let placement = place(
            ui,
            self.scrim.id(),
            Buttons {
                cancel: self.cancel.id(),
                commands: &command_ids,
            },
            verification,
            self.icon.get(),
            &title,
            &message,
        );

        ui.apply_moves(&placement.moves);
        self.layout.set(placement.layout);

        for id in self.shared.nodes.borrow().iter() {
            ui.set_visible(*id, true);
        }
        ui.raise(self.scrim.id());
        for button in commands.iter() {
            ui.raise(button.id());
        }
        ui.raise(self.cancel.id());
        if let Some(check) = &self.verification {
            ui.raise(check.id());
        }
        match commands.first() {
            Some(first) => ui.focus(first.id()),
            None => ui.focus(self.cancel.id()),
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

    /// Replaces the title; a visible dialog re-lays-out to fit.
    pub fn set_title(&self, title: &str) {
        *self.shared.title.borrow_mut() = title.to_string();
        if self.shared.open.get() {
            self.open();
        }
    }

    /// Records `id` as a dialog node: hidden until [`TaskDialog::open`], and
    /// listening for Escape/Enter alongside the scrim.
    fn register(&self, id: WidgetId) {
        let ui = &self.shared.ui;
        self.shared.nodes.borrow_mut().push(id);
        ui.set_visible(id, false);
        let shared = Rc::clone(&self.shared);
        let node_ui = ui.clone();
        ui.add_events(id, move |event| {
            if node_ui.is_design_mode() && event.is_input() {
                return None;
            }
            on_key(&shared, event)
        });
    }

    /// Hides every node the dialog owns.
    fn hide(&self) {
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
