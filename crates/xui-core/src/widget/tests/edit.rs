use super::*;

#[test]
fn typing_into_an_edit_changes_it_and_maps_a_message() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "")
        .unwrap()
        .on_change(|text| Some(text.len() as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &Event::Char('a'));
    runtime.deliver(edit.id(), &Event::Char('b'));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(edit.text(), "ab");
    assert_eq!(*log.borrow(), vec![1, 2]);
}

#[test]
fn editing_keys_move_the_caret_and_delete() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "abc").unwrap();
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::HOME));
    runtime.deliver(edit.id(), &key(Key::DELETE));
    assert_eq!(
        edit.text(),
        "bc",
        "Delete removes the character at the caret"
    );

    runtime.deliver(edit.id(), &key(Key::END));
    runtime.deliver(edit.id(), &key(Key::BACK));
    assert_eq!(edit.text(), "b", "Backspace removes before the caret");
}

#[test]
fn an_edit_paints_its_field_and_text() {
    let (backend, _core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "typed").unwrap();

    backend.render(edit.id());
    let ops = backend.ops(edit.id());
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Stroke(..))),
        "a field border was painted: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "typed")),
        "the text was painted: {ops:?}"
    );
}
