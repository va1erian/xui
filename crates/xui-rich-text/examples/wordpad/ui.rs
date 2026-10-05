#![forbid(unsafe_code)]

//! Builds the wordpad's widget tree: a command toolbar, a formatting row, the
//! table row, the editor and a status bar, laid out with `xui_core::arrange`.

use std::cell::RefCell;
use std::rc::Rc;

use xui_core::Dip;
use xui_core::app::Ui;
use xui_core::arrange::{
    Build, Entry, Handle, Layout, LayoutExt, build as create, button, column, combo_box, row,
    spacer, status_bar, toggle_button, toolbar,
};
use xui_core::backend::Result;
use xui_core::geometry::Rect;
use xui_core::layout::Insets;
use xui_core::widget::{
    ComboBox, Dialog, FileDialog, Lucide, Placeable, ToggleButton, Toolbar, Tooltip,
};
use xui_rich_text::model::{Align, BlockKind, ListKind, StyleSummary, Tri, Wrap};
use xui_rich_text::{RichTextEditor, ViewMode};

use crate::app::{Mark, Msg, Wordpad, shortcut};
use crate::table::TableTools;

/// The block kinds the picker offers, in order.
pub const BLOCKS: [&str; 5] = ["Normal", "Heading 1", "Heading 2", "Heading 3", "Quote"];
/// The icon of each block kind in the picker, in the same order.
const BLOCK_ICONS: [Lucide; 5] = [
    Lucide::Type,
    Lucide::Heading1,
    Lucide::Heading2,
    Lucide::Heading3,
    Lucide::TextQuote,
];
/// The font sizes the picker offers, in design units.
pub const SIZES: [f32; 8] = [10.0, 12.0, 14.0, 16.0, 18.0, 24.0, 32.0, 48.0];
/// The image wraps the picker offers, in order.
pub const WRAPS: [&str; 4] = ["Inline", "Float left", "Float right", "Top and bottom"];

const TOOLBAR_HEIGHT: Dip = Dip(36.0);
const FORMAT_HEIGHT: Dip = Dip(34.0);
/// The width of an icon-only button in the formatting and table rows.
pub const ICON_WIDTH: Dip = Dip(32.0);

/// The tooltips naming the icon-only buttons, attached as each is created.
pub type Tips = Rc<RefCell<Vec<Tooltip<Msg>>>>;

/// Names `widget` with `tip` on hover once it is created.
pub fn tipped<W: Placeable<Msg>>(
    widget: Build<W, Msg>,
    tips: &Tips,
    tip: &'static str,
) -> Build<W, Msg> {
    let tips = Rc::clone(tips);
    widget.then_with(move |widget, ui| {
        tips.borrow_mut()
            .push(Tooltip::attach(ui, widget.id(), tip)?);
        Ok(widget)
    })
}

/// The formatting controls the app keeps in step with the selection.
#[derive(Default)]
pub struct Tools {
    block: Handle<ComboBox<Msg>>,
    size: Handle<ComboBox<Msg>>,
    marks: [Handle<ToggleButton<Msg>>; 4],
    aligns: [Handle<ToggleButton<Msg>>; 4],
    lists: [Handle<ToggleButton<Msg>>; 2],
    wrap: Handle<ComboBox<Msg>>,
    /// The icon-only buttons' names, shown on hover.
    tips: Tips,
}

