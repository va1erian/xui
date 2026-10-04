#![forbid(unsafe_code)]

//! The wordpad's state, messages and the thin `update` dispatcher.

use std::path::PathBuf;
use std::rc::Rc;

use xui_core::app::{App, Ui};
use xui_core::message::{Key, Modifiers};
use xui_core::widget::{Dialog, DialogAction, FileDialog, StatusBar};
use xui_rich_text::RichTextEditor;
use xui_rich_text::model::{Align, ListKind, StyleSummary};

use crate::commands;
use crate::table::{TableAction, TableTools};
use crate::ui::Tools;

/// A character attribute the B / I / U / S buttons toggle.
#[derive(Clone, Copy, Debug)]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Strike,
}

/// One variant per user intent.
pub enum Msg {
    New,
    Open,
    OpenChosen(PathBuf),
    Save,
    SaveAs,
    SaveChosen(PathBuf),
    Export,
    ExportChosen(PathBuf),
    InsertImage,
    ImageChosen(PathBuf),
    /// Ctrl+Enter from the toolbar: the rest of the paragraph starts a page.
    PageBreak,
    /// The Page view toggle.
    PageView(bool),
    Undo,
    Redo,
    /// The document changed.
    Edited,
    /// The selection or the formatting under it changed.
    Selection(StyleSummary),
    /// A link was Ctrl+clicked.
    Link(String),
    /// A block kind was picked (index into the list).
    Block(usize),
    /// A font size was picked (index into the list).
    Size(usize),
    Toggle(Mark),
    Align(Align),
    List(ListKind),
    Indent,
    Outdent,
    /// A table button was pressed.
    Table(TableAction),
    /// A wrap was picked for the selected image (index into the list).
    Wrap(usize),
    /// A confirmation or message dialog was dismissed.
    Dialog(DialogAction),
    /// A file picker was cancelled.
    PickerClosed,
    CloseRequested,
    Autoclose,
}

/// What to do once the discard confirmation is accepted.
#[derive(Clone, Copy)]
pub enum After {
    New,
    Open,
    Quit,
}

/// The wordpad application.
pub struct Wordpad {
    pub editor: Rc<RichTextEditor<Msg>>,
    pub tools: Tools,
    pub table: TableTools,
    pub status: Rc<StatusBar<Msg>>,
    pub open_dialog: FileDialog<Msg>,
    pub save_dialog: FileDialog<Msg>,
    pub export_dialog: FileDialog<Msg>,
    pub image_dialog: FileDialog<Msg>,
    pub confirm: Dialog<Msg>,
    pub message: Dialog<Msg>,
    /// The saved file, if any.
    pub path: Option<PathBuf>,
    /// Whether the document changed since it was saved.
    pub dirty: bool,
    /// The last formatting summary, re-applied to the toolbar after commands.
    pub summary: Option<StyleSummary>,
    /// What an accepted discard confirmation leads to.
    pub after: Option<After>,
    /// Whether a dialog or picker is open, so shortcuts leave it alone.
    pub dialog_open: bool,
}

/// Maps Ctrl+N / O / S (and Shift for Save As) to messages.
pub fn shortcut(key: Key, modifiers: Modifiers) -> Option<Msg> {
    if !(modifiers.ctrl || modifiers.win) {
        return None;
    }
    match key {
        Key::N => Some(Msg::New),
        Key::O => Some(Msg::Open),
        Key::S if modifiers.shift => Some(Msg::SaveAs),
        Key::S => Some(Msg::Save),
        _ => None,
    }
}

impl App for Wordpad {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::New => commands::new_document(self, ui),
            Msg::Open => commands::open(self, ui),
            Msg::OpenChosen(path) => commands::open_chosen(self, ui, path),
            Msg::Save => commands::save(self, ui),
            Msg::SaveAs => commands::save_as(self),
            Msg::SaveChosen(path) => commands::save_chosen(self, ui, path),
            Msg::Export => commands::export(self),
            Msg::ExportChosen(path) => commands::export_chosen(self, path),
            Msg::InsertImage => commands::insert_image(self),
            Msg::ImageChosen(path) => commands::image_chosen(self, path),
            Msg::PageBreak => commands::format(self, xui_rich_text::edit::Command::InsertPageBreak),
            Msg::PageView(on) => commands::page_view(self, on),
            Msg::Undo => commands::format(self, xui_rich_text::edit::Command::Undo),
            Msg::Redo => commands::format(self, xui_rich_text::edit::Command::Redo),
            Msg::Edited => {
                commands::edited(self, ui);
                commands::refresh_pages(self);
            }
            Msg::Selection(summary) => {
                commands::selection(self, summary);
                commands::refresh_pages(self);
            }
            Msg::Link(url) => self.status.set_text(0, &url),
            Msg::Block(index) => commands::block(self, index),
            Msg::Size(index) => commands::size(self, index),
            Msg::Toggle(mark) => commands::toggle(self, mark),
            Msg::Align(align) => commands::align(self, align),
            Msg::List(kind) => commands::list(self, kind),
            Msg::Indent => commands::format(self, xui_rich_text::edit::Command::Indent),
            Msg::Outdent => commands::format(self, xui_rich_text::edit::Command::Outdent),
            Msg::Wrap(index) => commands::wrap(self, index),
            Msg::Table(action) => commands::table(self, action),
            Msg::Dialog(action) => commands::dialog(self, ui, action),
            Msg::PickerClosed => commands::picker_closed(self),
            Msg::CloseRequested => commands::quit(self, ui),
            Msg::Autoclose => ui.quit(),
        }
    }
}
