use std::cell::RefCell;
use std::rc::Rc;

use super::{TaskDialog, TaskDialogAction, TaskDialogIcon};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, NodeKind, PlatformSpec, WidgetId};
use crate::message::{Key, Modifiers};
use crate::units::Dip;

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
    setup_sized(Dip(640.0), Dip(480.0))
}

fn setup_sized(width: Dip, height: Dip) -> (Rc<HeadlessBackend>, Rc<Core<u32>>, Ui<u32>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend
        .open_window(&PlatformSpec::new("task dialog").size(width, height))
        .unwrap();
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
fn enter_picks_the_first_command() {
    let (_backend, core, ui) = setup();
    let dialog = TaskDialog::new(&ui, "Delete?", "This cannot be undone.")
        .unwrap()
        .icon(TaskDialogIcon::Warning)
        .command("Delete")
        .unwrap()
        .command("Keep")
        .unwrap()
        .verification("Don't ask again")
        .unwrap()
        .on_action(|action| match action {
            TaskDialogAction::Command(index) => Some(index as u32 + 10),
            TaskDialogAction::Cancel => Some(99),
        });
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);
    dialog.open();

    runtime.deliver(dialog.id(), &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![10], "Enter picks command 0");
    assert!(!dialog.is_open(), "the dialog closed on the command");
}

#[test]
fn escape_cancels_and_the_verification_state_is_exposed() {
    let (_backend, core, ui) = setup();
    let dialog = TaskDialog::new(&ui, "Confirm", "Go on?")
        .unwrap()
        .command("Go")
        .unwrap()
        .verification("Remember my choice")
        .unwrap()
        .on_action(|action| match action {
            TaskDialogAction::Command(_) => Some(1),
            TaskDialogAction::Cancel => Some(2),
        });
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);
    dialog.open();

    assert!(!dialog.is_checked(), "the checkbox starts unchecked");
    dialog.set_checked(true);
    assert!(dialog.is_checked());

    runtime.deliver(dialog.id(), &key(Key::ESCAPE));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![2], "Escape cancels");
    assert!(dialog.is_checked(), "the choice survives the dismissal");
}

#[test]
fn a_task_dialog_opens_over_the_whole_window() {
    let (backend, _core, ui) = setup();
    let dialog = TaskDialog::new(&ui, "Title", "Message")
        .unwrap()
        .command("OK")
        .unwrap();
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
fn a_task_dialog_paints_its_icon_title_and_message() {
    let (backend, _core, ui) = setup();
    let dialog = TaskDialog::new(&ui, "Delete?", "This cannot be undone.")
        .unwrap()
        .icon(TaskDialogIcon::Warning)
        .command("Delete")
        .unwrap();
    dialog.open();

    backend.render(dialog.id());
    let ops = backend.ops(dialog.id());
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Rounded(..))),
        "the card was painted: {ops:?}"
    );
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Polygon(..))),
        "the warning icon was painted: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Delete?")),
        "the title was painted: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "This cannot be undone.")),
        "the message was painted: {ops:?}"
    );
}

#[test]
fn every_icon_variant_paints_a_glyph() {
    for icon in [
        TaskDialogIcon::Info,
        TaskDialogIcon::Warning,
        TaskDialogIcon::Error,
        TaskDialogIcon::Shield,
    ] {
        let (backend, _core, ui) = setup();
        let dialog = TaskDialog::new(&ui, "Title", "Message")
            .unwrap()
            .icon(icon)
            .command("OK")
            .unwrap();
        dialog.open();

        backend.render(dialog.id());
        let ops = backend.ops(dialog.id());
        assert!(
            ops.iter()
                .any(|op| matches!(op, DrawOp::Ellipse(..) | DrawOp::Polygon(..))),
            "{icon:?} should paint a glyph: {ops:?}"
        );
    }
}

#[test]
fn a_tall_message_is_clipped_to_the_card_and_the_window() {
    let (backend, _core, ui) = setup_sized(Dip(200.0), Dip(140.0));
    let dialog = TaskDialog::new(&ui, "Title", &"a long message that has to wrap ".repeat(40))
        .unwrap()
        .command("OK")
        .unwrap();
    dialog.open();

    let client = ui.client_rect();
    let (_, bounds, _, visible, _) = backend.node(dialog.id()).unwrap();
    assert_eq!(bounds, client, "the scrim still covers the client");
    assert!(visible);

    // Painting must not panic and must still draw the card with the buttons in
    // reach: the message is clipped to the card while the card stays in view.
    backend.render(dialog.id());
    let ops = backend.ops(dialog.id());
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Rounded(..))),
        "the card was painted: {ops:?}"
    );
}

#[test]
fn closing_hides_the_task_dialog_without_an_action() {
    let (backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let dialog = TaskDialog::new(&ui, "Title", "Message")
        .unwrap()
        .command("OK")
        .unwrap()
        .on_action(|_| Some(0u32));
    let runtime = runtime(core, &log);
    dialog.open();
    dialog.close();

    runtime.deliver(dialog.id(), &key(Key::RETURN));
    runtime.deliver(dialog.id(), &key(Key::ESCAPE));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    let (_, _, _, visible, _) = backend.node(dialog.id()).unwrap();
    assert!(!visible, "a closed dialog is hidden");
    assert!(!dialog.is_open());
    assert!(
        log.borrow().is_empty(),
        "closing raises no action and a closed dialog ignores keys"
    );
}
