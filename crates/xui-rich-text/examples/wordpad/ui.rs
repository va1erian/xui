#![forbid(unsafe_code)]

//! Builds the wordpad's widget tree: a command toolbar, a formatting row, the
//! editor and a status bar, laid out with `xui_core::arrange`.

use std::rc::Rc;

use xui_core::Dip;
use xui_core::app::Ui;
use xui_core::arrange::{LayoutExt, column, row, spacer, widget};
use xui_core::backend::{Result, WidgetId};
use xui_core::geometry::{Rect, Size};
use xui_core::layout::Insets;
use xui_core::widget::{
    Button, ComboBox, Dialog, FileDialog, Lucide, Placeable, StatusBar, ToggleButton, Toolbar,
    Tooltip,
};
use xui_rich_text::RichTextEditor;
use xui_rich_text::model::{Align, BlockKind, ListKind, StyleSummary, Tri, Wrap};

use crate::app::{Mark, Msg, Wordpad, shortcut};

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
/// The width of an icon-only button in the formatting row.
const ICON_WIDTH: Dip = Dip(32.0);

/// The toolbar as a layout entry.
struct ToolbarPane(Toolbar<Msg>);

impl Placeable<Msg> for ToolbarPane {
    fn id(&self) -> WidgetId {
        self.0.id()
    }

    fn natural_size(&self, _ui: &Ui<Msg>, _dpi: u32) -> Size {
        Size::new(0, 0)
    }
}

/// The shared editor as a layout entry.
struct EditorPane(Rc<RichTextEditor<Msg>>);

impl Placeable<Msg> for EditorPane {
    fn id(&self) -> WidgetId {
        self.0.id()
    }

    fn natural_size(&self, _ui: &Ui<Msg>, _dpi: u32) -> Size {
        Size::new(0, 0)
    }
}

/// The formatting controls the app keeps in step with the selection.
pub struct Tools {
    block: Rc<ComboBox<Msg>>,
    size: Rc<ComboBox<Msg>>,
    marks: [Rc<ToggleButton<Msg>>; 4],
    aligns: [Rc<ToggleButton<Msg>>; 4],
    lists: [Rc<ToggleButton<Msg>>; 2],
    wrap: Rc<ComboBox<Msg>>,
    /// The icon-only buttons' names, shown on hover.
    _tips: Vec<Tooltip<Msg>>,
}

impl Tools {
    /// Shows `s` in the controls; a mixed attribute shows as off.
    pub fn sync(&self, s: &StyleSummary) {
        let on = |tri: &Tri<bool>| matches!(tri, Tri::Uniform(true));
        for (button, tri) in self
            .marks
            .iter()
            .zip([&s.bold, &s.italic, &s.underline, &s.strike])
        {
            button.set_checked(on(tri));
        }
        for (button, align) in self.aligns.iter().zip(ALIGNS) {
            button.set_checked(s.align == Tri::Uniform(align));
        }
        for (button, kind) in self
            .lists
            .iter()
            .zip([ListKind::Bullet, ListKind::Numbered])
        {
            button.set_checked(matches!(s.list, Tri::Uniform(Some(item)) if item.kind == kind));
        }
        if let Tri::Uniform(kind) = s.kind {
            self.block.select(match kind {
                BlockKind::Body => 0,
                BlockKind::Heading(n) => usize::from(n.clamp(1, 3)),
                BlockKind::Quote => 4,
            });
        }
        if let Tri::Uniform(size) = s.size
            && let Some(index) = SIZES.iter().position(|&v| v == size.0)
        {
            self.size.select(index);
        }
    }

