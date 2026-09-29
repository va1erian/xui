#![forbid(unsafe_code)]

//! [`FileDialog`]: a portable, modal file open/save picker.
//!
//! It is the file-dialog sibling of [`Dialog`](super::Dialog): the same
//! full-window scrim with a centred card, holding a path bar, an entry list, a
//! filename field, an optional extension filter, an inline error row and
//! accept/cancel buttons. It reads directories through a [`FileSystem`], never
//! writes, and returns a [`PathBuf`] to the app through the closure given at
//! construction.
//!
//! Before showing its own card, [`FileDialog::open`] asks the backend for a
//! native picker ([`Backend::file_dialog`](crate::backend::Backend::file_dialog));
//! a backend that declines (the default, the canvas and headless backends) gets
//! the portable card instead, and the app's closures are identical either way.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use self::events::{Role, on_key};
use self::layout::{Layout, Parts, RADIUS, TITLE_SIZE};
use self::shared::{Fields, Settings, Shared};
use super::button::Button;
use super::control::Control;
use super::edit::Edit;
use super::label::Label;
use super::listview::ListView;
use crate::app::Ui;
use crate::backend::{
    FileDialogMode, FileDialogOutcome, FileDialogRequest, FileFilter, NodeKind, NodeSpec, Result,
    TextStyle, WidgetId,
};
use crate::geometry::Rect;

mod events;
mod fs;
mod layout;
mod shared;
mod state;

#[cfg(test)]
mod tests;

pub use fs::{Entry, FileSystem, StdFileSystem};

/// A modal file open/save picker drawn in the window.
///
/// Build one with [`FileDialog::open_file`] or [`FileDialog::save_file`],
/// configure it with the builder methods, install the result closures with
/// [`FileDialog::on_accept`]/[`FileDialog::on_cancel`] and show it with
/// [`FileDialog::open`].
pub struct FileDialog<M: 'static> {
    shared: Rc<Shared<M>>,
    layout: Rc<Cell<Layout>>,
    scrim: Control<M>,
    path: Rc<Edit<M>>,
    list: Rc<ListView<M>>,
    name: Rc<Edit<M>>,
    filter: Rc<Button<M>>,
    error: Rc<Label<M>>,
    accept: Rc<Button<M>>,
    cancel: Rc<Button<M>>,
}