impl Tools {
    /// The formatting row, bound to these controls.
    fn row(&self) -> Layout<Msg> {
        let tips = &self.tips;
        let size_labels: Vec<String> = SIZES.iter().map(|s| format!("{s}")).collect();
        let size_refs: Vec<&str> = size_labels.iter().map(String::as_str).collect();
        let [bold, italic, underline, strike] = &self.marks;
        let [left, centre, right, justify] = &self.aligns;
        let [bullets, numbers] = &self.lists;
        let page_view = toggle_button("")
            .then(|button| button.icon(Lucide::BookOpen))
            .on_toggle(Msg::PageView)
            .checked(true);
        row()
            .gap(4)
            .padding(Insets::symmetric(Dip(8.0), Dip(3.0)))
            .children((
                combo_box(&BLOCKS)
                    .bind(&self.block)
                    .on_select(Msg::Block)
                    .then(|block| {
                        for (index, icon) in BLOCK_ICONS.into_iter().enumerate() {
                            block.set_item_icon(index, Some(icon.into()));
                        }
                        block
                    })
                    .width(120),
                combo_box(&size_refs)
                    .bind(&self.size)
                    .on_select(Msg::Size)
                    .then(|size| {
                        size.select(2);
                        size
                    })
                    .width(64),
                spacer().width(6),
                toggle(tips, bold, (Lucide::Bold, "Bold (Ctrl+B)"), || {
                    Msg::Toggle(Mark::Bold)
                }),
                toggle(tips, italic, (Lucide::Italic, "Italic (Ctrl+I)"), || {
                    Msg::Toggle(Mark::Italic)
                }),
                toggle(
                    tips,
                    underline,
                    (Lucide::Underline, "Underline (Ctrl+U)"),
                    || Msg::Toggle(Mark::Underline),
                ),
                toggle(
                    tips,
                    strike,
                    (Lucide::Strikethrough, "Strikethrough"),
                    || Msg::Toggle(Mark::Strike),
                ),
                spacer().width(6),
            ))
            .children((
                toggle(tips, left, (Lucide::TextAlignStart, "Align left"), || {
                    Msg::Align(Align::Left)
                }),
                toggle(tips, centre, (Lucide::TextAlignCenter, "Centre"), || {
                    Msg::Align(Align::Center)
                }),
                toggle(tips, right, (Lucide::TextAlignEnd, "Align right"), || {
                    Msg::Align(Align::Right)
                }),
                toggle(tips, justify, (Lucide::TextAlignJustify, "Justify"), || {
                    Msg::Align(Align::Justify)
                }),
                spacer().width(6),
                toggle(tips, bullets, (Lucide::List, "Bulleted list"), || {
                    Msg::List(ListKind::Bullet)
                }),
                toggle(
                    tips,
                    numbers,
                    (Lucide::ListOrdered, "Numbered list"),
                    || Msg::List(ListKind::Numbered),
                ),
                push(tips, (Lucide::IndentDecrease, "Outdent"), || Msg::Outdent),
                push(tips, (Lucide::IndentIncrease, "Indent"), || Msg::Indent),
                spacer().width(6),
            ))
            .children((
                combo_box(&WRAPS)
                    .bind(&self.wrap)
                    .on_select(Msg::Wrap)
                    .then(|wrap| {
                        wrap.set_enabled(false);
                        wrap
                    })
                    .width(120),
                spacer(),
                tipped(page_view, tips, "Page view").width(ICON_WIDTH),
            ))
    }

    /// Shows `s` in the controls; a mixed attribute shows as off.
    pub fn sync(&self, s: &StyleSummary) {
        let on = |tri: &Tri<bool>| matches!(tri, Tri::Uniform(true));
        for (button, tri) in self
            .marks
            .iter()
            .zip([&s.bold, &s.italic, &s.underline, &s.strike])
        {
            button.get().set_checked(on(tri));
        }
        for (button, align) in self.aligns.iter().zip(ALIGNS) {
            button.get().set_checked(s.align == Tri::Uniform(align));
        }
        for (button, kind) in self
            .lists
            .iter()
            .zip([ListKind::Bullet, ListKind::Numbered])
        {
            button
                .get()
                .set_checked(matches!(s.list, Tri::Uniform(Some(item)) if item.kind == kind));
        }
        if let Tri::Uniform(kind) = s.kind {
            self.block.get().select(match kind {
                BlockKind::Body => 0,
                BlockKind::Heading(n) => usize::from(n.clamp(1, 3)),
                BlockKind::Quote => 4,
            });
        }
        if let Tri::Uniform(size) = s.size
            && let Some(index) = SIZES.iter().position(|&v| v == size.0)
        {
            self.size.get().select(index);
        }
    }

    /// Enables the wrap picker when an image is selected and shows its wrap.
    pub fn sync_wrap(&self, wrap: Option<Wrap>) {
        let picker = self.wrap.get();
        picker.set_enabled(wrap.is_some());
        if let Some(wrap) = wrap {
            picker.select(match wrap {
                Wrap::Inline => 0,
                Wrap::Square {
                    side: xui_rich_text::model::Side::Left,
                    ..
                } => 1,
                Wrap::Square { .. } => 2,
                Wrap::TopAndBottom { .. } => 3,
            });
        }
    }
}

const ALIGNS: [Align; 4] = [Align::Left, Align::Center, Align::Right, Align::Justify];

