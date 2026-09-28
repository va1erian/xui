use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{App, Core, Runtime, Ui, run_app};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{
    Backend, BackendError, Event, ImplKind, NodeKind, NodeSpec, Painter, ParentRef, PlatformSpec,
    Result as BackendResult, TextMetrics, TextStyle, TimerId, Waker, WidgetId, WindowId,
};
use crate::geometry::Rect;
use crate::theme::Theme;

mod boot;
mod delivery;
mod runtime;
mod windows;

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
