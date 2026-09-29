#![forbid(unsafe_code)]

//! The state every part of a [`FileDialog`](super::FileDialog) shares: the
//! settings the builder collected, the open [`FileState`], the app's accept and
//! cancel mappers and weak handles to the widgets the view refreshes.
//!
//! The weak handles matter: the widgets' own event closures capture this
//! shared value, so a strong handle back to a widget would be a reference
//! cycle the widget's `Drop` could not break. A weak handle also
//! lets a refresh after the last strong reference is gone fall back to a no-op.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use super::super::button::Button;
use super::super::control::HasText;
use super::super::edit::Edit;
use super::super::label::Label;
use super::super::listview::{ListModel, ListView};
use super::fs::FileSystem;
use super::state::{Accept, Config, FileState};
use crate::app::Ui;
use crate::backend::{FileDialogMode, FileFilter, WidgetId};
use crate::icon::{IconRef, Lucide};

/// Maps an accepted path to an optional app message.
pub(super) type AcceptMapper<M> = Rc<RefCell<Option<Box<dyn Fn(PathBuf) -> Option<M>>>>>;
/// Maps a cancellation to an optional app message.
pub(super) type CancelMapper<M> = Rc<RefCell<Option<Box<dyn Fn() -> Option<M>>>>>;

/// The configuration the builder collects before the first open.
pub(super) struct Settings {
    pub(super) mode: FileDialogMode,
    pub(super) initial_dir: Option<PathBuf>,
    pub(super) suggested_name: String,
    pub(super) filters: Vec<FileFilter>,
    pub(super) require_existing: bool,
    pub(super) fs: Rc<dyn FileSystem>,
}

impl Settings {
    /// A fresh [`Config`], so each open gets its own [`FileState`].
    pub(super) fn to_config(&self) -> Config {
        Config {
            mode: self.mode,
            fs: Rc::clone(&self.fs),
            initial_dir: self.initial_dir.clone(),
            suggested_name: self.suggested_name.clone(),
            filters: self.filters.clone(),
            require_existing: self.require_existing,
        }
    }
}

/// Weak handles to the widgets a refresh updates.
pub(super) struct Fields<M: 'static> {
    pub(super) list: Weak<ListView<M>>,
    pub(super) path: Weak<Edit<M>>,
    pub(super) name: Weak<Edit<M>>,
    pub(super) filter: Weak<Button<M>>,
    pub(super) error: Weak<Label<M>>,
    pub(super) accept: Weak<Button<M>>,
}

/// The shared state behind one dialog.
pub(super) struct Shared<M: 'static> {
    pub(super) ui: Ui<M>,
    /// Every node the dialog owns, shown and hidden together.
    pub(super) nodes: RefCell<Vec<WidgetId>>,
    pub(super) settings: RefCell<Settings>,
    pub(super) state: RefCell<FileState>,
    pub(super) title: RefCell<String>,
    pub(super) on_accept: AcceptMapper<M>,
    pub(super) on_cancel: CancelMapper<M>,
    pub(super) fields: RefCell<Option<Fields<M>>>,
    pub(super) open: Cell<bool>,
}

/// Strong handles upgraded from the weak ones for the duration of a refresh.
struct View<M: 'static> {
    list: Rc<ListView<M>>,
    path: Rc<Edit<M>>,
    name: Rc<Edit<M>>,
    filter: Rc<Button<M>>,
    error: Rc<Label<M>>,
    accept: Rc<Button<M>>,
}

