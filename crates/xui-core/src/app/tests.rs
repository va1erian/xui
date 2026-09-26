use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{
    Backend, Event, NodeKind, NodeSpec, ParentRef, PlatformSpec, TimerId, WidgetId, WindowId,
};
use crate::geometry::Rect;

struct TestApp {
    log: Rc<RefCell<Vec<u32>>>,
}

impl App for TestApp {
    type Msg = u32;

    fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
        self.log.borrow_mut().push(msg);
    }
}

fn setup() -> (Rc<HeadlessBackend>, WindowId, Rc<Core<u32>>, Ui<u32>) {
    let backend = Rc::new(HeadlessBackend::new());
    backend.init();
    let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, window, core, ui)
}

fn test_app(log: &Rc<RefCell<Vec<u32>>>) -> TestApp {
    TestApp {
        log: Rc::clone(log),
    }
}

#[test]
fn a_widget_event_is_mapped_to_a_message_and_delivered() {
    let (backend, window, core, ui) = setup();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::default()),
        )
        .unwrap();
    ui.register_events(node, |event| match event {
        Event::Char('a') => Some(10),
        Event::Char('b') => Some(20),
        _ => None,
    });

    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    // The backend would deliver these; drive the runtime directly.
    assert!(runtime.deliver(node, &Event::Char('a')));
    assert!(runtime.deliver(node, &Event::Char('b')));
    assert!(!runtime.deliver(node, &Event::Char('z')), "unmapped event");

    // The mapper enqueued and woke the backend; the wake drains.
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![10, 20]);
}

#[test]
fn update_is_never_reentered() {
    struct DepthApp {
        in_update: Rc<Cell<usize>>,
        max_depth: Rc<Cell<usize>>,
        log: Rc<RefCell<Vec<u32>>>,
    }

    impl App for DepthApp {
        type Msg = u32;

        fn update(&mut self, msg: u32, ui: &mut Ui<u32>) {
            let depth = self.in_update.get() + 1;
            self.in_update.set(depth);
            self.max_depth.set(self.max_depth.get().max(depth));
            self.log.borrow_mut().push(msg);
            // A message raised while `update` runs must not re-enter it.
            if msg == 1 {
                ui.emit(2);
            }
            self.in_update.set(depth - 1);
        }
    }

    let (_backend, _window, core, _ui) = setup();
    let in_update = Rc::new(Cell::new(0));
    let max_depth = Rc::new(Cell::new(0));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        Rc::clone(&core),
        DepthApp {
            in_update: Rc::clone(&in_update),
            max_depth: Rc::clone(&max_depth),
            log: Rc::clone(&log),
        },
    );

    core.enqueue(1);
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(
        *log.borrow(),
        vec![1, 2],
        "both messages delivered, in order"
    );
    assert_eq!(max_depth.get(), 1, "update was never re-entered");
}

#[test]
fn a_close_request_maps_to_a_message() {
    let (backend, window, core, ui) = setup();
    ui.on_close(|| Some(99));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    assert!(runtime.deliver(WidgetId::NONE, &Event::Close));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![99]);
    assert!(
        backend.window_title(window).is_some(),
        "the window was kept open"
    );
}

#[test]
fn closing_without_a_mapping_closes_the_window_and_quits() {
    let (backend, window, core, _ui) = setup();
    let runtime = Runtime::primary(
        Rc::clone(&core),
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );

    runtime.deliver(WidgetId::NONE, &Event::Close);
    assert!(backend.window_title(window).is_none(), "the window closed");
    assert!(backend.quit_requested());
}

#[test]
fn a_secondary_window_closes_without_quitting() {
    let (backend, window, core, _ui) = setup();
    let runtime = Runtime::new(
        Rc::clone(&core),
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
        false,
    );

    runtime.deliver(WidgetId::NONE, &Event::Close);
    assert!(backend.window_title(window).is_none(), "the window closed");
    assert!(!backend.quit_requested(), "the loop keeps running");
}

#[test]
fn a_timer_tick_maps_to_a_message() {
    let (_backend, _window, core, ui) = setup();
    let id = TimerId(7);
    ui.on_timer(move |fired| (fired == id).then_some(5));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    runtime.deliver(WidgetId::NONE, &Event::Timer { id });
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![5]);
}

#[test]
fn a_worker_thread_proxy_reaches_update() {
    let (_backend, _window, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    let proxy = ui.proxy();
    let handles: Vec<_> = (0..4)
        .map(|worker| {
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                for message in 0..10 {
                    proxy.send(worker * 10 + message).unwrap();
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("worker panicked");
    }

    // The headless waker does not pump: drive the wake as the backend would.
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    let mut received = log.borrow().clone();
    received.sort_unstable();
    assert_eq!(received, (0..40).collect::<Vec<_>>());
}

#[test]
fn a_dpi_change_maps_to_a_message_with_the_new_dpi() {
    let (_backend, _window, core, ui) = setup();
    let seen = Rc::new(Cell::new(None));
    let seen_for_callback = Rc::clone(&seen);
    ui.on_dpi_changed(move |dpi, suggested| {
        seen_for_callback.set(Some((dpi, suggested)));
        Some(dpi)
    });
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    let suggested = Rect::new(10, 20, 810, 620);
    assert!(runtime.deliver(
        WidgetId::NONE,
        &Event::DpiChanged {
            dpi: 192,
            suggested,
        }
    ));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![192]);
    assert_eq!(seen.get(), Some((192, suggested)));
}

#[test]
fn a_dpi_change_without_a_mapper_is_a_no_op() {
    let (_backend, _window, core, _ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(Rc::clone(&core), test_app(&log));

    assert!(runtime.deliver(
        WidgetId::NONE,
        &Event::DpiChanged {
            dpi: 192,
            suggested: Rect::default(),
        }
    ));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(log.borrow().is_empty(), "nothing was mapped");
}

#[test]
fn geometry_queries_reach_the_backend() {
    let (_backend, _window, _core, ui) = setup();
    assert_eq!(ui.dpi(), 96);
    assert_eq!(ui.client_rect().width(), 640);
}

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
