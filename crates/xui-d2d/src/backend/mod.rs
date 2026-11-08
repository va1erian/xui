#![forbid(unsafe_code)]

//! The [`D2dBackend`]: one Win32 window per xui window, every widget a painted
//! node of a scene xui keeps itself, drawn with Direct2D and DirectWrite.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use xui_core::Theme;
use xui_core::backend::{Event, WidgetId, WindowId};
use xui_core::router::WidgetHost;

use crate::sys::gfx::{DwShaper, Target};
use crate::sys::surface::SurfaceWindow;
use crate::sys::window::Hwnd;

mod canvas;
mod contract;
mod handler;
mod input;
mod keys;
mod paint;
mod pointer_lock;
mod scene;
mod surface;
mod window;

#[cfg(test)]
mod input_tests;

use scene::Scene;
use surface::Surface;

/// What the backend keeps for one top-level window.
struct WindowState {
    hwnd: Hwnd,
    sink: Option<Rc<dyn WidgetHost>>,
    /// Created lazily, and again after a lost device.
    target: Option<Rc<Target>>,
    theme: Theme,
    dpi: u32,
    /// The client area in device pixels.
    size: (i32, i32),
    hover: Option<WidgetId>,
    focused: Option<WidgetId>,
    /// Whether a `WM_MOUSELEAVE` is already requested.
    tracking: bool,
    primary: bool,
}

/// The state every window shares.
pub(crate) struct Shared {
    windows: RefCell<HashMap<u64, WindowState>>,
    scene: RefCell<Scene>,
    /// The nodes that own a native child window.
    surfaces: RefCell<HashMap<WidgetId, Surface>>,
    captured: Cell<Option<WidgetId>>,
    /// The node that holds the pointer lock.
    locked: Cell<Option<WidgetId>>,
    /// An `Esc` that ended a lock: its `WM_CHAR` and key-up are swallowed too.
    swallow_escape: Cell<bool>,
    /// Native child windows given up by their node but still held, by window.
    orphans: RefCell<Vec<(WindowId, std::sync::Weak<SurfaceWindow>)>>,
    /// Timer id to the window that owns it.
    timers: RefCell<HashMap<usize, u64>>,
    next_window: Cell<u64>,
    next_widget: Cell<u64>,
    next_timer: Cell<usize>,
    /// A UTF-16 high surrogate waiting for its low half.
    high_surrogate: Cell<u16>,
    exit_code: Cell<i32>,
    shaper: Option<DwShaper>,
    this: Weak<Shared>,
}

/// The Direct2D backend.
///
/// Create it with [`D2dBackend::new`] and hand it to `xui_core::app`:
///
/// ```no_run
/// use std::rc::Rc;
/// use xui_core::prelude::*;
///
/// struct Hello;
///
/// impl App for Hello {
///     type Msg = ();
///     fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
/// }
///
/// fn main() -> Result<()> {
///     xui_core::app("Hello")
///         .backend(Rc::new(xui_d2d::D2dBackend::new()))
///         .run(|ui| {
///             ui.root(column().padding(16).child(label("Hello, world")))?;
///             Ok(Hello)
///         })
/// }
/// ```
pub struct D2dBackend {
    shared: Rc<Shared>,
}

impl Default for D2dBackend {
    fn default() -> D2dBackend {
        D2dBackend::new()
    }
}

impl D2dBackend {
    /// A backend with no windows yet.
    pub fn new() -> D2dBackend {
        D2dBackend {
            shared: Rc::new_cyclic(|this| Shared {
                windows: RefCell::new(HashMap::new()),
                scene: RefCell::new(Scene::default()),
                surfaces: RefCell::new(HashMap::new()),
                captured: Cell::new(None),
                locked: Cell::new(None),
                swallow_escape: Cell::new(false),
                orphans: RefCell::new(Vec::new()),
                timers: RefCell::new(HashMap::new()),
                next_window: Cell::new(1),
                next_widget: Cell::new(1),
                next_timer: Cell::new(1),
                high_surrogate: Cell::new(0),
                exit_code: Cell::new(0),
                shaper: DwShaper::new(),
                this: this.clone(),
            }),
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        self.release_on_drop();
    }
}

impl Shared {
    fn allocate(cell: &Cell<u64>) -> u64 {
        let id = cell.get();
        cell.set(id + 1);
        id
    }

    /// Sends `event` to `target` through `window`'s sink. Nothing is borrowed
    /// while the sink runs, since it calls back into the backend.
    fn deliver(&self, window: WindowId, target: WidgetId, event: &Event) -> bool {
        let sink = self
            .windows
            .borrow()
            .get(&window.raw())
            .and_then(|state| state.sink.clone());
        let handled = sink.is_some_and(|sink| sink.deliver(target, event));
        if !target.is_none() && marks_target(event) {
            self.invalidate(target);
        }
        handled
    }

    fn window_of(&self, id: WidgetId) -> Option<WindowId> {
        self.scene.borrow().window_of(id)
    }

    fn hwnd_of(&self, window: WindowId) -> Option<Hwnd> {
        self.windows.borrow().get(&window.raw()).map(|w| w.hwnd)
    }

    fn dpi_of(&self, window: WindowId) -> u32 {
        self.windows
            .borrow()
            .get(&window.raw())
            .map_or(96, |state| state.dpi)
    }
}

/// Events that can change how their target looks, so it is repainted even
/// when the widget forgets to ask.
fn marks_target(event: &Event) -> bool {
    matches!(
        event,
        Event::MouseDown { .. }
            | Event::MouseUp { .. }
            | Event::MouseDoubleClick { .. }
            | Event::MouseWheel { .. }
            | Event::MouseLeave
            | Event::CaptureChanged
            | Event::KeyDown { .. }
            | Event::KeyUp { .. }
            | Event::Char(_)
            | Event::SetFocus
            | Event::KillFocus
    )
}
