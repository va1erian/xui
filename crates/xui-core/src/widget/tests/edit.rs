use super::*;
use crate::theme::Theme;

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

fn runtime_for(core: Rc<Core<u32>>, log: &Rc<RefCell<Vec<u32>>>) -> Rc<Runtime<TestApp>> {
    Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(log),
        },
    )
}

#[test]
fn shift_arrows_extend_a_selection_and_typing_replaces_it() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "hello").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::HOME));
    runtime.deliver(edit.id(), &key_with(Key::RIGHT, shift()));
    runtime.deliver(edit.id(), &key_with(Key::RIGHT, shift()));
    runtime.deliver(edit.id(), &Event::Char('X'));
    assert_eq!(edit.text(), "Xllo", "typing replaces the selected `he`");
}

#[test]
fn control_arrows_move_by_word_and_shift_extends_by_word() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "hello brave world").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::HOME));
    runtime.deliver(edit.id(), &key_with(Key::RIGHT, ctrl()));
    runtime.deliver(edit.id(), &Event::Char('X'));
    assert_eq!(
        edit.text(),
        "helloX brave world",
        "Ctrl+Right lands at the end of `hello`"
    );

    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "hello brave world").unwrap();
    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::HOME));
    runtime.deliver(
        edit.id(),
        &key_with(
            Key::RIGHT,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        ),
    );
    runtime.deliver(edit.id(), &Event::Char('X'));
    assert_eq!(
        edit.text(),
        "X brave world",
        "Ctrl+Shift+Right selects the word"
    );
}

#[test]
fn control_backspace_and_delete_remove_a_word() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "hello brave").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::END));
    runtime.deliver(edit.id(), &key_with(Key::BACK, ctrl()));
    assert_eq!(edit.text(), "hello ");

    runtime.deliver(edit.id(), &key(Key::HOME));
    runtime.deliver(edit.id(), &key_with(Key::DELETE, ctrl()));
    assert_eq!(
        edit.text(),
        " ",
        "Ctrl+Delete removes the first word, leaving its space"
    );
}

#[test]
fn control_a_selects_all_and_typing_overwrites() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "hello").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::A));
    runtime.deliver(edit.id(), &Event::Char('y'));
    assert_eq!(
        edit.text(),
        "helloy",
        "a bare `a` types; Ctrl+A selects all first"
    );

    runtime.deliver(edit.id(), &key_with(Key::A, ctrl()));
    runtime.deliver(edit.id(), &Event::Char('z'));
    assert_eq!(edit.text(), "z");
}

#[test]
fn undo_groups_a_typing_run_and_redo_replays_it() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    runtime.deliver(edit.id(), &Event::SetFocus);
    for character in ['a', 'b', 'c'] {
        runtime.deliver(edit.id(), &Event::Char(character));
    }
    assert_eq!(edit.text(), "abc");

    runtime.deliver(edit.id(), &key_with(Key::Z, ctrl()));
    assert_eq!(edit.text(), "", "one Ctrl+Z undoes the whole run");
    runtime.deliver(edit.id(), &key_with(Key::Y, ctrl()));
    assert_eq!(edit.text(), "abc");
}

#[test]
fn copy_and_paste_round_trip_through_the_in_process_clipboard() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "abc").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key_with(Key::A, ctrl()));
    runtime.deliver(edit.id(), &key_with(Key::C, ctrl()));
    runtime.deliver(edit.id(), &key(Key::DELETE));
    assert_eq!(edit.text(), "", "the copied text is deleted");

    runtime.deliver(edit.id(), &key_with(Key::V, ctrl()));
    assert_eq!(edit.text(), "abc", "paste restores what was copied");
}

#[test]
fn shift_delete_cuts_and_shift_insert_pastes() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "cut me").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key_with(Key::A, ctrl()));
    runtime.deliver(edit.id(), &key_with(Key::DELETE, shift()));
    assert_eq!(edit.text(), "", "Shift+Delete cuts the selection");

    runtime.deliver(edit.id(), &key_with(Key::INSERT, shift()));
    assert_eq!(edit.text(), "cut me", "Shift+Insert pastes it back");
}

#[test]
fn a_mouse_drag_selects_text() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "hello world").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    // Padding is 4px and a headless character is 6px wide, so x=4 is text
    // position 0 and x=34 is position 5.
    runtime.deliver(
        edit.id(),
        &Event::MouseDown {
            x: 4,
            y: 5,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(
        edit.id(),
        &Event::MouseMove {
            x: 34,
            y: 5,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(
        edit.id(),
        &Event::MouseUp {
            x: 34,
            y: 5,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(edit.id(), &Event::Char('X'));
    assert_eq!(edit.text(), "X world", "the drag selected `hello`");
}

#[test]
fn a_double_click_selects_a_word() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "hello world").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(
        edit.id(),
        &Event::MouseDoubleClick {
            x: 52,
            y: 5,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(edit.id(), &Event::Char('X'));
    assert_eq!(edit.text(), "hello X", "the double-click selected `world`");
}

#[test]
fn a_selection_is_painted_with_the_theme_token_in_both_themes() {
    let (backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "hello").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::HOME));
    runtime.deliver(edit.id(), &key_with(Key::END, shift()));

    let light = Theme::light().selection;
    backend.render(edit.id());
    assert!(
        backend
            .ops(edit.id())
            .iter()
            .any(|op| matches!(op, DrawOp::Fill(_, color) if *color == light)),
        "the light selection highlight is painted"
    );

    ui.set_theme(Theme::dark());
    let dark = Theme::dark().selection;
    backend.render(edit.id());
    assert!(
        backend
            .ops(edit.id())
            .iter()
            .any(|op| matches!(op, DrawOp::Fill(_, color) if *color == dark)),
        "the dark selection highlight is painted"
    );
}

#[test]
fn the_text_scrolls_so_the_caret_stays_visible() {
    let (backend, core, ui) = setup();
    let text = "0123456789abcdefghijklmnopqrstuvwxyz";
    let edit = Edit::new(&ui, Rect::new(0, 0, 60, 28), text).unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::END));
    backend.render(edit.id());

    let left = backend
        .ops(edit.id())
        .iter()
        .find_map(|op| match op {
            DrawOp::Text(rect, drawn, _) if drawn == text => Some(rect.left),
            _ => None,
        })
        .expect("the text is painted");
    assert!(
        left < 4,
        "the text is scrolled left of the padding, at {left}"
    );
}

#[test]
fn a_degenerate_field_paints_without_panicking() {
    let (backend, _core, ui) = setup();

    for bounds in [
        Rect::new(0, 0, 4, 4),
        Rect::new(10, 10, 0, 0),
        Rect::new(0, 0, 0, 28),
    ] {
        let edit = Edit::new(&ui, bounds, "some text that cannot fit").unwrap();
        backend.render(edit.id());
    }
}
