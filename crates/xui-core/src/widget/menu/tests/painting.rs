//! Painting popups and menus.

use super::*;

#[test]
fn a_popup_paints_its_labels() {
    let (backend, _core, ui) = setup();
    let menu = Menu::context(&ui).build(|m| {
        m.item(MenuId::new(1), "&Copy");
        m.check(MenuId::new(2), "Wrap", true);
    });
    menu.show_context(0, 0);
    let popup = menu.popup_id(0).unwrap();

    backend.render(popup);
    let ops = backend.ops(popup);
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Copy")),
        "the popup painted its label: {ops:?}"
    );
}

#[test]
fn opening_the_bar_menu_paints_its_entries() {
    let (backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(bar, &down(5, 14));
    assert!(visible(&backend, popup), "the bar menu opened");

    backend.render(popup);
    let ops = backend.ops(popup);
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "New")),
        "the bar menu painted its entries: {ops:?}"
    );
}

#[test]
fn the_first_frame_of_an_opened_menu_already_shows_its_entries() {
    let (backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(bar, &down(5, 14));

    // A native show paints synchronously, which the headless backend models,
    // so the recorded ops must already hold the entries with no explicit
    // render: the level is recorded before the popup is shown (#130).
    let ops = backend.ops(popup);
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "New")),
        "the show-time frame already paints the entries: {ops:?}"
    );
}
