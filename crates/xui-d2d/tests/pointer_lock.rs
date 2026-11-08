#![cfg(windows)]
// Posting a key to the window and asking Windows where the cursor is confined
// are Win32 calls; they are the one place this test needs `unsafe`, confined to
// `os`.

//! The pointer lock: it pins the cursor, and every way it ends frees it, with
//! `PointerLockLost` only for the ends the node did not ask for.
//!
//! Every loop ends on a timer and a later watchdog fails the test instead of
//! letting it hang. Run inside the sandbox (`scripts\sandbox\run.ps1`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};

use xui_core::Rect;
use xui_core::backend::{
    Backend, Event, NodeKind, NodeSpec, ParentRef, PlatformSpec, TimerId, WidgetId, WindowId,
};
use xui_core::router::WidgetHost;
use xui_d2d::D2dBackend;

mod os {
    #![allow(unsafe_code)]

    use windows::Win32::Foundation::{LPARAM, RECT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetClipCursor, PostMessageW, WM_KEYDOWN, WM_KEYUP,
    };
    use windows::core::{HSTRING, PCWSTR};

    /// Presses and releases Esc in the window titled `title`.
    pub fn press_escape(title: &str) {
        // SAFETY: a lookup, then two posts of plain key messages.
        unsafe {
            let hwnd = FindWindowW(PCWSTR::null(), &HSTRING::from(title)).expect("the window");
            let key = WPARAM(usize::from(VK_ESCAPE.0));
            let _ = PostMessageW(Some(hwnd), WM_KEYDOWN, key, LPARAM(0));
            let _ = PostMessageW(Some(hwnd), WM_KEYUP, key, LPARAM(0));
        }
    }

    /// Whether the cursor is confined to a single point.
    pub fn pinned() -> bool {
        let mut clip = RECT::default();
        // SAFETY: `clip` is a valid out-pointer.
        let _ = unsafe { GetClipCursor(&mut clip) };
        clip.right - clip.left <= 1 && clip.bottom - clip.top <= 1
    }
}

/// The cursor clip belongs to the process, so the tests take turns.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn exclusive() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner())
}

/// A step run when its timer fires.
type Step = Box<dyn Fn()>;

/// Runs `steps` on timers 100 ms apart, then quits; a watchdog fails instead
/// of hanging.
struct Sink {
    backend: Rc<D2dBackend>,
    timers: RefCell<Vec<(TimerId, Step)>>,
    watchdog: Cell<Option<TimerId>>,
    timed_out: Cell<bool>,
    lost: Cell<u32>,
    keys: Cell<u32>,
    last: Cell<Option<TimerId>>,
    window: WindowId,
}

impl WidgetHost for Sink {
    fn deliver(&self, _target: WidgetId, event: &Event) -> bool {
        match event {
            Event::PointerLockLost => self.lost.set(self.lost.get() + 1),
            Event::KeyDown { .. } | Event::KeyUp { .. } | Event::Char(_) => {
                self.keys.set(self.keys.get() + 1);
            }
            Event::Timer { id } if Some(*id) == self.watchdog.get() => {
                self.timed_out.set(true);
                self.backend.quit(1);
            }
            Event::Timer { id } => {
                // Timers repeat: each step runs once.
                self.backend.kill_timer(self.window, *id);
                for (timer, step) in self.timers.borrow().iter() {
                    if timer == id {
                        step();
                    }
                }
                if self.last.get() == Some(*id) {
                    self.backend.quit(0);
                }
            }
            _ => {}
        }
        true
    }
}