    /// Enables the wrap picker when an image is selected and shows its wrap.
    pub fn sync_wrap(&self, wrap: Option<Wrap>) {
        self.wrap.set_enabled(wrap.is_some());
        if let Some(wrap) = wrap {
            self.wrap.select(match wrap {
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

/// An icon-only toggle that raises `msg` and is named `tip` on hover.
fn toggle(
    ui: &Ui<Msg>,
    tips: &mut Vec<Tooltip<Msg>>,
    (icon, tip): (Lucide, &str),
    msg: fn() -> Msg,
) -> Result<Rc<ToggleButton<Msg>>> {
    let button = ToggleButton::auto(ui, "")?
        .icon(icon)
        .on_toggle(move |_| Some(msg()));
    tips.push(Tooltip::attach(ui, button.id(), tip)?);
    Ok(Rc::new(button))
}

/// An icon-only push button that raises `msg` and is named `tip` on hover.
fn push(
    ui: &Ui<Msg>,
    tips: &mut Vec<Tooltip<Msg>>,
    (icon, tip): (Lucide, &str),
    msg: fn() -> Msg,
) -> Result<Button<Msg>> {
    let button = Button::auto(ui, "")?
        .icon(icon)
        .on_click(move || Some(msg()));
    tips.push(Tooltip::attach(ui, button.id(), tip)?);
    Ok(button)
}

fn picker(ui: &Ui<Msg>, items: &[&str], msg: fn(usize) -> Msg) -> Result<Rc<ComboBox<Msg>>> {
    Ok(Rc::new(
        ComboBox::auto(ui, items)?.on_select(move |i| Some(msg(i))),
    ))
}

/// Builds the app's widgets and mounts the layout.
pub fn build(ui: &Ui<Msg>) -> Result<Wordpad> {
    let editor = Rc::new(
        RichTextEditor::new(ui, Rect::default())?
            .on_change(|_| Some(Msg::Edited))
            .on_selection(|summary| Some(Msg::Selection(summary.clone())))
            .on_link(|url| Some(Msg::Link(url.to_owned()))),
    );

    let commands = Toolbar::empty(ui, Rect::default())?
        .item_with_text(Lucide::FilePlus, "New (Ctrl+N)", "New")
        .item_with_text(Lucide::FolderOpen, "Open (Ctrl+O)", "Open")
        .item_with_text(Lucide::Save, "Save (Ctrl+S)", "Save")
        .item_with_text(Lucide::Download, "Export as Markdown", "Export .md")
        .separator()
        .item_with_text(Lucide::Undo2, "Undo (Ctrl+Z)", "Undo")
        .item_with_text(Lucide::Redo2, "Redo (Ctrl+Y)", "Redo")
        .separator()
        .item_with_text(Lucide::Image, "Insert image", "Image")
        .on_click(|index| {
            Some(match index {
                0 => Msg::New,
                1 => Msg::Open,
                2 => Msg::Save,
                3 => Msg::Export,
                4 => Msg::Undo,
                5 => Msg::Redo,
                _ => Msg::InsertImage,
            })
        });

    let block = picker(ui, &BLOCKS, Msg::Block)?;
    for (index, icon) in BLOCK_ICONS.into_iter().enumerate() {
        block.set_item_icon(index, Some(icon.into()));
    }
    let size_labels: Vec<String> = SIZES.iter().map(|s| format!("{s}")).collect();
    let size_refs: Vec<&str> = size_labels.iter().map(String::as_str).collect();
    let size = picker(ui, &size_refs, Msg::Size)?;
    size.select(2);
    let wrap = picker(ui, &WRAPS, Msg::Wrap)?;
    wrap.set_enabled(false);

    let mut tips = Vec::new();
    let t = &mut tips;
    let marks = [
        toggle(ui, t, (Lucide::Bold, "Bold (Ctrl+B)"), || {
            Msg::Toggle(Mark::Bold)
        })?,
        toggle(ui, t, (Lucide::Italic, "Italic (Ctrl+I)"), || {
            Msg::Toggle(Mark::Italic)
        })?,
        toggle(ui, t, (Lucide::Underline, "Underline (Ctrl+U)"), || {
            Msg::Toggle(Mark::Underline)
        })?,
        toggle(ui, t, (Lucide::Strikethrough, "Strikethrough"), || {
            Msg::Toggle(Mark::Strike)
        })?,
    ];
    let aligns = [
        toggle(ui, t, (Lucide::TextAlignStart, "Align left"), || {
            Msg::Align(Align::Left)
        })?,
        toggle(ui, t, (Lucide::TextAlignCenter, "Centre"), || {
            Msg::Align(Align::Center)
        })?,
        toggle(ui, t, (Lucide::TextAlignEnd, "Align right"), || {
            Msg::Align(Align::Right)
        })?,
        toggle(ui, t, (Lucide::TextAlignJustify, "Justify"), || {
            Msg::Align(Align::Justify)
        })?,
    ];
    let lists = [
        toggle(ui, t, (Lucide::List, "Bulleted list"), || {
            Msg::List(ListKind::Bullet)
        })?,
        toggle(ui, t, (Lucide::ListOrdered, "Numbered list"), || {
            Msg::List(ListKind::Numbered)
        })?,
    ];
    let indent = push(ui, t, (Lucide::IndentIncrease, "Indent"), || Msg::Indent)?;
    let outdent = push(ui, t, (Lucide::IndentDecrease, "Outdent"), || Msg::Outdent)?;

    let status = Rc::new(StatusBar::auto(ui, &["New document", "Saved"])?);

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

    let gap = Dip(4.0);
    let root = column()
        .child(widget(ToolbarPane(commands)).height(TOOLBAR_HEIGHT))
        .child(
            row()
                .spacing(gap)
                .margins(Insets::symmetric(Dip(8.0), Dip(3.0)))
                .child((&block).width(Dip(120.0)))
                .child((&size).width(Dip(64.0)))
                .child(spacer().width(Dip(6.0)))
                .child((&marks[0]).width(ICON_WIDTH))
                .child((&marks[1]).width(ICON_WIDTH))
                .child((&marks[2]).width(ICON_WIDTH))
                .child((&marks[3]).width(ICON_WIDTH))
                .child(spacer().width(Dip(6.0)))
                .child((&aligns[0]).width(ICON_WIDTH))
                .child((&aligns[1]).width(ICON_WIDTH))
                .child((&aligns[2]).width(ICON_WIDTH))
                .child((&aligns[3]).width(ICON_WIDTH))
                .child(spacer().width(Dip(6.0)))
                .child((&lists[0]).width(ICON_WIDTH))
                .child((&lists[1]).width(ICON_WIDTH))
                .child(outdent.width(ICON_WIDTH))
                .child(indent.width(ICON_WIDTH))
                .child(spacer().width(Dip(6.0)))
                .child((&wrap).width(Dip(120.0)))
                .child(spacer())
                .fixed(FORMAT_HEIGHT),
        )
        .child(widget(EditorPane(Rc::clone(&editor))).fill(1))
        .child(&status);
    let mounted = ui.mount(root)?;
    editor.focus();

    Ok(Wordpad {
        editor,
        tools: Tools {
            block,
            size,
            marks,
            aligns,
            lists,
            wrap,
            _tips: tips,
        },
        status,
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
        _mounted: mounted,
    })
}
