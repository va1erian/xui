#![forbid(unsafe_code)]

//! The wordpad's behaviour: files, dialogs and the formatting commands.

use std::path::PathBuf;

use xui_core::Dip;
use xui_core::app::Ui;
use xui_core::widget::DialogAction;
use xui_rich_text::ViewMode;
use xui_rich_text::edit::Command;
use xui_rich_text::model::{
    Align, BlockKind, CharStylePatch, Document, ListKind, Selection, Side, StyleSummary, Wrap,
};

use crate::app::{After, Mark, Msg, Wordpad};
use crate::files;
use crate::ui::{BLOCKS, SIZES, WRAPS};

/// Refreshes the title bar and status bar.
pub fn refresh_title(app: &Wordpad, ui: &Ui<Msg>) {
    let name = app.path.as_ref().and_then(|p| p.file_name()).map_or_else(
        || "Untitled".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    ui.set_window_title(&format!(
        "{}{name} - Wordpad",
        if app.dirty { "*" } else { "" }
    ));
    app.status
        .set_text(1, if app.dirty { "Modified" } else { "Saved" });
}

/// Shows the caret's page and the page count in the status bar.
pub fn refresh_pages(app: &Wordpad) {
    let (page, count) = app.editor.page_info();
    app.status
        .set_text(2, &format!("Page {} of {count}", page + 1));
}

/// Switches between page and draft view.
pub fn page_view(app: &mut Wordpad, on: bool) {
    app.editor
        .set_view_mode(if on { ViewMode::Page } else { ViewMode::Draft });
    app.editor.focus();
    refresh_pages(app);
}

/// The document changed.
pub fn edited(app: &mut Wordpad, ui: &mut Ui<Msg>) {
    app.dirty = true;
    refresh_title(app, ui);
}

/// The selection or its formatting changed: update the toolbar.
pub fn selection(app: &mut Wordpad, summary: StyleSummary) {
    app.tools.sync(&summary);
    app.summary = Some(summary);
    let image = match app.editor.selection() {
        Selection::Object(id) => app
            .editor
            .with_document(|d| d.objects().get(id).map(|o| o.wrap)),
        Selection::Text { .. } => None,
    };
    app.tools.sync_wrap(image);
}

/// Runs an editor command from the toolbar and returns focus to the text.
pub fn format(app: &mut Wordpad, command: Command) {
    app.editor.exec(command);
    app.editor.focus();
    if let Some(summary) = &app.summary {
        app.tools.sync(summary);
    }
}

pub fn block(app: &mut Wordpad, index: usize) {
    let kind = match index {
        1..=3 => BlockKind::Heading(index as u8),
        4 => BlockKind::Quote,
        _ => BlockKind::Body,
    };
    debug_assert!(index < BLOCKS.len());
    format(app, Command::SetBlockKind(kind));
}

pub fn size(app: &mut Wordpad, index: usize) {
    let Some(&size) = SIZES.get(index) else {
        return;
    };
    let patch = CharStylePatch {
        size: Some(Dip(size)),
        ..CharStylePatch::default()
    };
    format(app, Command::SetCharStyle(patch));
}

pub fn toggle(app: &mut Wordpad, mark: Mark) {
    format(
        app,
        match mark {
            Mark::Bold => Command::ToggleBold,
            Mark::Italic => Command::ToggleItalic,
            Mark::Underline => Command::ToggleUnderline,
            Mark::Strike => Command::ToggleStrike,
        },
    );
}

pub fn align(app: &mut Wordpad, align: Align) {
    format(app, Command::SetAlign(align));
}

pub fn list(app: &mut Wordpad, kind: ListKind) {
    format(app, Command::ToggleList(kind));
}

/// Sets the wrap of the selected image.
pub fn wrap(app: &mut Wordpad, index: usize) {
    let Selection::Object(id) = app.editor.selection() else {
        return;
    };
    let wrap = match index {
        1 => Wrap::square(Side::Left),
        2 => Wrap::square(Side::Right),
        3 => Wrap::TopAndBottom { margin: Dip(8.0) },
        _ => Wrap::Inline,
    };
    debug_assert!(index < WRAPS.len());
    format(app, Command::SetWrap { id, wrap });
}

/// New: confirm discarding unsaved changes first.
pub fn new_document(app: &mut Wordpad, ui: &mut Ui<Msg>) {
    guard(app, ui, After::New);
}

/// Open: confirm discarding unsaved changes first.
pub fn open(app: &mut Wordpad, ui: &mut Ui<Msg>) {
    guard(app, ui, After::Open);
}

/// Quit: confirm discarding unsaved changes first.
pub fn quit(app: &mut Wordpad, ui: &mut Ui<Msg>) {
    guard(app, ui, After::Quit);
}

fn guard(app: &mut Wordpad, ui: &mut Ui<Msg>, after: After) {
    if app.dialog_open {
        return;
    }
    if app.dirty {
        app.after = Some(after);
        app.confirm.open();
        app.dialog_open = true;
    } else {
        proceed(app, ui, after);
    }
}

fn proceed(app: &mut Wordpad, ui: &mut Ui<Msg>, after: After) {
    match after {
        After::New => load(app, ui, Document::new(), None),
        After::Open => {
            app.dialog_open = true;
            app.open_dialog.open();
        }
        After::Quit => ui.quit(),
    }
}

/// A confirmation or message dialog was dismissed.
pub fn dialog(app: &mut Wordpad, ui: &mut Ui<Msg>, action: DialogAction) {
    app.dialog_open = false;
    match (app.after.take(), action) {
        (Some(after), DialogAction::Accept(_)) => proceed(app, ui, after),
        _ => app.editor.focus(),
    }
}

/// A file picker was cancelled.
pub fn picker_closed(app: &mut Wordpad) {
    app.dialog_open = false;
    app.editor.focus();
}

fn load(app: &mut Wordpad, ui: &mut Ui<Msg>, doc: Document, path: Option<PathBuf>) {
    app.editor.set_document(doc);
    app.path = path;
    app.dirty = false;
    app.summary = None;
    app.status.set_text(
        0,
        &app.path
            .as_ref()
            .map_or_else(|| "New document".to_owned(), |p| p.display().to_string()),
    );
    refresh_title(app, ui);
    app.editor.focus();
    let summary = app
        .editor
        .with_document(|d| d.style_summary(&Selection::default()));
    selection(app, summary);
}

fn fail(app: &mut Wordpad, what: &str, error: String) {
    app.message.set_message(&format!("{what}: {error}"));
    app.message.open();
    app.dialog_open = true;
}

pub fn open_chosen(app: &mut Wordpad, ui: &mut Ui<Msg>, path: PathBuf) {
    app.dialog_open = false;
    match files::load_json(&path) {
        Ok(doc) => load(app, ui, doc, Some(path)),
        Err(error) => fail(app, &format!("Could not open {}", path.display()), error),
    }
}

pub fn save(app: &mut Wordpad, ui: &mut Ui<Msg>) {
    if app.dialog_open {
        return;
    }
    match app.path.clone() {
        Some(path) => save_to(app, ui, path),
        None => save_as(app),
    }
}

pub fn save_as(app: &mut Wordpad) {
    if app.dialog_open {
        return;
    }
    app.dialog_open = true;
    app.save_dialog.open();
}

pub fn save_chosen(app: &mut Wordpad, ui: &mut Ui<Msg>, path: PathBuf) {
    app.dialog_open = false;
    save_to(app, ui, path);
}

fn save_to(app: &mut Wordpad, ui: &mut Ui<Msg>, path: PathBuf) {
    match app.editor.with_document(|d| files::save_json(&path, d)) {
        Ok(()) => {
            app.path = Some(path);
            app.dirty = false;
            refresh_title(app, ui);
            app.editor.focus();
        }
        Err(error) => fail(app, &format!("Could not save {}", path.display()), error),
    }
}

pub fn export(app: &mut Wordpad) {
    if !app.dialog_open {
        app.dialog_open = true;
        app.export_dialog.open();
    }
}

pub fn export_chosen(app: &mut Wordpad, path: PathBuf) {
    app.dialog_open = false;
    match app
        .editor
        .with_document(|d| files::export_markdown(&path, d))
    {
        Ok(()) => app
            .status
            .set_text(0, &format!("Exported {}", path.display())),
        Err(error) => fail(app, &format!("Could not export {}", path.display()), error),
    }
    app.editor.focus();
}

pub fn insert_image(app: &mut Wordpad) {
    if !app.dialog_open {
        app.dialog_open = true;
        app.image_dialog.open();
    }
}

pub fn image_chosen(app: &mut Wordpad, path: PathBuf) {
    app.dialog_open = false;
    match files::load_image(&path) {
        Ok(image) => format(app, Command::InsertImage(image)),
        Err(error) => fail(app, &format!("Could not insert {}", path.display()), error),
    }
}
