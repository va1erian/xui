#![forbid(unsafe_code)]

//! Builds the notepad's widget tree: a menu bar, the find bar, the editor and a
//! status bar, laid out with `xui_core::arrange` so the window resizes cleanly.

use std::cell::Cell;
use std::rc::Rc;

use xui_code_editor::{Editor, FontConfig, Options};
use xui_core::Dip;
use xui_core::app::Ui;
use xui_core::arrange::{
    Handle, Layout, LayoutExt, build as build_with, button, checkbox, column, edit, label, row,
    status_bar,
};
use xui_core::backend::{Result, WidgetId};
use xui_core::geometry::{Rect, Size};
use xui_core::layout::Constraints;
use xui_core::widget::{Dialog, FileDialog, Menu, MenuId, Placeable};

use crate::app::{FindBar, Msg, Notepad};

/// The menu bar's design height.
const MENU_HEIGHT: Dip = Dip(30.0);
/// The find bar's design height.
const FIND_HEIGHT: Dip = Dip(34.0);

// File menu commands.
const NEW: MenuId = MenuId::new(1);
const OPEN: MenuId = MenuId::new(2);
const SAVE: MenuId = MenuId::new(3);
const SAVE_AS: MenuId = MenuId::new(4);
const QUIT: MenuId = MenuId::new(5);
// Edit menu commands.
const UNDO: MenuId = MenuId::new(11);
const REDO: MenuId = MenuId::new(12);
const CUT: MenuId = MenuId::new(13);
const COPY: MenuId = MenuId::new(14);
const PASTE: MenuId = MenuId::new(15);
const SELECT_ALL: MenuId = MenuId::new(16);
const FIND: MenuId = MenuId::new(17);
const REPLACE: MenuId = MenuId::new(18);

/// Maps a chosen menu command to a message.
fn menu_msg(id: MenuId) -> Option<Msg> {
    Some(match id {
        NEW => Msg::New,
        OPEN => Msg::Open,
        SAVE => Msg::Save,
        SAVE_AS => Msg::SaveAs,
        QUIT => Msg::Quit,
        UNDO => Msg::Undo,
        REDO => Msg::Redo,
        CUT => Msg::Cut,
        COPY => Msg::Copy,
        PASTE => Msg::Paste,
        SELECT_ALL => Msg::SelectAll,
        FIND => Msg::Find,
        REPLACE => Msg::Replace,
        _ => return None,
    })
}

/// A menu bar as a layout entry: it has a fixed design height and fills the
/// window's width.
struct MenuPane<M: 'static>(Menu<M>);

impl<M: 'static> Placeable<M> for MenuPane<M> {
    fn id(&self) -> WidgetId {
        self.0.id().expect("a menu bar owns a node")
    }

    fn measure(&self, _ui: &Ui<M>, constraints: Constraints) -> Size {
        let dpi = constraints.dpi;
        Size::new(0, MENU_HEIGHT.to_px(dpi).value())
    }
}

/// The editor as a layout entry: it takes all the leftover space.
struct EditorPane<M: 'static>(Rc<Editor<M>>);

impl<M: 'static> Placeable<M> for EditorPane<M> {
    fn id(&self) -> WidgetId {
        self.0.id()
    }

    fn measure(&self, _ui: &Ui<M>, _constraints: Constraints) -> Size {
        Size::new(0, 0)
    }
}

