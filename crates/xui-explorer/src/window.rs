#![forbid(unsafe_code)]

//! The per-window [`App`]: one open folder, its [`IconView`] and [`StatusBar`].
//!
//! Widgets map their events to [`Msg`] through closures fixed at construction;
//! every effect (open a window, show a menu or dialog, delete, refresh) runs in
//! [`App::update`], after any `RefCell` borrow has been released. Window state
//! lives in this struct, not in shared cells.

mod actions;

use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::Rc;

use xui_core::app::{App, Ui};
use xui_core::backend::BackendError;
use xui_core::geometry::{Point, Rect};
use xui_core::message::Key;
use xui_core::units::Dip;
use xui_core::widget::{Dialog, IconView, Menu, MenuId, StatusBar, TaskDialog, TaskDialogAction};

use crate::model::{Listing, SharedListing, summarize, title};
use crate::platform::Kind;
use crate::shell::Explorer;

/// The status bar's design height.
const STATUS_HEIGHT: Dip = Dip(24.0);

/// The context menu's command ids.
const MENU_OPEN: MenuId = MenuId::new(0);
const MENU_DELETE: MenuId = MenuId::new(1);
const MENU_PROPERTIES: MenuId = MenuId::new(2);
const MENU_REFRESH: MenuId = MenuId::new(3);

/// One explorer window's messages.
#[derive(Clone, Copy)]
pub enum Msg {
    /// The icon view's selection changed.
    Selection,
    /// An item was double-clicked or activated with Return.
    Activate(usize),
    /// The icon view was right-clicked: the item (or nothing on empty space)
    /// and the pointer position in node-local pixels.
    Context(Option<usize>, Point),
    /// A context-menu command was chosen.
    Menu(MenuId),
    /// Re-list the folder.
    Refresh,
    /// Delete the selection (opens the confirm dialog).
    Delete,
    /// Show the selection's properties.
    Properties,
    /// The delete confirmation was answered.
    Confirm(TaskDialogAction),
    /// The properties dialog was dismissed.
    PropertiesClosed,
}

/// One open folder window.
pub struct ExplorerWindow {
    explorer: Rc<Explorer>,
    dir: PathBuf,
    title: String,
    listing: Rc<Listing>,
    view: Rc<IconView<Msg>>,
    status: Rc<StatusBar<Msg>>,
    menu: Menu<Msg>,
    context_item: Option<usize>,
    pending_delete: Vec<OsString>,
    confirm: Option<TaskDialog<Msg>>,
    properties: Option<Dialog<Msg>>,
}

impl ExplorerWindow {
    /// Builds a window showing `dir`, wired to `explorer`'s platform and
    /// launcher. The listing is read immediately, on the UI thread.
    pub fn new(
        ui: &mut Ui<Msg>,
        explorer: Rc<Explorer>,
        dir: PathBuf,
    ) -> Result<ExplorerWindow, BackendError> {
        let listing = Rc::new(Listing::load(explorer.platform(), &dir));
        let (view_rect, status_rect) = layout(ui);
        let view = Rc::new(
            IconView::with_model(ui, view_rect, SharedListing::new(Rc::clone(&listing)))?
                .multi_select(true)
                .on_selection(|_| Some(Msg::Selection))
                .on_activate(|index| Some(Msg::Activate(index)))
                .on_context(|item, at| Some(Msg::Context(item, at))),
        );
        let status = Rc::new(StatusBar::new(ui, status_rect, &[""])?);
        let menu = Menu::context(ui)
            .build(|scope| {
                scope.item(MENU_OPEN, "Open");
                scope.item(MENU_DELETE, "Delete");
                scope.item(MENU_PROPERTIES, "Properties");
                scope.separator();
                scope.item(MENU_REFRESH, "Refresh");
            })
            .on_select(|id| Some(Msg::Menu(id)));

        // Delete / Alt+Enter / F5 are claimed ahead of the focused widget.
        ui.on_key(|key, modifiers| match key {
            Key::DELETE => Some(Msg::Delete),
            Key::F5 => Some(Msg::Refresh),
            Key::RETURN if modifiers.alt => Some(Msg::Properties),
            _ => None,
        });

        let mut window = ExplorerWindow {
            explorer,
            dir,
            title: String::new(),
            listing,
            view,
            status,
            menu,
            context_item: None,
            pending_delete: Vec::new(),
            confirm: None,
            properties: None,
        };
        window.refresh(ui);
        Ok(window)
    }

    /// The window's current title (the folder's name, or the root's display
    /// form).
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The status bar, shared so a test can read its parts after the app is
    /// built. The window owns the other reference.
    pub fn status_bar(&self) -> Rc<StatusBar<Msg>> {
        Rc::clone(&self.status)
    }

    /// The icon view, shared so a test can read or set the selection after the
    /// app is built. The window owns the other reference.
    pub fn view_handle(&self) -> Rc<IconView<Msg>> {
        Rc::clone(&self.view)
    }

