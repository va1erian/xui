//! #120: a Win32 menu popup is a real top-level owned window, so it floats
//! above every client node (never repainted over by a later sibling) and its
//! whole area is the popup's face, not a clipped child.
//!
//! Window-creating test: it quits itself under a watchdog and never leaves a
//! window behind.

#![cfg(windows)]

use core::ffi::c_void;
use std::cell::Cell;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    GW_OWNER, GWL_STYLE, GetClientRect, GetWindow, GetWindowLongPtrW, WS_CHILD,
};

use xui_core::app::{App, Ui};
use xui_core::backend::{Backend, NodeKind, NodeSpec, PlatformSpec};
use xui_core::property::{Properties, Value};
use xui_core::widget::{Menu, MenuId};
use xui_core::{Color, Dip, Rect, WidgetId};
use xui_win32::{Hwnd, Win32Backend};

/// The colour a later-created node paints, so it is a real overlapping sibling.
const OVERLAY: Color = Color::rgb(0x00, 0x78, 0xD4);

#[derive(Clone, Copy, Default)]
struct Checks {
    /// The popup is a top-level window (`WS_CHILD` is clear).
    top_level: bool,
    /// The popup is owned by the host window.
    owned: bool,
    /// The keyboard focus moved to the popup when it opened.
    focus_on_popup: bool,
    /// The popup's client area is exactly the node's face, so no frame clips it.
    face_is_client: bool,
    /// The later-created node is an ordinary child, not a popup.
    overlay_is_child: bool,
}

#[derive(Clone, Copy)]
enum Msg {
    Open,
    Check,
    Quit,
}

struct PopupApp {
    backend: Rc<Win32Backend>,
    menu: Menu<Msg>,
    popup: WidgetId,
    overlay: WidgetId,
    owner: Hwnd,
    checks: Rc<Cell<Checks>>,
}

impl App for PopupApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Open => {
                // Public portable API: `("open", true)` opens the first bar
                // menu, the same path a title click takes. A short timer
                // checks once the top-level popup has been composed.
                self.menu.set_property("open", Value::Bool(true));
            }
            Msg::Check => {
                let mut checks = Checks::default();
                if let Some(hwnd) = self.backend.node_hwnd(self.popup) {
                    checks.top_level = !is_child(hwnd);
                    checks.owned = owner(hwnd) == Some(self.owner);
                    checks.focus_on_popup = focus() == Some(hwnd);
                    let face = self.backend.bounds(self.popup);
                    checks.face_is_client = client_size(hwnd) == (face.width(), face.height());
                }
                checks.overlay_is_child =
                    self.backend.node_hwnd(self.overlay).is_some_and(is_child);
                self.checks.set(checks);
                ui.quit();
            }
            Msg::Quit => ui.quit(),
        }
    }
}

#[test]
fn a_bar_menu_popup_floats_above_a_later_node() {
    let backend = Rc::new(Win32Backend::new());
    let backend_for_run: Rc<dyn Backend> = backend.clone();
    let checks = Rc::new(Cell::new(Checks::default()));
    let timed_out = Rc::new(Cell::new(false));

    let result = {
        let checks = Rc::clone(&checks);
        let timed_out = Rc::clone(&timed_out);
        let backend = Rc::clone(&backend);
        xui_core::app::run_app(
            backend_for_run,
            PlatformSpec::new("xui.menu.popup").size(Dip(320.0), Dip(240.0)),
            move |ui| {
                // One long watchdog (fails the test instead of hanging) and one
                // short timer that checks the composed popup.
                let watchdog = ui.set_timer(5000);
                let check = ui.set_timer(400);
                let timed_out_for_timer = Rc::clone(&timed_out);
                ui.on_timer(move |fired| {
                    if fired == check {
                        Some(Msg::Check)
                    } else if fired == watchdog {
                        timed_out_for_timer.set(true);
                        Some(Msg::Quit)
                    } else {
                        None
                    }
                });
                let owner = ui
                    .native_window()
                    .map(|handle| Hwnd::from_raw(handle.raw()))
                    .expect("the Win32 backend exposes the host handle");
                let menu = Menu::bar(ui, Rect::new(0, 0, 260, 28))
                    .expect("create the menu bar")
                    .on_select(|_| None)
                    .build(|m| {
                        m.submenu(MenuId::new(0), "&File", |f| {
                            f.item(MenuId::new(1), "&New");
                            f.item(MenuId::new(2), "&Open");
                            f.item(MenuId::new(3), "&Save");
                        });
                    });
                let popup = menu.popup_id(0).expect("a popup node");
                // Created after the menu's pooled popups, directly under the
                // dropdown: a later sibling a child popup could be clipped by.
                let overlay = ui
                    .create_node(&NodeSpec::new(NodeKind::Custom, Rect::new(0, 27, 200, 140)))
                    .expect("create the later node");
                ui.set_painter(overlay, Rc::new(|canvas| canvas.clear(OVERLAY)));
                ui.emit(Msg::Open);
                PopupApp {
                    backend,
                    menu,
                    popup,
                    overlay,
                    owner,
                    checks,
                }
            },
        )
    };

    // No desktop: skip rather than fail.
    if result.is_err() {
        return;
    }
    assert!(!timed_out.get(), "the watchdog fired; the app never quit");

    let checks = checks.get();
    assert!(
        checks.top_level,
        "the open menu popup is a top-level window, so no client sibling can clip it"
    );
    assert!(checks.owned, "the popup is owned by its host window");
    assert!(
        checks.overlay_is_child,
        "the later node is an ordinary child, so the test exercises the overlap"
    );
    assert!(
        checks.focus_on_popup,
        "opening the menu moves the keyboard focus to the popup"
    );
    assert!(
        checks.face_is_client,
        "the popup's whole face is its client area, so the frame clips nothing"
    );
}

/// Whether `hwnd` is a `WS_CHILD`; a top-level window is not.
fn is_child(hwnd: Hwnd) -> bool {
    // SAFETY: `hwnd` is a live window; `GWL_STYLE` only reads the style bits.
    let style = unsafe { GetWindowLongPtrW(raw(hwnd), GWL_STYLE) };
    style & (WS_CHILD.0 as isize) != 0
}

/// The owner of `hwnd`, or `None`.
fn owner(hwnd: Hwnd) -> Option<Hwnd> {
    // SAFETY: `hwnd` is a live window; `GetWindow(GW_OWNER)` only reads it.
    let raw = unsafe { GetWindow(raw(hwnd), GW_OWNER) }.ok()?;
    (!raw.0.is_null()).then(|| Hwnd::from_raw(raw.0 as usize))
}

/// The keyboard focus of the calling (UI) thread, or `None`.
fn focus() -> Option<Hwnd> {
    // SAFETY: `GetFocus` returns the thread's focus window, no arguments.
    let raw = unsafe { GetFocus() };
    (!raw.0.is_null()).then(|| Hwnd::from_raw(raw.0 as usize))
}

/// The width and height of `hwnd`'s client area.
fn client_size(hwnd: Hwnd) -> (i32, i32) {
    let mut rect = RECT::default();
    // SAFETY: `hwnd` is a live window; `rect` is a valid out-pointer.
    if unsafe { GetClientRect(raw(hwnd), &mut rect) }.is_err() {
        return (0, 0);
    }
    (rect.right - rect.left, rect.bottom - rect.top)
}

/// `Hwnd` as the raw `windows` `HWND`.
fn raw(hwnd: Hwnd) -> HWND {
    HWND(hwnd.raw() as *mut c_void)
}
