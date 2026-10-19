//! Opening and closing the bar, context popup and submenus.

use super::*;
use crate::backend::{NodeKind, NodeSpec};
use crate::widget::control::Control;

#[test]
fn a_bar_click_opens_its_menu_and_escape_closes_it() {
    let (backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    assert!(!visible(&backend, popup), "the popup starts hidden");
    runtime.deliver(bar, &down(5, 14));
    assert!(menu.is_open(), "clicking the title opens its menu");
    assert!(visible(&backend, popup));

    runtime.deliver(popup, &key(Key::ESCAPE));
    assert!(!menu.is_open(), "Escape closes it");
    assert!(!visible(&backend, popup));
}

#[test]
fn a_context_menu_opens_at_a_point_and_maps_its_selection() {
    let (backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.item(MenuId::new(7), "&Copy");
            m.item(MenuId::new(8), "&Paste");
        });
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    menu.show_context(10, 20);
    assert!(menu.is_open());
    assert!(visible(&backend, popup));

    let (row, _, pad) = geometry(ui.dpi());
    runtime.deliver(popup, &down(5, pad + row + row / 2));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![8], "the second row raised its id");
    assert!(!menu.is_open(), "choosing a command closes the menu");
}

#[test]
fn a_submenu_opens_sideways_and_selects_a_nested_item() {
    let (backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.submenu(MenuId::new(1), "&More", |s| {
                s.item(MenuId::new(2), "&Deep");
            });
        });
    let outer = menu.popup_id(0).unwrap();
    let inner = menu.popup_id(1).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    menu.show_context(0, 0);
    assert!(!visible(&backend, inner), "the submenu starts hidden");
    runtime.deliver(outer, &key(Key::RIGHT));
    assert!(visible(&backend, inner), "Right opens the submenu");

    runtime.deliver(inner, &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![2]);
    assert!(!menu.is_open());
}