impl<M: 'static> Shared<M> {
    /// Raises the entry list with its scrollbar above the other card nodes.
    pub(super) fn raise_list(&self) {
        if let Some(view) = self.view() {
            view.list.raise();
        }
    }

    fn view(&self) -> Option<View<M>> {
        let fields = self.fields.borrow();
        let fields = fields.as_ref()?;
        Some(View {
            list: fields.list.upgrade()?,
            path: fields.path.upgrade()?,
            name: fields.name.upgrade()?,
            filter: fields.filter.upgrade()?,
            error: fields.error.upgrade()?,
            accept: fields.accept.upgrade()?,
        })
    }

    /// Repaints the view from the state: the list rows, the inline message, the
    /// filter selector and the accept button's label. It never writes the
    /// filename field, so a refresh triggered by typing keeps the caret.
    pub(super) fn refresh_view(&self) {
        let Some(view) = self.view() else {
            return;
        };
        let (labels, icons, selection, message, filter_label, show_filter, confirming, mode) = {
            let state = self.state.borrow();
            let message = state
                .overwrite_prompt()
                .or_else(|| Some(state.error()).filter(|error| !error.is_empty()))
                .unwrap_or_default();
            (
                state.row_labels(),
                state.row_icons(),
                state.selection(),
                message,
                state.filter_label(),
                state.has_filter_choice(),
                state.is_confirming_overwrite(),
                state.mode(),
            )
        };
        let has_rows = !labels.is_empty();
        view.list.set_model(EntryRows { labels, icons });
        if has_rows {
            view.list.select(Some(selection));
        }
        view.error.set_text(&message);
        self.ui.set_visible(view.error.id(), !message.is_empty());
        view.filter.set_text(&filter_label);
        self.ui.set_visible(view.filter.id(), show_filter);
        view.accept.set_text(match (confirming, mode) {
            (true, _) => "Overwrite",
            (false, FileDialogMode::Open) => "Open",
            (false, FileDialogMode::Save) => "Save",
        });
    }

    /// Writes the path bar and the filename field from the state.
    pub(super) fn sync_fields(&self) {
        let Some(view) = self.view() else {
            return;
        };
        let (path_text, name) = {
            let state = self.state.borrow();
            (state.path_text(), state.name())
        };
        view.path.set_text(&path_text);
        view.name.set_text(&name);
    }

    /// Gives the filename field the focus.
    pub(super) fn focus_name(&self) {
        if let Some(view) = self.view() {
            view.name.focus();
        }
    }

    /// Gives the accept button the focus, for the overwrite confirmation.
    pub(super) fn focus_accept(&self) {
        if let Some(view) = self.view() {
            self.ui.focus(view.accept.id());
        }
    }

    /// Hides every node and marks the dialog closed.
    pub(super) fn hide(&self) {
        self.open.set(false);
        let nodes = self.nodes.borrow();
        for id in nodes.iter() {
            self.ui.set_visible(*id, false);
        }
        if let Some(scrim) = nodes.first() {
            self.ui.invalidate(*scrim);
        }
    }

    /// Accepts the current input: navigates, shows an inline error, asks to
    /// overwrite, or delivers the chosen path.
    pub(super) fn do_accept(&self) -> Option<M> {
        let action = self.state.borrow_mut().accept();
        match action {
            Accept::None => None,
            Accept::Error(_) => {
                self.refresh_view();
                None
            }
            Accept::Navigate(dir) => {
                self.state.borrow_mut().navigate_to(dir);
                self.after_navigate();
                None
            }
            Accept::Overwrite(_) => {
                self.refresh_view();
                self.focus_accept();
                None
            }
            Accept::Chosen(path) => {
                self.hide();
                self.deliver_accept(path)
            }
        }
    }

    /// Cancels: while an overwrite prompt is pending this returns to the
    /// dialog, otherwise it hides the dialog and delivers the cancellation.
    pub(super) fn do_cancel(&self) -> Option<M> {
        if self.state.borrow().is_confirming_overwrite() {
            self.state.borrow_mut().cancel_overwrite();
            self.refresh_view();
            self.focus_name();
            return None;
        }
        self.hide();
        self.deliver_cancel()
    }

    /// Re-lists the view and refocuses the filename field after navigating.
    pub(super) fn after_navigate(&self) {
        self.sync_fields();
        self.refresh_view();
        self.focus_name();
    }

    /// Delivers `path` to the accept mapper without holding the borrow across
    /// the call, so the closure may reopen the dialog.
    pub(super) fn deliver_accept(&self, path: PathBuf) -> Option<M> {
        let mapper = self.on_accept.borrow_mut().take();
        let message = mapper.as_ref().and_then(|mapper| mapper(path));
        *self.on_accept.borrow_mut() = mapper;
        message
    }

    /// Delivers the cancellation to the cancel mapper, as [`deliver_accept`].
    ///
    /// [`deliver_accept`]: Shared::deliver_accept
    pub(super) fn deliver_cancel(&self) -> Option<M> {
        let mapper = self.on_cancel.borrow_mut().take();
        let message = mapper.as_ref().and_then(|mapper| mapper());
        *self.on_cancel.borrow_mut() = mapper;
        message
    }
}

/// The entry list's rows: one label and one leading icon per visible entry.
struct EntryRows {
    labels: Vec<String>,
    icons: Vec<Lucide>,
}

impl ListModel for EntryRows {
    fn rows(&self) -> usize {
        self.labels.len()
    }

    fn cell(&self, row: usize, column: usize) -> Option<&str> {
        (column == 0)
            .then(|| self.labels.get(row).map(String::as_str))
            .flatten()
    }

    fn icon(&self, row: usize) -> Option<IconRef> {
        self.icons.get(row).map(|icon| (*icon).into())
    }
}
