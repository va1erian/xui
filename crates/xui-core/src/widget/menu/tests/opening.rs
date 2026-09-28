//! Opening and closing the bar, context popup and submenus.

use super::*;

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