/// Builds the app's widgets and mounts the layout.
pub fn build(ui: &Ui<Msg>) -> Result<Notepad> {
    let editor_pane = Handle::new();
    let find_bar = FindBar::default();
    let status = Handle::new();

    let confirm = Dialog::confirm(
        ui,
        "Discard unsaved changes?",
        "This document has unsaved changes. Discard them?",
    )?
    .on_action(dialog_msg);
    let message = Dialog::message(ui, "Error", "")?.on_action(dialog_msg);

    let dialog_open = Rc::new(Cell::new(false));
    let find_open = Rc::new(Cell::new(false));

    let open_dialog = FileDialog::open_file(ui, "Open")?
        .require_existing(true)
        .on_accept(|path| Some(Msg::OpenChosen(path)))
        .on_cancel(refocus(&editor_pane, &dialog_open));
    let save_dialog = FileDialog::save_file(ui, "Save As")?
        .filter("Text files", &["txt", "md", "rs"])
        .filter("All files", &[])
        .on_accept(|path| Some(Msg::SaveChosen(path)))
        .on_cancel(refocus(&editor_pane, &dialog_open));
    ui.on_close(|| Some(Msg::CloseRequested));
    {
        let dialog_open = Rc::clone(&dialog_open);
        let find_open = Rc::clone(&find_open);
        ui.on_key(move |key, modifiers| {
            crate::app::shortcut(key, modifiers, &dialog_open, &find_open)
        });
    }

    ui.root(column().children((
        build_with(menu_bar).height(MENU_HEIGHT),
        find_row(&find_bar).fixed(FIND_HEIGHT),
        build_with(editor).bind(&editor_pane).fill(1),
        status_bar(&["Ln 1, Col 1", "Sel 0", "LF", "Saved"]).bind(&status),
    )))?;
    find_bar.set_visible(ui, false);

    let editor = Rc::clone(&editor_pane.get().0);
    editor.focus();

    Ok(Notepad {
        editor,
        document: xui_code_editor::Document::untitled(),
        search: Default::default(),
        find_bar,
        status,
        open_dialog,
        save_dialog,
        confirm,
        message,
        pending: crate::app::Pending::None,
        dialog_open,
        find_open,
    })
}

/// Creates the editor: a monospace grid, since the default UI font is
/// proportional and would space the glyphs apart.
fn editor(ui: &Ui<Msg>) -> Result<EditorPane<Msg>> {
    let options = Options {
        font: FontConfig {
            family: Some("monospace".to_owned()),
            ..FontConfig::default()
        },
        ..Options::default()
    };
    let editor =
        Editor::with_options(ui, Rect::default(), options)?.on_change(|_text| Some(Msg::Edited));
    Ok(EditorPane(Rc::new(editor)))
}

/// Creates the File and Edit menu bar.
fn menu_bar(ui: &Ui<Msg>) -> Result<MenuPane<Msg>> {
    let menu = Menu::bar(ui, Rect::default())?
        .on_select(menu_msg)
        .build(|bar| {
            bar.submenu(MenuId::new(100), "&File", |file| {
                file.item(NEW, "&New");
                file.item(OPEN, "&Open...");
                file.separator();
                file.item(SAVE, "&Save");
                file.item(SAVE_AS, "Save &As...");
                file.separator();
                file.item(QUIT, "&Quit");
            });
            bar.submenu(MenuId::new(200), "&Edit", |edit| {
                edit.item(UNDO, "&Undo");
                edit.item(REDO, "&Redo");
                edit.separator();
                edit.item(CUT, "Cu&t");
                edit.item(COPY, "&Copy");
                edit.item(PASTE, "&Paste");
                edit.item(SELECT_ALL, "Select &All");
                edit.separator();
                edit.item(FIND, "&Find...");
                edit.item(REPLACE, "&Replace...");
            });
        });
    Ok(MenuPane(menu))
}

/// The find/replace bar's widgets, wired to their messages and bound to
/// `bar`'s handles.
fn find_row(bar: &FindBar) -> Layout<Msg> {
    let [next, prev, replace, replace_all] = &bar.buttons;
    row().gap(4).children((
        edit()
            .placeholder("Find")
            .on_change(Msg::QueryChanged)
            .bind(&bar.query),
        edit().placeholder("Replace").bind(&bar.replacement),
        button("Next").on_click(Msg::FindNext).bind(next),
        button("Previous").on_click(Msg::FindPrevious).bind(prev),
        button("Replace")
            .on_click(Msg::ReplaceCurrent)
            .bind(replace),
        button("Replace all")
            .on_click(Msg::ReplaceAll)
            .bind(replace_all),
        checkbox("Regex")
            .on_toggle(Msg::RegexToggled)
            .bind(&bar.regex),
        checkbox("Match case")
            .on_toggle(Msg::CaseToggled)
            .bind(&bar.case),
        label("").bind(&bar.status),
    ))
}

/// A file dialog's cancel handler: it closes the dialog state and gives the
/// editor its focus back.
fn refocus(
    editor: &Handle<EditorPane<Msg>>,
    dialog_open: &Rc<Cell<bool>>,
) -> impl Fn() -> Option<Msg> + 'static {
    let editor = editor.clone();
    let dialog_open = Rc::clone(dialog_open);
    move || {
        dialog_open.set(false);
        editor.get().0.focus();
        None
    }
}

/// Maps a dialog dismissal to a message.
fn dialog_msg(action: xui_core::widget::DialogAction) -> Option<Msg> {
    Some(Msg::Dialog(action))
}
