//! Check/radio toggles and disabled entries.

use super::*;

#[test]
fn check_and_radio_items_toggle_and_clear_siblings() {
    let (_backend, core, ui) = setup();
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&toggles);
    let menu = Menu::context(&ui)
        .on_toggle(move |id, checked| {
            seen.borrow_mut().push((id.0, checked));
            None
        })
        .build(|m| {
            m.check(MenuId::new(1), "Auto", false);
            m.separator();
            m.radio(MenuId::new(2), "Left", true);
            m.radio(MenuId::new(3), "Right", false);
        });
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    let (row, separator, pad) = geometry(ui.dpi());

    menu.show_context(0, 0);
    runtime.deliver(popup, &down(5, pad + row / 2));
    assert!(menu.is_checked(MenuId::new(1)), "the check toggled on");

    menu.show_context(0, 0);
    let right_top = pad + row * 2 + separator;
    runtime.deliver(popup, &down(5, right_top + row / 2));
    assert!(menu.is_checked(MenuId::new(3)));
    assert!(
        !menu.is_checked(MenuId::new(2)),
        "the sibling radio cleared"
    );
    assert_eq!(*toggles.borrow(), vec![(1, true), (3, true)]);
}

#[test]
fn a_disabled_item_is_skipped_and_ignores_clicks() {
    let (_backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.item(MenuId::new(1), "One");
            m.item(MenuId::new(2), "Two");
        });
    menu.set_enabled(MenuId::new(1), false);
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    menu.show_context(0, 0);
    assert_eq!(
        menu.rt.view.borrow().levels[0].hover,
        Some(1),
        "the first selectable row skips the disabled one"
    );
    let (row, _, pad) = geometry(ui.dpi());
    runtime.deliver(popup, &down(5, pad + row / 2));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert!(log.borrow().is_empty(), "a disabled item raises nothing");
    assert!(menu.is_open(), "and does not close the menu");
}