impl<M: 'static> FileDialog<M> {
    /// An open dialog: choose an existing file by default (see
    /// [`require_existing`](FileDialog::require_existing)).
    pub fn open_file(ui: &Ui<M>, title: &str) -> Result<FileDialog<M>> {
        FileDialog::build(ui, title, FileDialogMode::Open)
    }

    /// A save dialog: choose a path, confirming before overwriting an existing
    /// file.
    pub fn save_file(ui: &Ui<M>, title: &str) -> Result<FileDialog<M>> {
        FileDialog::build(ui, title, FileDialogMode::Save)
    }

    fn build(ui: &Ui<M>, title: &str, mode: FileDialogMode) -> Result<FileDialog<M>> {
        let settings = Settings {
            mode,
            initial_dir: None,
            suggested_name: String::new(),
            filters: Vec::new(),
            require_existing: mode == FileDialogMode::Open,
            fs: Rc::new(StdFileSystem),
        };
        let shared = Rc::new(Shared {
            ui: ui.clone(),
            nodes: RefCell::new(Vec::new()),
            state: RefCell::new(state::FileState::new(settings.to_config())),
            settings: RefCell::new(settings),
            title: RefCell::new(title.to_string()),
            on_accept: Rc::new(RefCell::new(None)),
            on_cancel: Rc::new(RefCell::new(None)),
            fields: RefCell::new(None),
            open: Cell::new(false),
        });

        let scrim = Control::new(ui, &NodeSpec::new(NodeKind::Custom, Rect::default()))?;
        let layout = Rc::new(Cell::new(Layout::default()));
        {
            let shared = Rc::clone(&shared);
            let layout = Rc::clone(&layout);
            let theme = ui.theme_handle();
            let selected = scrim.selected_handle();
            scrim.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let layout = layout.get();
                canvas.fill_rect_rgba(canvas.bounds(), theme.scrim);
                if !layout.visible {
                    return;
                }
                canvas.fill_rounded_rect(layout.card, RADIUS, theme.raised);
                canvas.stroke_rounded_rect(layout.card, RADIUS, theme.border, 1.0);
                let title = shared.title.borrow();
                let style = TextStyle::new(theme.text, TITLE_SIZE).bold();
                canvas.draw_text(&title, layout.title, &style);
                if selected.get() {
                    canvas.stroke_rect(layout.card, theme.accent, 2.0);
                }
            }));
        }

        let path = Rc::new(Edit::new(ui, Rect::default(), "")?.cue("Path").on_change({
            let shared = Rc::clone(&shared);
            move |text| {
                shared.state.borrow_mut().set_path_text(text);
                None
            }
        }));
        let list = Rc::new(
            ListView::new(ui, Rect::default(), &[])?
                .on_select({
                    let shared = Rc::clone(&shared);
                    move |row| {
                        shared.state.borrow_mut().select(row);
                        None
                    }
                })
                .on_activate({
                    let shared = Rc::clone(&shared);
                    move |_row| shared.do_accept()
                }),
        );
        let name = Rc::new(
            Edit::new(ui, Rect::default(), "")?
                .cue("File name")
                .on_change({
                    let shared = Rc::clone(&shared);
                    move |text| {
                        shared.state.borrow_mut().set_name(text);
                        shared.refresh_view();
                        None
                    }
                }),
        );
        let filter = Rc::new(Button::new(ui, Rect::default(), "")?.on_click({
            let shared = Rc::clone(&shared);
            move || {
                shared.state.borrow_mut().cycle_filter();
                shared.refresh_view();
                None
            }
        }));
        let error = Rc::new(Label::new(ui, Rect::default(), "")?);
        let accept = Rc::new(Button::new(ui, Rect::default(), "Open")?.on_click({
            let shared = Rc::clone(&shared);
            move || shared.do_accept()
        }));
        let cancel = Rc::new(Button::new(ui, Rect::default(), "Cancel")?.on_click({
            let shared = Rc::clone(&shared);
            move || shared.do_cancel()
        }));

        let ids = [
            (scrim.id(), Role::Scrim),
            (path.id(), Role::Path),
            (list.id(), Role::List),
            (name.id(), Role::Name),
            (filter.id(), Role::Button),
            (error.id(), Role::Scrim),
            (accept.id(), Role::Button),
            (cancel.id(), Role::Button),
        ];
        {
            let mut nodes = shared.nodes.borrow_mut();
            for (id, _) in ids {
                nodes.push(id);
                ui.set_visible(id, false);
            }
        }
        for (id, role) in ids {
            let shared = Rc::clone(&shared);
            ui.add_events(id, move |event| on_key(&shared, role, event));
        }

        let dialog = FileDialog {
            shared: Rc::clone(&shared),
            layout,
            scrim,
            path,
            list,
            name,
            filter,
            error,
            accept,
            cancel,
        };
        *shared.fields.borrow_mut() = Some(Fields {
            list: Rc::downgrade(&dialog.list),
            path: Rc::downgrade(&dialog.path),
            name: Rc::downgrade(&dialog.name),
            filter: Rc::downgrade(&dialog.filter),
            error: Rc::downgrade(&dialog.error),
            accept: Rc::downgrade(&dialog.accept),
        });
        Ok(dialog)
    }

    /// Sets the directory the dialog starts in (the caller's initial directory
    /// otherwise wins over the filesystem's home).
    pub fn initial_dir(self, dir: impl Into<PathBuf>) -> FileDialog<M> {
        self.shared.settings.borrow_mut().initial_dir = Some(dir.into());
        self
    }

    /// Sets the prefilled filename (save mode).
    pub fn suggested_name(self, name: &str) -> FileDialog<M> {
        self.set_suggested_name(name);
        self
    }

    /// Adds an extension filter; with more than one the card shows a filter
    /// selector. Repeatable.
    pub fn filter(self, label: &str, extensions: &[&str]) -> FileDialog<M> {
        self.shared
            .settings
            .borrow_mut()
            .filters
            .push(FileFilter::new(label, extensions));
        self
    }

    /// Whether an open result must already exist (default `true` for
    /// [`open_file`](FileDialog::open_file), `false` for
    /// [`save_file`](FileDialog::save_file)).
    pub fn require_existing(self, require: bool) -> FileDialog<M> {
        self.shared.settings.borrow_mut().require_existing = require;
        self
    }

    /// Replaces the filesystem the dialog reads through.
    pub fn file_system(self, fs: Rc<dyn FileSystem>) -> FileDialog<M> {
        self.shared.settings.borrow_mut().fs = fs;
        self
    }

    /// Maps an accepted path to the app's message.
    pub fn on_accept(self, mapper: impl Fn(PathBuf) -> Option<M> + 'static) -> FileDialog<M> {
        self.shared.on_accept.borrow_mut().replace(Box::new(mapper));
        self
    }

    /// Maps a cancellation to the app's message.
    pub fn on_cancel(self, mapper: impl Fn() -> Option<M> + 'static) -> FileDialog<M> {
        self.shared.on_cancel.borrow_mut().replace(Box::new(mapper));
        self
    }

    /// Replaces the prefilled filename at run time (its initial dir too with
    /// [`set_initial_dir`](FileDialog::set_initial_dir)).
    pub fn set_suggested_name(&self, name: &str) {
        self.shared.settings.borrow_mut().suggested_name = name.to_string();
    }

    /// Replaces the starting directory at run time.
    pub fn set_initial_dir(&self, dir: impl Into<PathBuf>) {
        self.shared.settings.borrow_mut().initial_dir = Some(dir.into());
    }

    /// Shows the dialog, asking the backend for a native picker first.
    pub fn open(&self) {
        let request = self.request();
        match self.shared.ui.file_dialog(&request) {
            FileDialogOutcome::Declined => {}
            FileDialogOutcome::Cancelled => {
                if let Some(message) = self.shared.deliver_cancel() {
                    self.shared.ui.emit(message);
                }
                return;
            }
            FileDialogOutcome::Chosen(path) => {
                if let Some(message) = self.shared.deliver_accept(path) {
                    self.shared.ui.emit(message);
                }
                return;
            }
        }
        let config = self.shared.settings.borrow().to_config();
        *self.shared.state.borrow_mut() = state::FileState::new(config);

        let filter = self
            .shared
            .settings
            .borrow()
            .filters
            .len()
            .gt(&1)
            .then(|| self.filter.id());
        let placement = self::layout::place(
            &self.shared.ui,
            &Parts {
                scrim: self.scrim.id(),
                path: self.path.id(),
                list: self.list.id(),
                name: self.name.id(),
                filter,
                error: self.error.id(),
                accept: self.accept.id(),
                cancel: self.cancel.id(),
            },
        );
        self.shared.ui.apply_moves(&placement.moves);
        self.layout.set(placement.layout);

        let nodes = self.shared.nodes.borrow();
        for id in nodes.iter() {
            self.shared.ui.set_visible(*id, true);
        }
        for id in nodes.iter() {
            self.shared.ui.raise(*id);
        }
        drop(nodes);
        self.shared.raise_list();

        self.shared.open.set(true);
        self.shared.sync_fields();
        self.shared.refresh_view();
        self.shared.focus_name();
        self.shared.ui.invalidate(self.scrim.id());
    }

    /// Closes the dialog without raising an action.
    pub fn close(&self) {
        if self.shared.open.get() {
            self.shared.hide();
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

    fn request(&self) -> FileDialogRequest {
        let settings = self.shared.settings.borrow();
        FileDialogRequest {
            mode: settings.mode,
            title: self.shared.title.borrow().clone(),
            initial_dir: settings.initial_dir.clone(),
            suggested_name: (!settings.suggested_name.is_empty())
                .then(|| settings.suggested_name.clone()),
            filters: settings.filters.clone(),
            require_existing: settings.require_existing,
        }
    }
}
