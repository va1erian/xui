//! Widget-level tests on the headless backend: the modal card, delivery
//! through the app's messages, the backend hook and a fresh state on reopen.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::super::FileDialog;
use super::super::fs::{MemoryFileSystem, file_entry};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{
    Backend, Event, FileDialogMode, FileDialogOutcome, FileDialogRequest, NodeKind, PlatformSpec,
    WidgetId,
};
use crate::message::{Key, Modifiers};

/// A plain app for the widget-level tests.
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
    let window = backend
        .open_window(&PlatformSpec::new("filedialog"))
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

fn flush(runtime: &Runtime<TestApp>) {
    runtime.deliver(WidgetId::NONE, &Event::Wake);
}

#[test]
fn opening_draws_the_card_over_the_whole_window() {
    let (backend, _core, ui) = setup();
    let fs = MemoryFileSystem::new()
        .dir("/", vec![file_entry("x.txt")])
        .file("/x.txt");
    let dialog = FileDialog::open_file(&ui, "Open")
        .unwrap()
        .file_system(Rc::new(fs))
        .initial_dir("/");
    assert!(!dialog.is_open());

    dialog.open();

    assert!(dialog.is_open());
    let client = ui.client_rect();
    let (kind, bounds, _, visible, _) = backend.node(dialog.id()).unwrap();
    assert_eq!(kind, NodeKind::Custom);
    assert_eq!(bounds, client, "the scrim covers the whole client area");
    assert!(visible, "the open dialog is shown");

    backend.render(dialog.id());
    let ops = backend.ops(dialog.id());
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Rounded(..))),
        "the card was painted: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Open")),
        "the title was painted: {ops:?}"
    );
}

#[test]
fn enter_on_the_filename_accepts_and_escape_cancels() {
    let (backend, core, ui) = setup();
    let fs = MemoryFileSystem::new()
        .dir("/", vec![file_entry("x.txt")])
        .file("/x.txt");
    let dialog = FileDialog::open_file(&ui, "Open")
        .unwrap()
        .file_system(Rc::new(fs))
        .initial_dir("/")
        .on_accept(|_| Some(1))
        .on_cancel(|| Some(2));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    dialog.open();
    let focused = backend.focused().expect("the filename field is focused");
    runtime.deliver(focused, &key(Key::RETURN));
    flush(&runtime);

    assert_eq!(*log.borrow(), vec![1]);
    assert!(!dialog.is_open(), "accepting closed the dialog");
}

#[test]
fn arrow_keys_move_the_selection_from_the_filename_field() {
    let (backend, core, ui) = setup();
    let fs = MemoryFileSystem::new()
        .dir("/", vec![file_entry("a.txt"), file_entry("b.txt")])
        .file("/a.txt")
        .file("/b.txt");
    let dialog = FileDialog::open_file(&ui, "Open")
        .unwrap()
        .file_system(Rc::new(fs))
        .initial_dir("/")
        .on_accept(|path| {
            Some(if path.as_path() == Path::new("/b.txt") {
                5
            } else {
                4
            })
        });
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    dialog.open();
    let name = backend.focused().unwrap();
    runtime.deliver(name, &key(Key::DOWN));
    runtime.deliver(name, &key(Key::RETURN));
    flush(&runtime);

    assert_eq!(*log.borrow(), vec![5], "Down moved to the second row");
}

#[test]
fn escape_cancels_without_accepting() {
    let (backend, core, ui) = setup();
    let fs = MemoryFileSystem::new().dir("/", vec![]);
    let dialog = FileDialog::open_file(&ui, "Open")
        .unwrap()
        .file_system(Rc::new(fs))
        .initial_dir("/")
        .on_accept(|_| Some(1))
        .on_cancel(|| Some(2));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    dialog.open();
    let focused = backend.focused().unwrap();
    runtime.deliver(focused, &key(Key::ESCAPE));
    flush(&runtime);

    assert_eq!(*log.borrow(), vec![2]);
    assert!(!dialog.is_open());
}

