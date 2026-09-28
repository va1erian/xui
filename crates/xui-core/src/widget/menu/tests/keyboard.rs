//! Keyboard navigation and mnemonics.

use super::*;

#[test]
fn keyboard_navigation_moves_and_activates() {
    let (_backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.item(MenuId::new(7), "One");
            m.item(MenuId::new(8), "Two");
            m.item(MenuId::new(9), "Three");
        });
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    menu.show_context(0, 0);
    runtime.deliver(popup, &key(Key::DOWN));
    assert_eq!(
        menu.rt.view.borrow().levels[0].hover,
        Some(1),
        "the arrow moves off the initially highlighted row"
    );
    runtime.deliver(popup, &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![8]);
    assert!(!menu.is_open());
}

#[test]
fn a_mnemonic_letter_activates_its_item() {
    let (_backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.item(MenuId::new(7), "&Copy");
            m.item(MenuId::new(8), "&Paste");
        });
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    menu.show_context(0, 0);
    runtime.deliver(popup, &Event::Char('p'));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![8]);
}
