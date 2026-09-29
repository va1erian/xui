//! Entry icons: the icon column, label alignment and untouched icon-free menus.

use super::*;
use crate::icon::{IconRef, Lucide};
use crate::property::{Properties, Value};
use crate::widget::menu::model::Node;

fn text_left(ops: &[DrawOp], needle: &str) -> i32 {
    ops.iter()
        .find_map(|op| match op {
            DrawOp::Text(rect, text, _) if text == needle => Some(rect.left),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text {needle:?} in {ops:?}"))
}

fn icon_strokes(ops: &[DrawOp]) -> usize {
    ops.iter()
        .filter(|op| {
            matches!(
                op,
                DrawOp::LineStroked(..) | DrawOp::StrokeEllipseStroked(..)
            )
        })
        .count()
}

fn ops_of(menu: &Menu<u32>, backend: &HeadlessBackend) -> Vec<DrawOp> {
    menu.show_context(0, 0);
    let popup = menu.popup_id(0).unwrap();
    backend.render(popup);
    backend.ops(popup)
}

#[test]
fn an_icon_column_is_drawn_and_only_replaces_the_mark_column() {
    let (backend, _core, ui) = setup();
    let plain = Menu::context(&ui).build(|m| {
        m.item(MenuId::new(1), "Copy");
    });
    let plain_ops = ops_of(&plain, &backend);
    assert_eq!(icon_strokes(&plain_ops), 0, "no icon, no icon strokes");
    let mark = layout::MARK.to_px(ui.dpi()).value();
    assert_eq!(text_left(&plain_ops, "Copy"), mark);

    let iconed = Menu::context(&ui).build(|m| {
        m.item(MenuId::new(1), "Copy").icon(Lucide::Copy);
        m.item(MenuId::new(2), "Plain");
    });
    let ops = ops_of(&iconed, &backend);
    assert!(icon_strokes(&ops) > 0, "the icon painted strokes: {ops:?}");
    // With no check or radio entry the icon column takes the mark column's
    // place, so every label stays aligned at the same offset.
    assert_eq!(text_left(&ops, "Copy"), mark);
    assert_eq!(text_left(&ops, "Plain"), mark);
}

#[test]
fn icons_and_marks_get_separate_columns() {
    let (backend, _core, ui) = setup();
    let menu = Menu::context(&ui).build(|m| {
        m.item(MenuId::new(1), "Copy").icon(Lucide::Copy);
        m.check(MenuId::new(2), "Wrap", true);
    });
    let ops = ops_of(&menu, &backend);
    let mark = layout::MARK.to_px(ui.dpi()).value();
    assert_eq!(text_left(&ops, "Copy"), mark * 2);
    assert_eq!(text_left(&ops, "Wrap"), mark * 2);
}

#[test]
fn an_icon_widens_a_popup_only_when_it_adds_a_column() {
    let nodes = |icon: bool, check: bool| {
        let mut command = Node::command(MenuId::new(1), "A fairly long label");
        if icon {
            command.icon = Some(IconRef::Lucide(Lucide::Copy));
        }
        let mut nodes = vec![command];
        if check {
            nodes.push(Node::check(MenuId::new(2), "Wrap", false));
        }
        nodes
    };
    let column = layout::MARK.to_px(96).value();
    let width = |icon, check| layout::measure(&nodes(icon, check), 96).0;
    assert_eq!(
        width(true, false),
        width(false, false),
        "icons take the mark column's place"
    );
    assert_eq!(width(true, true), width(false, true) + column);
    assert_eq!(layout::columns(&nodes(false, false), 96), (column, 0));
    assert_eq!(layout::columns(&nodes(true, false), 96), (0, column));
    assert_eq!(layout::columns(&nodes(true, true), 96), (column, column));
}

#[test]
fn an_icon_without_an_entry_is_ignored_and_bar_menus_paint() {
    let (backend, _core, ui) = setup();
    let menu = Menu::bar(&ui, Rect::new(0, 0, 260, 28))
        .unwrap()
        .build(|m| {
            m.icon(Lucide::Copy);
            m.submenu(MenuId::new(1), "File", |f| {
                f.item(MenuId::new(2), "Open").icon(Lucide::FolderOpen);
            });
        });
    let bar = menu.id().unwrap();
    backend.render(bar);
    let popup = menu.popup_id(0).unwrap();
    assert!(menu.set_property("open", Value::Bool(true)));
    backend.render(popup);
    assert!(icon_strokes(&backend.ops(popup)) > 0);
}
