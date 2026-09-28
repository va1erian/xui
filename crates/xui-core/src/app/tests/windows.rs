use super::*;

/// A child app that records the messages it receives.
struct ChildApp {
    log: Rc<RefCell<Vec<u32>>>,
}

impl App for ChildApp {
    type Msg = u32;

    fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
        self.log.borrow_mut().push(msg);
    }
}

#[test]
fn a_secondary_window_runs_its_own_app() {
    let (backend, _window, _core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let log_for_child = Rc::clone(&log);
    let handle = ui
        .open_window(PlatformSpec::new("child"), move |_ui| ChildApp {
            log: log_for_child,
        })
        .expect("the secondary window opened");
    let child = handle.window();
    assert!(handle.is_open(), "a fresh secondary window is open");

    handle.send(7);
    // The headless backend does not pump; drive the wake it would.
    backend.inject(child, WidgetId::NONE, Event::Wake);
    assert_eq!(*log.borrow(), vec![7], "the child app received the message");

    handle.set_title("renamed");
    assert_eq!(backend.window_title(child), Some("renamed".to_string()));

    handle.close();
    assert!(!handle.is_open(), "the handle forgot the closed window");
    assert!(backend.window_title(child).is_none(), "the window is gone");
}

#[test]
fn a_secondary_windows_capture_reaches_the_backend() {
    let (_backend, _window, _core, ui) = setup();
    let handle = ui
        .open_window(
            PlatformSpec::new("shot").size(crate::units::dip(200.0), crate::units::dip(120.0)),
            |_ui| ChildApp {
                log: Rc::new(RefCell::new(Vec::new())),
            },
        )
        .unwrap();
    let image = handle.capture().expect("the window was captured");
    assert_eq!(image.size(), (200, 120));
    handle.close();
}

#[test]
fn a_primary_windows_capture_reaches_the_backend() {
    let (_backend, _window, _core, ui) = setup();
    assert_eq!(ui.capture().expect("captured").size(), (640, 480));
}

#[test]
fn a_backend_without_a_native_handle_reports_none() {
    let (_backend, _window, _core, ui) = setup();
    assert!(ui.native_window().is_none());
}

#[test]
fn a_modal_child_returns_its_result_and_reenables_the_owner() {
    struct Child;

    impl App for Child {
        type Msg = ();

        fn update(&mut self, _msg: (), ui: &mut Ui<()>) {
            ui.close_with_result(42u32);
        }
    }

    let (backend, window, _core, ui) = setup();
    let result: Option<u32> = ui.open_modal(PlatformSpec::new("modal"), |ui| {
        // Queue a message so the child's `update` runs and closes it.
        ui.emit(());
        Child
    });
    assert_eq!(result, Some(42), "the child's result came back");
    assert!(
        backend.window_enabled(window),
        "the owner was re-enabled after the modal child closed"
    );
}