/// An icon-only toggle bound to `handle` that raises `msg` and is named `tip`
/// on hover.
fn toggle(
    tips: &Tips,
    handle: &Handle<ToggleButton<Msg>>,
    (icon, tip): (Lucide, &'static str),
    msg: fn() -> Msg,
) -> Entry<Msg> {
    let button = toggle_button("")
        .bind(handle)
        .then(move |button| button.icon(icon))
        .on_toggle(move |_| msg());
    tipped(button, tips, tip).width(ICON_WIDTH)
}

/// An icon-only push button that raises `msg` and is named `tip` on hover.
fn push(tips: &Tips, (icon, tip): (Lucide, &'static str), msg: fn() -> Msg) -> Entry<Msg> {
    let button = button("")
        .then(move |button| button.icon(icon))
        .on_click_with(move || Some(msg()));
    tipped(button, tips, tip).width(ICON_WIDTH)
}

/// The command toolbar.
fn commands() -> Build<Toolbar<Msg>, Msg> {
    toolbar()
        .item_with_text(Lucide::FilePlus, "New (Ctrl+N)", "New")
        .item_with_text(Lucide::FolderOpen, "Open (Ctrl+O)", "Open")
        .item_with_text(Lucide::Save, "Save (Ctrl+S)", "Save")
        .item_with_text(Lucide::Download, "Export as Markdown", "Export .md")
        .separator()
        .item_with_text(Lucide::Undo2, "Undo (Ctrl+Z)", "Undo")
        .item_with_text(Lucide::Redo2, "Redo (Ctrl+Y)", "Redo")
        .separator()
        .item_with_text(Lucide::Image, "Insert image", "Image")
        .item_with_text(
            Lucide::SeparatorHorizontal,
            "Page break (Ctrl+Enter)",
            "Page break",
        )
        .on_click(|index| match index {
            0 => Msg::New,
            1 => Msg::Open,
            2 => Msg::Save,
            3 => Msg::Export,
            4 => Msg::Undo,
            5 => Msg::Redo,
            6 => Msg::InsertImage,
            _ => Msg::PageBreak,
        })
}

/// The rich-text editor, in page view.
fn new_editor(ui: &Ui<Msg>) -> Result<RichTextEditor<Msg>> {
    let editor = RichTextEditor::new(ui, Rect::default())?
        .on_change(|_| Some(Msg::Edited))
        .on_selection(|summary| Some(Msg::Selection(summary.clone())))
        .on_link(|url| Some(Msg::Link(url.to_owned())));
    editor.set_view_mode(ViewMode::Page);
    Ok(editor)
}

/// Builds the app's widgets and mounts the layout.
pub fn build(ui: &Ui<Msg>) -> Result<Wordpad> {
    let tools = Tools::default();
    let table = TableTools::default();
    let editor = Handle::new();
    let status = Handle::new();
    ui.root(column().children((
        commands().height(TOOLBAR_HEIGHT),
        tools.row().fixed(FORMAT_HEIGHT),
        table.row().fixed(FORMAT_HEIGHT),
        create(new_editor).bind(&editor).fill(1),
        status_bar(&["New document", "Saved", "Page 1 of 1"]).bind(&status),
    )))?;
    table.sync(None);
    let editor = editor.get();
    editor.focus();

    let confirm = Dialog::confirm(
        ui,
        "Discard unsaved changes?",
        "This document has unsaved changes. Discard them?",
    )?
    .on_action(|action| Some(Msg::Dialog(action)));
    let message = Dialog::message(ui, "Wordpad", "")?.on_action(|action| Some(Msg::Dialog(action)));
    let open_dialog = FileDialog::open_file(ui, "Open")?
        .require_existing(true)
        .filter("Wordpad documents", &["json"])
        .on_accept(|p| Some(Msg::OpenChosen(p)))
        .on_cancel(|| Some(Msg::PickerClosed));
    let save_dialog = FileDialog::save_file(ui, "Save As")?
        .suggested_name("document.json")
        .filter("Wordpad documents", &["json"])
        .on_accept(|p| Some(Msg::SaveChosen(p)))
        .on_cancel(|| Some(Msg::PickerClosed));
    let export_dialog = FileDialog::save_file(ui, "Export Markdown")?
        .suggested_name("document.md")
        .filter("Markdown", &["md"])
        .on_accept(|p| Some(Msg::ExportChosen(p)))
        .on_cancel(|| Some(Msg::PickerClosed));
    let image_dialog = FileDialog::open_file(ui, "Insert image")?
        .require_existing(true)
        .filter("Images", &["png", "jpg", "jpeg"])
        .on_accept(|p| Some(Msg::ImageChosen(p)))
        .on_cancel(|| Some(Msg::PickerClosed));
    ui.on_close(|| Some(Msg::CloseRequested));
    ui.on_key(shortcut);

    Ok(Wordpad {
        editor,
        tools,
        table,
        status: status.get(),
        open_dialog,
        save_dialog,
        export_dialog,
        image_dialog,
        confirm,
        message,
        path: None,
        dirty: false,
        summary: None,
        after: None,
        dialog_open: false,
    })
}