#[test]
fn leaving_a_submenu_returns_focus_to_its_parent() {
    let (backend, core, ui) = setup();
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.submenu(MenuId::new(1), "&More", |s| {
                s.item(MenuId::new(2), "&Deep");
            });
            m.item(MenuId::new(3), "Plain");
        });
    let popup = menu.popup_id(0).unwrap();
    let submenu = menu.popup_id(1).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    let (row, _, pad) = geometry(ui.dpi());

    menu.show_context(0, 0);
    assert_eq!(backend.focused(), Some(popup), "the root popup holds focus");

    // Hovering the submenu row opens it and moves the focus to it.
    runtime.deliver(
        popup,
        &Event::MouseMove {
            x: 5,
            y: pad + row / 2,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(backend.focused(), Some(submenu), "the submenu took focus");

    // Hovering a plain row closes the submenu; the focus must come back to the
    // parent, or the open menu stops receiving keys on a native backend.
    runtime.deliver(
        popup,
        &Event::MouseMove {
            x: 5,
            y: pad + row + row / 2,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(
        backend.focused(),
        Some(popup),
        "closing the submenu returned focus to its parent"
    );
}

/// A node outside the menu, to hand the focus to.
fn outside(ui: &Ui<u32>) -> Control<u32> {
    Control::new(
        ui,
        &NodeSpec::new(NodeKind::Custom, Rect::new(0, 100, 50, 130)),
    )
    .unwrap()
}

#[test]
fn losing_the_focus_to_something_outside_closes_the_menu() {
    let (backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let other = outside(&ui);
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(bar, &down(5, 14));
    assert!(menu.is_open());

    // A press elsewhere moves the focus off the open popup.
    ui.focus(other.id());
    runtime.deliver(popup, &Event::KillFocus);
    assert!(!menu.is_open(), "a click outside dismisses the menu");
    assert!(!visible(&backend, popup));
}

#[test]
fn a_kill_focus_whose_new_holder_is_the_menu_itself_is_ignored() {
    let (_backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(bar, &down(5, 14));
    // The backend still reports the popup as the holder.
    runtime.deliver(popup, &Event::KillFocus);
    assert!(
        menu.is_open(),
        "a KillFocus whose new holder is the menu itself is not a dismissal"
    );
}

#[test]
fn a_context_menu_closes_when_the_focus_leaves_it() {
    let (backend, core, ui) = setup();
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.item(MenuId::new(7), "&Copy");
        });
    let popup = menu.popup_id(0).unwrap();
    let other = outside(&ui);
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    menu.show_context(10, 20);
    ui.focus(other.id());
    runtime.deliver(popup, &Event::KillFocus);

    assert!(!menu.is_open());
    assert!(!visible(&backend, popup));
}

#[test]
fn losing_the_focus_closes_every_open_submenu_level() {
    let (backend, core, ui) = setup();
    let menu = Menu::context(&ui).build(|m| {
        m.submenu(MenuId::new(1), "&More", |s| {
            s.item(MenuId::new(2), "&Deep");
        });
    });
    let outer = menu.popup_id(0).unwrap();
    let inner = menu.popup_id(1).unwrap();
    let other = outside(&ui);
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    menu.show_context(0, 0);
    runtime.deliver(outer, &key(Key::RIGHT));
    assert!(visible(&backend, inner));

    ui.focus(other.id());
    runtime.deliver(inner, &Event::KillFocus);
    assert!(!menu.is_open());
    assert!(!visible(&backend, outer), "the parent level closed too");
    assert!(!visible(&backend, inner));
}

#[test]
fn the_focus_moving_to_a_parent_level_does_not_close_the_menu() {
    let (backend, core, ui) = setup();
    let menu = Menu::context(&ui).build(|m| {
        m.submenu(MenuId::new(1), "&More", |s| {
            s.item(MenuId::new(2), "&Deep");
        });
    });
    let outer = menu.popup_id(0).unwrap();
    let inner = menu.popup_id(1).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    menu.show_context(0, 0);
    runtime.deliver(outer, &key(Key::RIGHT));

    // A press on the parent popup takes the focus from the submenu popup.
    ui.focus(outer);
    runtime.deliver(inner, &Event::KillFocus);
    assert!(menu.is_open(), "the focus stayed inside the menu");
    assert!(visible(&backend, inner));
}

#[test]
fn pressing_the_open_title_closes_its_menu_instead_of_reopening_it() {
    let (_backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(bar, &down(5, 14));
    assert!(menu.is_open());

    // The window system focuses the bar before it sees the press, which takes
    // the focus off the popup but keeps it inside the menu; the press then
    // lands on the open title.
    ui.focus(bar);
    runtime.deliver(popup, &Event::KillFocus);
    assert!(
        menu.is_open(),
        "moving the focus to the bar is not a dismissal"
    );
    runtime.deliver(bar, &down(5, 14));
    assert!(!menu.is_open(), "the second press on the title closes it");

    // The next press opens the menu again.
    runtime.deliver(bar, &down(5, 14));
    assert!(menu.is_open());
}

#[test]
fn a_press_on_another_title_switches_to_its_menu() {
    let (_backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    let edit = layout::title_rect(
        &menu.rt.model.borrow(),
        crate::geometry::Point::new(0, 0),
        28,
        ui.dpi(),
        1,
    )
    .unwrap();

    runtime.deliver(bar, &down(5, 14));
    ui.focus(bar);
    runtime.deliver(popup, &Event::KillFocus);
    runtime.deliver(bar, &down(edit.left + 3, 14));
    assert!(menu.is_open(), "the other title's menu opened");
    assert_eq!(menu.rt.view.borrow().bar_open, Some(1));
}

#[test]
fn pressing_the_bars_empty_area_closes_the_open_menu() {
    let (_backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(bar, &down(5, 14));
    assert!(menu.is_open());
    runtime.deliver(bar, &down(250, 14));
    assert!(
        !menu.is_open(),
        "the empty part of the bar is outside the menu"
    );
}
