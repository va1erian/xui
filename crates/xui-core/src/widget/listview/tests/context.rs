//! The context hook: right clicks and the Menu key.

use super::*;

#[test]
fn a_right_click_calls_the_context_hook() {
    let (_backend, core, ui) = setup();
    let log = log();
    let list = ListView::new(&ui, Rect::new(0, 0, 200, 200), &["a", "b", "c"])
        .unwrap()
        .on_context(|row, _at| Some(100 + row as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(list.id(), &right(5, row_y(1)));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![101]);
}

#[test]
fn a_right_click_reports_the_pointer_position() {
    let (_backend, core, ui) = setup();
    let at = Rc::new(RefCell::new(None));
    let seen = Rc::clone(&at);
    let list = ListView::new(&ui, Rect::new(40, 200, 240, 400), &["a", "b", "c"])
        .unwrap()
        .on_context(move |_row, point| {
            *seen.borrow_mut() = Some(point);
            Some(1)
        });
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(list.id(), &right(37, row_y(2)));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*at.borrow(), Some(Point::new(37, row_y(2))));
}

#[test]
fn the_menu_key_anchors_at_the_focused_row() {
    let (_backend, core, ui) = setup();
    let at = Rc::new(RefCell::new(None));
    let seen = Rc::clone(&at);
    let list = ListView::new(&ui, Rect::new(0, 0, 200, 200), &["a", "b", "c"])
        .unwrap()
        .on_context(move |_row, point| {
            *seen.borrow_mut() = Some(point);
            Some(1)
        });
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(list.id(), &key(Key::DOWN));
    runtime.deliver(list.id(), &key(Key::MENU));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    // Row 1 is the second 22px row; the menu opens at its bottom-left.
    assert_eq!(*at.borrow(), Some(Point::new(0, 2 * ROW.to_px(96).value())));
}
