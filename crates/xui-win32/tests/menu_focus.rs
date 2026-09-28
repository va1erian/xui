//! A portable bar menu is a transient surface, not a window that steals
//! activation: opening one keeps the host window in the foreground while the
//! host's key input is routed to the open popup.
#![cfg(windows)]

use core::ffi::c_void;
use std::cell::Cell;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, VK_DOWN, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, PostMessageW, WM_KEYDOWN};

use xui_core::app::{App, Ui};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::property::{Properties, Value};
use xui_core::widget::{Menu, MenuId};
use xui_core::{Dip, Rect};
use xui_win32::{Hwnd, Win32Backend};

#[derive(Clone, Copy, Default)]
struct Report {
    /// The popup did not become the foreground window (the host was not
    /// deactivated by the menu).
    popup_not_foreground: bool,
    /// The OS focus stayed off the top-level popup.
    focus_off_popup: bool,
    /// The command the popup activated, proving the host's key reached it.
    selected: i32,
    /// Whether the menu closed after the command.
    menu_closed: bool,
}

enum Msg {
    Open,
    Keys,
    Selected(i32),
    Check,
    Quit,
}

struct MenuApp {
    menu: Menu<Msg>,
    owner: Hwnd,
    popup: Option<Hwnd>,
    selected: i32,
    report: Rc<Cell<Report>>,
}

impl App for MenuApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Open => {
                self.menu.set_property("open", Value::Bool(true));
            }
            Msg::Keys => {
                let hwnd = raw(self.owner);
                // SAFETY: `hwnd` is the live host window; the messages carry a
                // virtual-key code a real press would. The popup is open and
                // logically focused, so the host routes them to it.
                unsafe {
                    let _ = PostMessageW(
                        Some(hwnd),
                        WM_KEYDOWN,
                        WPARAM(VK_DOWN.0 as usize),
                        LPARAM(0),
                    );
                    let _ = PostMessageW(
                        Some(hwnd),
                        WM_KEYDOWN,
                        WPARAM(VK_RETURN.0 as usize),
                        LPARAM(0),
                    );
                }
            }
            Msg::Selected(id) => self.selected = id,
            Msg::Check => {
                let foreground = unsafe { GetForegroundWindow() };
                let focus = unsafe { GetFocus() };
                self.report.set(Report {
                    popup_not_foreground: self.popup.is_none_or(|popup| foreground != raw(popup)),
                    focus_off_popup: self.popup.is_none_or(|popup| focus != raw(popup)),
                    selected: self.selected,
                    menu_closed: !self.menu.is_open(),
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

#[test]
fn keyboard_navigation_keeps_the_host_active() {
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
            PlatformSpec::new("xui.menu.focus").size(Dip(320.0), Dip(240.0)),
            move |ui| {
                let open = ui.set_timer(150);
                let keys = ui.set_timer(400);
                let check = ui.set_timer(900);
                let watchdog = ui.set_timer(5000);
                let flag = Rc::clone(&timed_out);
                let window = ui.window();
                let timers = Rc::clone(&backend_for_make);
                ui.on_timer(move |fired| {
                    // Timers repeat; every step here must run once, or a later
                    // `open` tick re-opens the menu the keys just closed (#177).
                    timers.kill_timer(window, fired);
                    if fired == open {
                        Some(Msg::Open)
                    } else if fired == keys {
                        Some(Msg::Keys)
                    } else if fired == check {
                        Some(Msg::Check)
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
                    .on_select(|id| {
                        let which = if id == MenuId::new(1) {
                            1
                        } else if id == MenuId::new(2) {
                            2
                        } else {
                            0
                        };
                        Some(Msg::Selected(which))
                    })
                    .build(|m| {
                        m.submenu(MenuId::new(0), "&File", |f| {
                            f.item(MenuId::new(1), "&New");
                            f.item(MenuId::new(2), "&Open");
                        });
                    });
                let popup = menu
                    .popup_id(0)
                    .and_then(|id| backend_for_make.node_hwnd(id));
                MenuApp {
                    menu,
                    owner,
                    popup,
                    selected: 0,
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
    assert!(
        report.popup_not_foreground,
        "opening a menu must not make the popup the foreground window"
    );
    assert!(
        report.focus_off_popup,
        "the OS focus must stay on the host, not the top-level popup"
    );
    assert_eq!(
        report.selected, 2,
        "the host's Down+Enter reached the open menu and picked the second item"
    );
    assert!(report.menu_closed, "choosing a command closed the menu");
}
