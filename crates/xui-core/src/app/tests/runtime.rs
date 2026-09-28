use super::*;

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