fn run(backend: &Rc<D2dBackend>, window: WindowId, steps: Vec<Box<dyn Fn()>>) -> Rc<Sink> {
    let sink = Rc::new(Sink {
        backend: Rc::clone(backend),
        timers: RefCell::default(),
        watchdog: Cell::new(None),
        timed_out: Cell::new(false),
        lost: Cell::new(0),
        keys: Cell::new(0),
        last: Cell::new(None),
        window,
    });
    backend.set_event_sink(window, Rc::clone(&sink) as Rc<dyn WidgetHost>);
    for (i, step) in steps.into_iter().enumerate() {
        let id = backend.set_timer(window, 100 * (i as u32 + 1));
        sink.last.set(Some(id));
        sink.timers.borrow_mut().push((id, step));
    }
    sink.watchdog.set(Some(backend.set_timer(window, 8000)));
    let code = backend.run_with(window, &mut || {});
    assert!(!sink.timed_out.get(), "the loop did not quit on its own");
    assert_eq!(code, 0);
    sink
}

fn setup(title: &str) -> (Rc<D2dBackend>, WindowId, WidgetId) {
    let backend = Rc::new(D2dBackend::new());
    backend.init();
    let window = backend.open_window(&PlatformSpec::new(title)).unwrap();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 100, 100)),
        )
        .unwrap();
    (backend, window, node)
}

#[test]
fn escape_ends_the_lock_with_a_notice_and_is_not_passed_on() {
    let title = "xui-d2d lock escape test";
    let _turn = exclusive();
    let (backend, window, node) = setup(title);
    backend.set_pointer_lock(node, true);
    let sink = run(
        &backend,
        window,
        vec![Box::new(move || os::press_escape(title)), Box::new(|| {})],
    );
    assert_eq!(sink.lost.get(), 1, "one PointerLockLost for the Esc");
    assert_eq!(
        sink.keys.get(),
        0,
        "the Esc that ended the lock stops there"
    );
    backend.close_window(window);
}

#[test]
fn removing_the_node_or_closing_the_window_ends_the_lock_without_a_notice() {
    let _turn = exclusive();
    let (backend, window, node) = setup("xui-d2d lock release test");
    backend.set_pointer_lock(node, true);
    let probe = Rc::clone(&backend);
    let sink = run(
        &backend,
        window,
        vec![Box::new(move || probe.destroy(node)), Box::new(|| {})],
    );
    assert_eq!(sink.lost.get(), 0, "a release the node caused is no loss");
    let again = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 10, 10)),
        )
        .unwrap();
    backend.set_pointer_lock(again, true);
    backend.close_window(window);
    backend.set_pointer_lock(again, false);
}

/// Reads where Windows confines the cursor, which only an interactive desktop
/// honours: a headless CI runner reports no confinement. Run it in the
/// sandbox: `scripts/sandbox/run.ps1` with `-TestArgs --include-ignored`.
#[test]
#[ignore = "reads the cursor clip: needs an interactive desktop; run in the sandbox with --include-ignored"]
fn the_lock_pins_the_cursor_and_every_end_frees_it() {
    let title = "xui-d2d lock pin test";
    let _turn = exclusive();
    let (backend, window, node) = setup(title);
    backend.set_pointer_lock(node, true);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let (log, probe) = (Rc::clone(&observed), Rc::clone(&backend));
    run(
        &backend,
        window,
        vec![
            Box::new({
                let log = Rc::clone(&log);
                move || log.borrow_mut().push(os::pinned())
            }),
            Box::new(move || os::press_escape(title)),
            Box::new({
                let log = Rc::clone(&log);
                move || log.borrow_mut().push(os::pinned())
            }),
            Box::new({
                let log = Rc::clone(&log);
                let probe = Rc::clone(&probe);
                move || {
                    probe.set_pointer_lock(node, true);
                    log.borrow_mut().push(os::pinned());
                    probe.destroy(node);
                    log.borrow_mut().push(os::pinned());
                }
            }),
        ],
    );
    assert_eq!(
        *observed.borrow(),
        vec![true, false, true, false],
        "pinned; freed by Esc; pinned again; freed by removing the node"
    );
    let again = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 10, 10)),
        )
        .unwrap();
    backend.set_pointer_lock(again, true);
    assert!(os::pinned());
    backend.close_window(window);
    assert!(!os::pinned(), "closing the window frees the cursor");
}
