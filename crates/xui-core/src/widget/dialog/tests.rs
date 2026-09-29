use std::cell::RefCell;
use std::rc::Rc;

use super::{Dialog, DialogAction};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, NodeKind, PlatformSpec, WidgetId};
use crate::message::{Key, Modifiers};

struct TestApp {
    log: Rc<RefCell<Vec<u32>>>,
}

impl App for TestApp {
    type Msg = u32;

    fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
        self.log.borrow_mut().push(msg);
    }
}

fn setup() -> (Rc<HeadlessBackend>, Rc<Core<u32>>, Ui<u32>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("dialog")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn key(key: Key) -> Event {
    Event::KeyDown {
        key,
        modifiers: Modifiers::NONE,
        repeat: 1,
        system: false,
    }
}

fn runtime(core: Rc<Core<u32>>, log: &Rc<RefCell<Vec<u32>>>) -> Rc<Runtime<TestApp>> {
    Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(log),
        },
    )
}

#[test]
fn a_dialog_opens_over_the_whole_window() {
    let (backend, _core, ui) = setup();
    let dialog = Dialog::message(&ui, "Saved", "Your changes were saved.").unwrap();
    assert!(!dialog.is_open());

    dialog.open();

    let client = ui.client_rect();
    let (kind, bounds, _, visible, _) = backend.node(dialog.id()).unwrap();
    assert_eq!(kind, NodeKind::Custom);
    assert_eq!(bounds, client, "the scrim covers the whole client area");
    assert!(visible, "the open dialog is shown");
    assert!(dialog.is_open());
}

#[test]
fn enter_accepts_a_message_dialog() {
    let (_backend, core, ui) = setup();
    let dialog = Dialog::message(&ui, "Saved", "Your changes were saved.")
        .unwrap()
        .on_action(|action| match action {
            DialogAction::Accept(_) => Some(1),
            DialogAction::Cancel => Some(2),
        });
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);
    dialog.open();

    runtime.deliver(dialog.id(), &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![1]);
    assert!(!dialog.is_open(), "the dialog closed on accept");
}

#[test]
fn escape_cancels_and_a_closed_dialog_ignores_keys() {
    let (_backend, core, ui) = setup();
    let dialog = Dialog::confirm(&ui, "Delete?", "This cannot be undone.")
        .unwrap()
        .on_action(|action| match action {
            DialogAction::Accept(_) => Some(1),
            DialogAction::Cancel => Some(2),
        });
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);
    dialog.open();

    runtime.deliver(dialog.id(), &key(Key::ESCAPE));
    runtime.deliver(dialog.id(), &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(
        *log.borrow(),
        vec![2],
        "Escape cancels, the later Enter is ignored"
    );
}

#[test]
fn a_prompt_returns_the_entered_text() {
    let (_backend, core, ui) = setup();
    let dialog = Dialog::prompt(&ui, "Rename", "New name:", "seed")
        .unwrap()
        .on_action(|action| match action {
            DialogAction::Accept(text) => Some(text.len() as u32),
            DialogAction::Cancel => Some(0),
        });
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);
    dialog.open();

    assert_eq!(dialog.text(), "seed");
    runtime.deliver(dialog.id(), &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(
        *log.borrow(),
        vec![4],
        "the entered text reached the mapper"
    );
}

#[test]
fn closing_hides_the_dialog_without_an_action() {
    let (backend, _core, ui) = setup();
    let dialog = Dialog::confirm(&ui, "Discard?", "The draft will be lost.").unwrap();
    dialog.open();
    dialog.close();

    let (_, _, _, visible, _) = backend.node(dialog.id()).unwrap();
    assert!(!visible, "a closed dialog is hidden");
    assert!(!dialog.is_open());
}

#[test]
fn a_dialog_paints_its_card_title_and_message() {
    let (backend, _core, ui) = setup();
    let dialog = Dialog::message(&ui, "Saved", "Your changes were saved.").unwrap();
    dialog.open();
    let theme = ui.theme();

    backend.render(dialog.id());
    let ops = backend.ops(dialog.id());
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Rounded(..))),
        "the card was painted: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Saved")),
        "the title was painted: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Your changes were saved.")),
        "the message was painted: {ops:?}"
    );
    assert!(
        ops.iter().any(
            |op| matches!(op, DrawOp::StrokeRounded(_, _, color, _) if *color == theme.border)
        ),
        "the card has a border: {ops:?}"
    );
}

#[test]
fn the_scrim_is_a_translucent_overlay_not_an_opaque_clear() {
    let (backend, _core, ui) = setup();
    let dialog = Dialog::message(&ui, "Saved", "Your changes were saved.").unwrap();
    dialog.open();
    let theme = ui.theme();
    let client = ui.client_rect();

    backend.render(dialog.id());
    let ops = backend.ops(dialog.id());

    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::FillRgba(rect, color) if *rect == client && *color == theme.scrim)),
        "the scrim is one translucent fill over the whole client: {ops:?}"
    );
    assert!(
        !ops.iter().any(|op| matches!(op, DrawOp::Clear(_))),
        "the scrim must blend over the content, not clear it: {ops:?}"
    );
    assert!(
        theme.scrim.a > 0 && theme.scrim.a < 255,
        "the scrim token must carry partial alpha: {:?}",
        theme.scrim
    );
}
