//! The gallery's menu bar and context menu, and the top bar's commands.

use xui::prelude::*;
use xui_core::widget::Glyph;

use crate::Msg;

const FILE: MenuId = MenuId::new(0);
const NEW: MenuId = MenuId::new(1);
const OPEN: MenuId = MenuId::new(2);
const AUTO_SAVE: MenuId = MenuId::new(3);
const RECENT: MenuId = MenuId::new(4);
const EDIT: MenuId = MenuId::new(7);
/// The bar's `Undo` item, which the gallery disables.
pub const UNDO: MenuId = MenuId::new(10);
const REDO: MenuId = MenuId::new(11);
const LEFT: MenuId = MenuId::new(12);
const RIGHT: MenuId = MenuId::new(13);

const CUT: MenuId = MenuId::new(20);
const COPY: MenuId = MenuId::new(21);
const PASTE: MenuId = MenuId::new(22);
const WRAP: MenuId = MenuId::new(23);
const MORE: MenuId = MenuId::new(24);

const BAR_NEW: TopBarId = TopBarId::new(1);
const BAR_STAR: TopBarId = TopBarId::new(2);
const BAR_VOLUME: TopBarId = TopBarId::new(3);
const BAR_SEARCH: TopBarId = TopBarId::new(4);
const BAR_LABEL: TopBarId = TopBarId::new(5);
const BAR_PLAY: TopBarId = TopBarId::new(6);

/// The `File`/`Edit` menu bar.
pub fn bar() -> Build<Menu<Msg>, Msg> {
    menu_bar(|m| {
        m.submenu(FILE, "&File", |f| {
            f.item(NEW, "&New");
            f.item(OPEN, "&Open");
            f.separator();
            f.check(AUTO_SAVE, "Auto &Save", true);
            f.separator();
            f.submenu(RECENT, "&Recent", |r| {
                r.item(MenuId::new(5), "Report 1");
                r.item(MenuId::new(6), "Report 2");
            });
        });
        m.submenu(EDIT, "&Edit", |e| {
            e.item(UNDO, "&Undo");
            e.item(REDO, "&Redo");
            e.separator();
            e.radio(LEFT, "Align &Left", true);
            e.radio(RIGHT, "Align &Right", false);
        });
    })
    .on_select(|id| {
        Msg::BarMenu(match id {
            NEW => "new",
            OPEN => "open",
            RECENT => "recent",
            UNDO => "undo",
            REDO => "redo",
            _ => "command",
        })
    })
    .on_toggle(|id, checked| {
        let name = match id {
            AUTO_SAVE => "auto save",
            LEFT => "left",
            RIGHT => "right",
            _ => "toggle",
        };
        Msg::BarToggle(name, checked)
    })
}

/// The menu the `Context menu` button pops up.
pub fn context(ui: &Ui<Msg>) -> Menu<Msg> {
    Menu::context(ui)
        .on_select(|id| {
            Some(Msg::BarMenu(match id {
                CUT => "cut",
                COPY => "copy",
                PASTE => "paste",
                MORE => "more",
                _ => "command",
            }))
        })
        .on_toggle(|id, checked| (id == WRAP).then_some(Msg::BarToggle("word wrap", checked)))
        .build(|m| {
            m.item(CUT, "Cu&t");
            m.item(COPY, "&Copy");
            m.item(PASTE, "&Paste");
            m.separator();
            m.check(WRAP, "&Word wrap", false);
            m.separator();
            m.submenu(MORE, "&More", |s| {
                s.item(MenuId::new(25), "Item &A");
                s.item(MenuId::new(26), "Item &B");
            });
        })
}

/// The material band: icon buttons, a toggle, a label and a volume slider.
pub fn top() -> Build<TopBar<Msg>, Msg> {
    top_bar().then(|bar| {
        bar.icon(BAR_NEW, Glyph::Menu)
            .icon(BAR_PLAY, Glyph::Play)
            .toggle(BAR_STAR, Glyph::Star)
            .label(BAR_LABEL, "xui")
            .slider(BAR_VOLUME, 0.0, 100.0)
            .expand(BAR_VOLUME)
            .icon(BAR_SEARCH, Glyph::Search)
            .on_click(|id| match id {
                BAR_NEW => Some(Msg::TopBarNew),
                BAR_SEARCH => Some(Msg::TopBarSearch),
                BAR_PLAY => Some(Msg::TopBarPlay),
                _ => None,
            })
            .on_toggle(|id, checked| (id == BAR_STAR).then_some(Msg::TopBarStar(checked)))
            .on_change(|id, value| (id == BAR_VOLUME).then_some(Msg::TopBarVolume(value)))
    })
}
