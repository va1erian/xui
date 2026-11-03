//! The gallery's layout: four columns of widgets, with the status lines
//! under the first two.

use xui::Lucide;
use xui::prelude::*;
use xui_core::Color;
use xui_core::image::Image;
use xui_core::widget::Glyph;

use crate::{Msg, menus};

/// The widgets the app changes after it starts.
#[derive(Default)]
pub struct Parts {
    pub echo: Handle<Label<Msg>>,
    pub status: Handle<StatusBar<Msg>>,
    pub bar: Handle<ProgressBar<Msg>>,
    pub link: Handle<Hyperlink<Msg>>,
    pub menu: Handle<Menu<Msg>>,
}

/// The whole window: `switch` is the renderer button's label, when the
/// gallery can switch backends; `dark` starts the theme choice on `Dark`.
pub fn page(parts: &Parts, switch: Option<String>, dark: bool) -> Layout<Msg> {
    let left = column().gap(8).children((
        row()
            .gap(20)
            .children((controls(parts, switch, dark).fill(1), views(parts).fill(1)))
            .fill(1),
        separator(),
        status_bar(&["Ready", ""]).bind(&parts.status),
        material_status_bar(&["Native", "light"]),
        label("Edit: ").bind(&parts.echo),
    ));
    row().padding(16).gap(20).children((
        left.width(748),
        containers().width(236),
        colors().width(240),
    ))
}

/// The first column: text, choices and values.
fn controls(parts: &Parts, switch: Option<String>, dark: bool) -> Layout<Msg> {
    let mut buttons = vec![button("Context menu").on_click(Msg::ContextMenu).width(174)];
    if let Some(label) = switch {
        buttons.push(button(label).on_click(Msg::Switch).fill(1));
    }
    column().gap(8).children((
        label("xui widgets"),
        edit().text("type here").on_change(Msg::Edit),
        number_field(0.0, 100.0, 5.0)
            .then(|field| field.on_commit(|value| Some(Msg::Number(value)))),
        row().gap(10).children((
            checkbox("Enabled").on_toggle(Msg::Check).width(134),
            toggle_button("Bold").on_toggle(Msg::Toggle).fill(1),
        )),
        radio_group(&["Small", "Medium", "Large"]).on_select(Msg::Radio),
        combo_box(&["Alpha", "Beta", "Gamma"]).on_select(Msg::Combo),
        slider(0.0, 100.0).on_change(Msg::Slide).then(|slider| {
            slider.set_value(40.0);
            slider
        }),
        progress(100).value(40).bind(&parts.bar),
        hyperlink("Open docs").on_click(Msg::Link).bind(&parts.link),
        radio_group(&["Light", "Dark"])
            .selected(usize::from(dark))
            .on_select(Msg::Theme),
        menus::top(),
        row().gap(8).children(buttons),
    ))
}

/// The second column: the menu bar, item views and grouping containers.
fn views(parts: &Parts) -> Layout<Msg> {
    column().gap(8).children((
        menus::bar().bind(&parts.menu),
        list()
            .then(|list| {
                list.set_items(&["Inbox", "Sent", "Drafts", "Archive"]);
                list.select(Some(0));
                list.on_select(|index| Some(Msg::List(index)))
            })
            .height(112),
        tree_view()
            .rows([
                TreeRow::new("Inbox", 0)
                    .expandable(true)
                    .expanded(true)
                    .icon(Glyph::Folder),
                TreeRow::new("Work", 1).icon(Glyph::Tag),
                TreeRow::new("Home", 1).icon(art_image()),
                TreeRow::new("Archive", 0)
                    .expandable(true)
                    .icon(Glyph::History),
            ])
            .on_select(Msg::Tree)
            .height(96),
        group(
            "Group",
            column()
                .padding(8)
                .child(checkbox("Inside the group").on_toggle(Msg::Check)),
        ),
        multiline_edit()
            .then(|notes| {
                notes.set_text("Notes…");
                notes
            })
            .height(64),
        panel(column().padding(12).child(label("In a panel"))).height(68),
        toolbar()
            .item(Lucide::FilePlus, "New")
            .item(Lucide::FolderOpen, "Open")
            .separator()
            .item_with_text(Lucide::Save, "Save", "Save")
            .on_click(Msg::Tool),
    ))
}

/// The third column: a scroll view, tabs and a split, each holding labels.
fn containers() -> Layout<Msg> {
    let rows: Vec<Entry<Msg>> = (1..=6)
        .map(|row| label(format!("Scroll row {row}")).height(40))
        .collect();
    column().gap(12).children((
        scroll(column().children(rows))
            .on_scroll(|offset| Msg::Scroll(offset.value()))
            .height(184),
        tabs()
            .page("General", column().child(label("General settings")))
            .page("Advanced", column().child(label("Advanced settings")))
            .on_change(Msg::Tab)
            .height(176),
        split(
            column().child(label("Left pane")),
            column().child(label("Right pane")),
        )
        .min(60, 60)
        .position(110)
        .on_moved(|position| Msg::SplitPane(position.value()))
        .height(168),
    ))
}

/// The fourth column: colour choosers, the dialog button and an icon view.
fn colors() -> Layout<Msg> {
    let palette = [
        Color::hex(0x00_78_D4),
        Color::hex(0x00_B2_94),
        Color::hex(0x10_7C_10),
        Color::hex(0xFF_B9_00),
        Color::hex(0xFF_8C_00),
        Color::hex(0xE8_11_23),
        Color::hex(0x87_64_B8),
        Color::hex(0x4C_4A_48),
    ];
    column().gap(12).children((
        color_picker(&palette)
            .columns(4)
            .selected(palette[0])
            .on_select(Msg::Swatch)
            .height(72),
        button("Show dialog").on_click(Msg::DialogOpen),
        color_panel()
            .color(Color::hex(0x87_64_B8))
            .on_change(Msg::Panel)
            .height(252),
        icon_view(&["Documents", "Pictures", "Music", "Videos"])
            .on_select(Msg::Icon)
            .fill(1),
    ))
}

/// A 16x16 two-tone artwork icon built in memory, so the gallery exercises an
/// `Image` row icon (and the backend's decoded-image cache) beside the glyphs.
fn art_image() -> Image {
    let mut pixels = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            let (r, g, b) = if (x / 4 + y / 4) % 2 == 0 {
                (0x7A, 0xC8, 0xE8)
            } else {
                (0x2A, 0x62, 0xA8)
            };
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Image::from_rgba(16, 16, pixels).expect("16x16 RGBA")
}
