//! A portable menu popup is pinned to its host window: dragging the window must
//! carry the open drop-down along, not leave it behind (#127).
#![cfg(windows)]

use core::ffi::c_void;
use std::cell::Cell;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
};

use xui_core::app::{App, Ui};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::property::{Properties, Value};
use xui_core::widget::{Menu, MenuId};
use xui_core::{Dip, Rect};
use xui_win32::{Hwnd, Win32Backend};

/// How far the host is moved, in pixels.
const DX: i32 = 40;
const DY: i32 = 30;

#[derive(Clone, Copy, Default)]
struct Report {
    /// The popup moved by the same delta as the host.
    popup_followed: bool,
    host_delta: (i32, i32),
    popup_delta: (i32, i32),
}

enum Msg {
    Open,
    Move,
    Quit,
}

struct FollowApp {
    menu: Menu<Msg>,
    owner: Hwnd,
    popup: Hwnd,
    report: Rc<Cell<Report>>,
}

impl App for FollowApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Open => {
                self.menu.set_property("open", Value::Bool(true));
            }
            Msg::Move => {
                let before_host = window_rect(self.owner);
                let before_popup = window_rect(self.popup);
                // SAFETY: both handles are live windows; only integer geometry
                // and documented flags are passed.
                unsafe {
                    let _ = SetWindowPos(
                        raw(self.owner),
                        None,
                        before_host.left + DX,
                        before_host.top + DY,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                let after_host = window_rect(self.owner);
                let after_popup = window_rect(self.popup);
                let host_delta = (
                    after_host.left - before_host.left,
                    after_host.top - before_host.top,
                );
                let popup_delta = (
                    after_popup.left - before_popup.left,
                    after_popup.top - before_popup.top,
                );
                self.report.set(Report {
                    popup_followed: popup_delta == host_delta,
                    host_delta,
                    popup_delta,
                });
                ui.quit();
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn raw(hwnd: Hwnd) -> HWND {
    HWND(hwnd.raw() as *mut c_void)
}

/// `hwnd`'s screen rectangle.
fn window_rect(hwnd: Hwnd) -> Rect {
    let mut rect = RECT::default();
    // SAFETY: `hwnd` is live; `rect` is a valid out-pointer.
    let _ = unsafe { GetWindowRect(raw(hwnd), &mut rect) };
    Rect::new(rect.left, rect.top, rect.right, rect.bottom)
}

#[test]
fn the_popup_follows_a_moved_host() {
    let backend = Rc::new(Win32Backend::new());
    let backend_for_run: Rc<dyn Backend> = backend.clone();
    let report = Rc::new(Cell::new(Report::default()));
    let timed_out = Rc::new(Cell::new(false));

    let result = {
        let report = Rc::clone(&report);
        let timed_out = Rc::clone(&timed_out);
        let backend_for_make = Rc::clone(&backend);
        xui_core::app::run_app(
            backend_for_run,
            PlatformSpec::new("xui.menu.follow").size(Dip(320.0), Dip(240.0)),
            move |ui| {
                let open = ui.set_timer(150);
                let move_at = ui.set_timer(500);
                let watchdog = ui.set_timer(5000);
                let flag = Rc::clone(&timed_out);
                ui.on_timer(move |fired| {
                    if fired == open {
                        Some(Msg::Open)
                    } else if fired == move_at {
                        Some(Msg::Move)
                    } else if fired == watchdog {
                        flag.set(true);
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
                        });
                    });
                let popup = menu
                    .popup_id(0)
                    .and_then(|id| backend_for_make.node_hwnd(id))
                    .expect("the pool created a popup");
                FollowApp {
                    menu,
                    owner,
                    popup,
                    report,
                }
            },
        )
    };

    if result.is_err() {
        return;
    }
    assert!(!timed_out.get(), "the watchdog fired; the app never quit");
    let report = report.get();
    assert_eq!(
        report.host_delta,
        (DX, DY),
        "the test moved the host window"
    );
    assert_eq!(
        report.popup_delta, report.host_delta,
        "the open popup must move with its host window"
    );
    assert!(report.popup_followed);
}
