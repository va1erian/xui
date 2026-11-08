#![cfg(windows)]
// Whether the child window is shown, and whose child it is, can only be asked
// of Win32; that is the one place this test needs `unsafe`, confined to `os`.

//! A node's native child window: handed out as a Win32 handle, a child of the
//! node's window, shown only while the node can show it, and refused once the
//! node gives it up.
//!
//! The loop ends on a timer and a later watchdog fails the test instead of
//! letting it hang. Run inside the sandbox (`scripts\sandbox\run.ps1`).

use std::cell::Cell;
use std::rc::Rc;

use raw_window_handle::RawWindowHandle;
use xui_core::Rect;
use xui_core::backend::{
    Backend, Event, NativeSurface, NodeKind, NodeSpec, ParentRef, PlatformSpec, TimerId, WidgetId,
    WindowId,
};
use xui_core::router::WidgetHost;
use xui_d2d::D2dBackend;

mod os {
    #![allow(unsafe_code)]

    use raw_window_handle::RawWindowHandle;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetParent, GetWindowRect, IsWindowVisible};

    use super::NativeSurface;

    fn hwnd(surface: &NativeSurface) -> HWND {
        let Ok(handle) = surface.window_handle() else {
            panic!("the surface refused its handle");
        };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            panic!("not a Win32 handle");
        };
        HWND(win32.hwnd.get() as *mut _)
    }

    pub fn shown(surface: &NativeSurface) -> bool {
        // SAFETY: a query on a live window handle.
        unsafe { IsWindowVisible(hwnd(surface)).as_bool() }
    }

    pub fn is_child_of_some_window(surface: &NativeSurface) -> bool {
        // SAFETY: a query on a live window handle.
        unsafe { GetParent(hwnd(surface)).is_ok() }
    }

    /// The window's size in pixels.
    pub fn size(surface: &NativeSurface) -> (i32, i32) {
        let mut rect = Default::default();
        // SAFETY: `rect` is a valid out-pointer.
        let _ = unsafe { GetWindowRect(hwnd(surface), &mut rect) };
        (rect.right - rect.left, rect.bottom - rect.top)
    }
}

/// Quits on one timer, fails on a later one, and runs `at_quit` first.
struct Sink {
    backend: Rc<D2dBackend>,
    quit_at: Cell<Option<TimerId>>,
    watchdog: Cell<Option<TimerId>>,
    timed_out: Cell<bool>,
    at_quit: Box<dyn Fn()>,
}

impl WidgetHost for Sink {
    fn deliver(&self, _target: WidgetId, event: &Event) -> bool {
        match event {
            Event::Timer { id } if Some(*id) == self.quit_at.get() => {
                (self.at_quit)();
                self.backend.quit(0);
            }
            Event::Timer { id } if Some(*id) == self.watchdog.get() => {
                self.timed_out.set(true);
                self.backend.quit(1);
            }
            _ => {}
        }
        true
    }
}

fn run(backend: &Rc<D2dBackend>, window: WindowId, at_quit: impl Fn() + 'static) {
    let sink = Rc::new(Sink {
        backend: Rc::clone(backend),
        quit_at: Cell::new(None),
        watchdog: Cell::new(None),
        timed_out: Cell::new(false),
        at_quit: Box::new(at_quit),
    });
    backend.set_event_sink(window, Rc::clone(&sink) as Rc<dyn WidgetHost>);
    sink.quit_at.set(Some(backend.set_timer(window, 300)));
    sink.watchdog.set(Some(backend.set_timer(window, 8000)));
    let code = backend.run_with(window, &mut || {});
    assert!(!sink.timed_out.get(), "the loop did not quit on its own");
    assert_eq!(code, 0);
}

fn node(backend: &D2dBackend, window: WindowId, bounds: Rect, popup: bool) -> WidgetId {
    let mut spec = NodeSpec::new(NodeKind::Custom, bounds);
    if popup {
        spec = spec.popup();
    }
    backend.create(ParentRef::Window(window), &spec).unwrap()
}

#[test]
fn a_node_gets_a_child_window_that_follows_its_visibility_and_popups() {
    let backend = Rc::new(D2dBackend::new());
    backend.init();
    let window = backend
        .open_window(&PlatformSpec::new("xui-d2d surface test"))
        .unwrap();
    let view = node(&backend, window, Rect::new(10, 10, 210, 110), false);
    assert!(backend.native_surface(view).is_none(), "not asked for yet");
    backend.set_native_surface(view, true);
    let surface = backend.native_surface(view).expect("a child window");
    assert!(matches!(
        surface.window_handle().unwrap().as_raw(),
        RawWindowHandle::Win32(_)
    ));
    assert!(os::is_child_of_some_window(&surface));
    backend.apply_moves(window, &[(view, Rect::new(10, 10, 160, 90))]);

    let checked = surface.clone();
    let probe = Rc::clone(&backend);
    run(&backend, window, move || {
        assert!(os::shown(&checked), "a visible whole node shows its window");
        assert_eq!(
            os::size(&checked),
            (150, 80),
            "it follows the node's bounds"
        );

        let popup = node(&probe, window, Rect::new(100, 50, 300, 150), true);
        assert!(!os::shown(&checked), "a popup over it hides it");
        probe.destroy(popup);
        assert!(os::shown(&checked), "and the window returns without it");

        probe.set_visible(view, false);
        assert!(!os::shown(&checked), "a hidden node hides its window");
        probe.set_visible(view, true);
        assert!(os::shown(&checked));
    });
    backend.close_window(window);
}

#[test]
fn a_surface_given_up_or_removed_refuses_its_handles() {
    let backend = Rc::new(D2dBackend::new());
    backend.init();
    let window = backend
        .open_window(&PlatformSpec::new("xui-d2d surface release test"))
        .unwrap();
    let given_up = node(&backend, window, Rect::new(0, 0, 50, 50), false);
    let removed = node(&backend, window, Rect::new(60, 0, 110, 50), false);
    backend.set_native_surface(given_up, true);
    backend.set_native_surface(removed, true);
    let a = backend.native_surface(given_up).unwrap();
    let b = backend.native_surface(removed).unwrap();

    backend.set_native_surface(given_up, false);
    assert!(backend.native_surface(given_up).is_none());
    assert!(a.window_handle().is_err(), "given up: handles refused");

    backend.destroy(removed);
    assert!(b.window_handle().is_err(), "removed: handles refused");

    let c = {
        backend.set_native_surface(given_up, true);
        backend.native_surface(given_up).unwrap()
    };
    backend.close_window(window);
    assert!(c.window_handle().is_err(), "window closed: handles refused");
}
