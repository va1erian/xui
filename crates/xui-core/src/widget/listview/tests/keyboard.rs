//! Keyboard navigation.

use super::*;

#[test]
fn the_keyboard_navigates_and_space_toggles_in_multi() {
    let (_backend, core, ui) = setup();
    let log = log();
    let list = ListView::new(&ui, Rect::new(0, 0, 200, 200), &["a", "b", "c", "d", "e"])
        .unwrap()
        .selection_mode(SelectionMode::Multi)
        .on_selection(|rows| Some(rows.len() as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(list.id(), &key(Key::DOWN));
    assert_eq!(list.focused(), Some(1));
    assert_eq!(list.selection(), vec![1]);
    runtime.deliver(list.id(), &key(Key::SPACE));
    assert_eq!(list.selection(), vec![]);
    runtime.deliver(list.id(), &key(Key::SPACE));
    assert_eq!(list.selection(), vec![1]);
    runtime.deliver(list.id(), &key(Key::END));
    assert_eq!(list.selection(), vec![4]);
    runtime.deliver(list.id(), &key(Key::HOME));
    assert_eq!(list.selection(), vec![0]);
    runtime.deliver(list.id(), &key(Key::PAGE_DOWN));
    assert_eq!(list.selection(), vec![4]);
    runtime.deliver(list.id(), &key(Key::UP));
    assert_eq!(list.selection(), vec![3]);
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert!(!log.borrow().is_empty());
}
