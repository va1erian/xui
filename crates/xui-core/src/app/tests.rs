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
fn geometry_queries_reach_the_backend() {
    let (_backend, _window, _core, ui) = setup();
    assert_eq!(ui.dpi(), 96);
    assert_eq!(ui.client_rect().width(), 640);
}