    /// Re-lists the folder, keeps the selection where the items still exist,
    /// and updates the title and status bar.
    fn refresh(&mut self, ui: &mut Ui<Msg>) {
        let selected_names = self.listing.names_of(&self.view.selection());
        let listing = Rc::new(Listing::load(self.explorer.platform(), &self.dir));
        self.listing = Rc::clone(&listing);
        self.view.set_model(SharedListing::new(Rc::clone(&listing)));
        self.view
            .set_selection(&listing.indices_of(&selected_names));

        self.title = title(&self.dir);
        ui.set_window_title(&self.title);
        self.explorer.publish_title(ui.window(), &self.title);
        self.update_status();
    }

    /// Rewrites the status bar from the listing and the selection, or shows the
    /// read error when the folder could not be listed.
    fn update_status(&self) {
        match &self.listing.error {
            Some(error) => self.status.set_parts(&[error]),
            None => {
                let parts = summarize(&self.listing.entries, &self.view.selection());
                let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
                self.status.set_parts(&refs);
            }
        }
    }

    /// Opens a directory in its own window (or reports it already open) or
    /// hands a file to the launcher.
    fn activate(&mut self, index: usize, ui: &mut Ui<Msg>) {
        let Some(entry) = self.listing.entries.get(index).cloned() else {
            return;
        };
        let path = self.dir.join(&entry.name);
        match entry.kind {
            Kind::Dir => {
                if !self.explorer.open_or_reuse(ui, path) {
                    self.status.set_parts(&["already open"]);
                }
            }
            Kind::File | Kind::Symlink => {
                if let Err(error) = self.explorer.launcher().open(&path) {
                    self.status
                        .set_parts(&[&format!("Cannot open {}: {error}", entry.display)]);
                }
            }
        }
    }

    /// Shows the context menu, first making the right-clicked item the selection
    /// if it was not part of it, and disabling the item commands on empty space.
    fn context(&mut self, item: Option<usize>, at: Point, ui: &mut Ui<Msg>) {
        self.context_item = item;
        let has_item = item.is_some();
        self.menu.set_enabled(MENU_OPEN, has_item);
        self.menu.set_enabled(MENU_DELETE, has_item);
        self.menu.set_enabled(MENU_PROPERTIES, has_item);
        self.menu.set_enabled(MENU_REFRESH, true);
        // The right click may have changed the selection (XP selects the
        // unselected tile under the pointer), so refresh the status line.
        self.update_status();
        let bounds = ui.bounds(self.view.id());
        self.menu
            .show_context(bounds.left + at.x, bounds.top + at.y);
    }

    /// Whether a confirm or properties dialog is open. Shortcuts and the
    /// context menu are ignored while one is: the modal owns the window.
    fn modal_open(&self) -> bool {
        self.confirm.as_ref().is_some_and(TaskDialog::is_open)
            || self.properties.as_ref().is_some_and(Dialog::is_open)
    }

    /// Runs a context-menu command against the right-clicked item.
    fn menu_command(&mut self, id: MenuId, ui: &mut Ui<Msg>) {
        if id == MENU_OPEN {
            if let Some(index) = self.context_item {
                self.activate(index, ui);
            }
        } else if id == MENU_DELETE {
            self.begin_delete(ui);
        } else if id == MENU_PROPERTIES {
            self.show_properties(ui);
        } else if id == MENU_REFRESH {
            self.refresh(ui);
        }
    }
}

impl App for ExplorerWindow {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Selection => self.update_status(),
            Msg::Activate(index) => {
                if !self.modal_open() {
                    self.activate(index, ui);
                }
            }
            Msg::Context(item, at) => {
                if !self.modal_open() {
                    self.context(item, at, ui);
                }
            }
            Msg::Menu(id) => {
                if !self.modal_open() {
                    self.menu_command(id, ui);
                }
            }
            Msg::Refresh => {
                if !self.modal_open() {
                    self.refresh(ui);
                }
            }
            Msg::Delete => {
                if !self.modal_open() {
                    self.begin_delete(ui);
                }
            }
            Msg::Properties => {
                if !self.modal_open() {
                    self.show_properties(ui);
                }
            }
            Msg::Confirm(action) => self.resolve_delete(action, ui),
            Msg::PropertiesClosed => self.properties = None,
        }
    }
}

/// The icon view's and status bar's rectangles, in device pixels.
fn layout(ui: &Ui<Msg>) -> (Rect, Rect) {
    let client = ui.client_rect();
    let status_height = STATUS_HEIGHT.to_px(ui.dpi()).value();
    let status_top = (client.bottom - status_height).max(client.top);
    (
        Rect::new(client.left, client.top, client.right, status_top),
        Rect::new(client.left, status_top, client.right, client.bottom),
    )
}

#[cfg(test)]
mod tests;