#[test]
fn save_confirms_the_overwrite_before_accepting() {
    let (backend, core, ui) = setup();
    let fs = MemoryFileSystem::new()
        .dir("/", vec![file_entry("x.txt")])
        .file("/x.txt");
    let dialog = FileDialog::save_file(&ui, "Save As")
        .unwrap()
        .file_system(Rc::new(fs))
        .initial_dir("/")
        .suggested_name("x.txt")
        .on_accept(|_| Some(1))
        .on_cancel(|| Some(2));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    dialog.open();
    let name = backend.focused().unwrap();
    runtime.deliver(name, &key(Key::RETURN));
    flush(&runtime);
    assert!(log.borrow().is_empty(), "the overwrite must be confirmed");
    assert!(dialog.is_open());

    let accept = backend.focused().expect("the accept button is focused");
    assert_ne!(accept, name, "focus moved to the overwrite confirmation");
    runtime.deliver(accept, &key(Key::RETURN));
    flush(&runtime);
    assert_eq!(*log.borrow(), vec![1]);
    assert!(!dialog.is_open());
}

#[test]
fn reopening_starts_from_a_fresh_state() {
    let (backend, core, ui) = setup();
    let fs = MemoryFileSystem::new().dir("/", vec![]);
    let chosen: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    let recorder = Rc::clone(&chosen);
    let dialog = FileDialog::save_file(&ui, "Save As")
        .unwrap()
        .file_system(Rc::new(fs))
        .initial_dir("/")
        .suggested_name("first.txt")
        .on_accept(move |path| {
            recorder.borrow_mut().push(path);
            Some(1)
        });
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    dialog.open();
    let name = backend.focused().unwrap();
    runtime.deliver(name, &key(Key::RETURN));
    flush(&runtime);
    assert!(!dialog.is_open());

    dialog.set_suggested_name("second.txt");
    dialog.open();
    let name = backend.focused().unwrap();
    runtime.deliver(name, &key(Key::RETURN));
    flush(&runtime);

    assert_eq!(
        *chosen.borrow(),
        vec![PathBuf::from("/first.txt"), PathBuf::from("/second.txt")],
        "each open uses its own suggested name"
    );
}

#[test]
fn a_native_choice_skips_the_portable_card() {
    let (backend, core, ui) = setup();
    let chosen = PathBuf::from("/native.txt");
    backend.set_file_dialog(FileDialogOutcome::Chosen(chosen.clone()));
    let dialog = FileDialog::open_file(&ui, "Open")
        .unwrap()
        .on_accept(move |path| Some(if path == chosen { 7 } else { 0 }))
        .on_cancel(|| Some(8));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    dialog.open();
    flush(&runtime);

    assert_eq!(*log.borrow(), vec![7], "the chosen path was delivered");
    assert!(!dialog.is_open(), "the card never opened");
    assert!(
        !backend.node(dialog.id()).unwrap().3,
        "the scrim stayed hidden"
    );
    let request = backend.file_dialog_request().expect("a request was made");
    assert_eq!(request.mode, FileDialogMode::Open);
    assert_eq!(request.title, "Open");
}

#[test]
fn a_native_cancel_reaches_the_cancel_mapper() {
    let (backend, core, ui) = setup();
    backend.set_file_dialog(FileDialogOutcome::Cancelled);
    let dialog = FileDialog::open_file(&ui, "Open")
        .unwrap()
        .on_accept(|_| Some(1))
        .on_cancel(|| Some(8));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = runtime(core, &log);

    dialog.open();
    flush(&runtime);

    assert_eq!(*log.borrow(), vec![8]);
    assert!(!dialog.is_open());
}

#[test]
fn the_default_backend_hook_declines() {
    let backend = HeadlessBackend::new();
    let window = backend.open_window(&PlatformSpec::new("x")).unwrap();
    let request = FileDialogRequest {
        mode: FileDialogMode::Open,
        title: "Open".into(),
        initial_dir: None,
        suggested_name: None,
        filters: Vec::new(),
        require_existing: true,
    };
    assert_eq!(
        backend.file_dialog(window, &request),
        FileDialogOutcome::Declined,
        "the portable modal runs when the backend has no picker"
    );
}
