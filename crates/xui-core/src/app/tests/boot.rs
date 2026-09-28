use super::*;

/// A backend that only creates its window when the loop starts, like `winit`:
/// `run_with` reports a real DPI and then builds the app, so a test can prove
/// the deferred path lays out at that DPI.
struct DeferredBackend {
    dpi: Cell<u32>,
}

impl Backend for DeferredBackend {
    fn run(&self) -> i32 {
        0
    }

    fn run_with(&self, _window: WindowId, on_ready: &mut dyn FnMut()) -> i32 {
        // The platform window now exists and reports a real (non-96) DPI.
        self.dpi.set(120);
        on_ready();
        0
    }

    fn quit(&self, _code: i32) {}

    fn wake(&self, _window: WindowId) {}

    fn waker(&self, _window: WindowId) -> Waker {
        Box::new(|| {})
    }

    fn set_event_sink(&self, _window: WindowId, _sink: Rc<dyn crate::router::WidgetHost>) {}

    fn open_window(&self, _spec: &PlatformSpec) -> BackendResult<WindowId> {
        Ok(WindowId::from_raw(1))
    }

    fn close_window(&self, _window: WindowId) {}

    fn create(&self, _parent: ParentRef, _spec: &NodeSpec) -> BackendResult<WidgetId> {
        Err(BackendError::CreateFailed("deferred test backend"))
    }

    fn destroy(&self, _id: WidgetId) {}

    fn apply_moves(&self, _window: WindowId, _moves: &[(WidgetId, Rect)]) {}

    fn set_visible(&self, _id: WidgetId, _visible: bool) {}

    fn set_enabled(&self, _id: WidgetId, _enabled: bool) {}

    fn focus(&self, _id: WidgetId) {}

    fn set_text(&self, _id: WidgetId, _text: &str) {}

    fn invalidate(&self, _id: WidgetId) {}

    fn invalidate_rect(&self, _id: WidgetId, _rect: Rect) {}

    fn set_painter(&self, _id: WidgetId, _painter: Painter) {}

    fn measure_text(&self, _text: &str, _style: &TextStyle, _dpi: u32) -> TextMetrics {
        TextMetrics {
            width: 0,
            height: 0,
            ascent: 0,
            descent: 0,
        }
    }

    fn dpi(&self, _window: WindowId) -> u32 {
        self.dpi.get()
    }

    fn client_rect(&self, _window: WindowId) -> Rect {
        Rect::new(0, 0, 640, 480)
    }

    fn set_theme(&self, _window: WindowId, _theme: &Theme) {}

    fn set_timer(&self, _window: WindowId, _millis: u32) -> TimerId {
        TimerId(0)
    }

    fn kill_timer(&self, _window: WindowId, _id: TimerId) {}

    fn supports(&self, _kind: NodeKind) -> ImplKind {
        ImplKind::Painted
    }
}

#[test]
fn run_app_builds_the_app_after_the_window_reports_its_dpi() {
    struct Recording {
        received: Rc<RefCell<Vec<u32>>>,
    }

    impl App for Recording {
        type Msg = u32;

        fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
            self.received.borrow_mut().push(msg);
        }
    }

    let backend: Rc<dyn Backend> = Rc::new(DeferredBackend { dpi: Cell::new(96) });
    let seen_dpi = Rc::new(Cell::new(0u32));
    let received = Rc::new(RefCell::new(Vec::new()));
    let seen_for_make = Rc::clone(&seen_dpi);
    let received_for_make = Rc::clone(&received);

    run_app(backend, PlatformSpec::new("deferred"), move |ui| {
        seen_for_make.set(ui.dpi());
        // Queue a message while building; `prime` must drain it once the
        // app exists, even though the sink already exists.
        ui.emit(7);
        Recording {
            received: received_for_make,
        }
    })
    .expect("the app ran");

    assert_eq!(seen_dpi.get(), 120, "make saw the window's real DPI");
    assert_eq!(
        *received.borrow(),
        vec![7],
        "a message queued by make reached `App::update`"
    );
}
