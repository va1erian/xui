//! [`Edit::password`]: the mask, its geometry, and the clipboard and
//! automation paths a secret must not leak through.

use super::*;
use crate::theme::Theme;

/// The secret the tests type; its multi-byte chars are one bullet each.
const SECRET: &str = "pässwörd 1";
const BULLET: char = '\u{2022}';

/// A headless character is 6px wide and the padding is 4px, so text position
/// `n` sits at x = 4 + 6n.
fn x_of(position: i32) -> i32 {
    4 + 6 * position
}

fn runtime_for(core: Rc<Core<u32>>, log: &Rc<RefCell<Vec<u32>>>) -> Rc<Runtime<TestApp>> {
    Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(log),
        },
    )
}

fn bullets(count: usize) -> String {
    std::iter::repeat_n(BULLET, count).collect()
}

fn texts(backend: &HeadlessBackend, id: WidgetId) -> Vec<String> {
    backend.render(id);
    backend
        .ops(id)
        .into_iter()
        .filter_map(|op| match op {
            DrawOp::Text(_, text, _) => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn a_password_edit_paints_one_bullet_per_char_and_never_the_text() {
    let (backend, _core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), SECRET)
        .unwrap()
        .password(true);

    assert!(edit.is_password());
    assert_eq!(
        texts(&backend, edit.id()),
        vec![bullets(SECRET.chars().count())]
    );
    assert_eq!(edit.text(), SECRET, "the app still reads the real text");

    let edit = edit.password(false);
    assert_eq!(texts(&backend, edit.id()), vec![SECRET.to_string()]);
}

#[test]
fn a_password_edit_shows_its_cue_while_empty() {
    let (backend, _core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "")
        .unwrap()
        .password(true)
        .cue("Password");

    assert_eq!(texts(&backend, edit.id()), vec!["Password".to_string()]);
}

#[test]
fn the_caret_and_selection_line_up_with_the_bullets() {
    let (backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "aé😀d")
        .unwrap()
        .password(true);
    let runtime = runtime_for(core, &Rc::new(RefCell::new(Vec::new())));

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::HOME));
    runtime.deliver(edit.id(), &key_with(Key::RIGHT, shift()));
    runtime.deliver(edit.id(), &key_with(Key::RIGHT, shift()));
    backend.render(edit.id());
    let ops = backend.ops(edit.id());

    let selection = Theme::light().selection;
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Fill(rect, color)
            if *color == selection && rect.left == x_of(0) && rect.right == x_of(2))),
        "the selection covers exactly two bullets: {ops:?}"
    );
    assert!(
        ops.iter().any(
            |op| matches!(op, DrawOp::Line(from, to, ..) if from.x == x_of(2) && to.x == x_of(2))
        ),
        "the caret sits after the second bullet: {ops:?}"
    );
}

#[test]
fn a_click_on_a_password_edit_hits_the_bullet_under_it() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "aé😀d")
        .unwrap()
        .password(true);
    let runtime = runtime_for(core, &Rc::new(RefCell::new(Vec::new())));

    runtime.deliver(
        edit.id(),
        &Event::MouseDown {
            x: x_of(3),
            y: 5,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(edit.id(), &Event::Char('X'));
    assert_eq!(edit.text(), "aé😀Xd");

    runtime.deliver(
        edit.id(),
        &Event::MouseDoubleClick {
            x: x_of(1),
            y: 5,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(edit.id(), &Event::Char('Z'));
    assert_eq!(edit.text(), "Z", "a double-click selects the whole secret");
}

#[test]
fn copy_and_cut_are_refused_and_paste_and_undo_work() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), SECRET)
        .unwrap()
        .password(true)
        .on_change(|text| Some(text.chars().count() as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime_for(core, &log);
    ui.set_clipboard_text("clip");

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key_with(Key::A, ctrl()));
    runtime.deliver(edit.id(), &key_with(Key::C, ctrl()));
    runtime.deliver(edit.id(), &key_with(Key::X, ctrl()));
    runtime.deliver(edit.id(), &key_with(Key::DELETE, shift()));
    assert_eq!(ui.clipboard_text().as_deref(), Some("clip"));
    assert_eq!(edit.text(), SECRET, "a refused cut leaves the text");

    runtime.deliver(edit.id(), &key_with(Key::V, ctrl()));
    assert_eq!(edit.text(), "clip", "paste replaces the selection");
    runtime.deliver(edit.id(), &key_with(Key::Z, ctrl()));
    assert_eq!(edit.text(), SECRET, "undo restores the secret");
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![4, 10], "on_change saw the real text");
}

#[test]
fn word_keys_treat_a_password_as_one_word() {
    let (_backend, core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), "two words")
        .unwrap()
        .password(true);
    let runtime = runtime_for(core, &Rc::new(RefCell::new(Vec::new())));

    runtime.deliver(edit.id(), &Event::SetFocus);
    runtime.deliver(edit.id(), &key(Key::HOME));
    runtime.deliver(edit.id(), &key_with(Key::RIGHT, ctrl()));
    runtime.deliver(edit.id(), &Event::Char('X'));
    assert_eq!(edit.text(), "two wordsX", "Ctrl+Right skips the space");

    runtime.deliver(edit.id(), &key_with(Key::LEFT, ctrl()));
    runtime.deliver(edit.id(), &key_with(Key::DELETE, ctrl()));
    assert_eq!(edit.text(), "", "Ctrl+Delete removes to the end");

    edit.set_text("two words");
    runtime.deliver(edit.id(), &key_with(Key::BACK, ctrl()));
    assert_eq!(edit.text(), "", "Ctrl+Backspace removes to the start");
}

#[test]
fn a_password_edit_reports_bullets_as_its_text_property() {
    let (_backend, _core, ui) = setup();
    let edit = Edit::new(&ui, Rect::new(0, 0, 160, 28), SECRET)
        .unwrap()
        .password(true);

    assert_eq!(
        edit.property("text"),
        Some(Value::Text(bullets(SECRET.chars().count())))
    );
    assert!(edit.set_property("text", Value::Text("new".into())));
    assert_eq!(edit.text(), "new", "setting the property still works");
}
