//! Moving the pointer from one open menu to another must not deactivate the
//! host window: showing the next popup may not take the host's activation, or
//! its chrome flickers on every switch.
#![cfg(windows)]

use core::cell::Cell;
use core::ffi::c_void;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, PostMessageW, WINEVENT_OUTOFCONTEXT, WM_MOUSEMOVE,
};

use xui_core::app::{App, Ui};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::property::{Properties, Value};
use xui_core::widget::{Menu, MenuId};
use xui_core::{Dip, Rect};
use xui_win32::{Hwnd, Win32Backend};

/// The `OBJID_WINDOW` value, from `winuser.h`: a foreground event for a window
/// itself, not one of its parts.
const OBJID_WINDOW: i32 = 0;

thread_local! {
    /// How many times the foreground moved to a window other than the host
    /// while the menu was open. A transient count is exactly the flicker.
    static FOREGROUND_LOSSES: Cell<u32> = const { Cell::new(0) };
    /// The host window's handle, as a raw value.
    static HOST: Cell<isize> = const { Cell::new(0) };
}

/// Counts a foreground change away from the host.
unsafe extern "system" fn on_foreground(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if event != EVENT_SYSTEM_FOREGROUND || id_object != OBJID_WINDOW || id_child != 0 {
        return;
    }
    let host = HOST.with(Cell::get);
    if hwnd.0 as isize != host {
        FOREGROUND_LOSSES.with(|count| count.set(count.get() + 1));
    }
}

#[derive(Clone, Copy, Default)]
struct Report {
    /// Foreground changes away from the host during the menu session.
    losses: u32,
}

enum Msg {
    Open,
    Switch,
    Check,
    Quit,
}

struct SwitchApp {
    menu: Menu<Msg>,
    bar: Hwnd,
    report: Rc<Cell<Report>>,
}

impl App for SwitchApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Open => {
                self.menu.set_property("open", Value::Bool(true));
            }
            Msg::Switch => {
                // Move the pointer across the bar while the menu is open; the
                // titles it crosses switch the popup from one menu to the next.
                for x in [70, 130, 160] {
                    let at = ((14 << 16) | x) as isize;
                    // SAFETY: the bar is a live window; the message carries
                    // only integer client coordinates.
                    unsafe {
                        let _ =
                            PostMessageW(Some(raw(self.bar)), WM_MOUSEMOVE, WPARAM(0), LPARAM(at));
                    }
                }
            }
            Msg::Check => {
                self.report.set(Report {
                    losses: FOREGROUND_LOSSES.with(Cell::get),
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
fn switching_menus_keeps_the_host_active() {
    let backend = Rc::new(Win32Backend::new());
    let backend_for_run: Rc<dyn Backend> = backend.clone();
    let report = Rc::new(Cell::new(Report::default()));
    let timed_out = Rc::new(Cell::new(false));

    let result = {
        let report = Rc::clone(&report);
        let timed_out = Rc::clone(&timed_out);
        xui_core::app::run_app(
            backend_for_run,
            PlatformSpec::new("xui.menu.switch").size(Dip(320.0), Dip(240.0)),
            move |ui| {
                let open = ui.set_timer(150);
                let switch = ui.set_timer(500);
                let check = ui.set_timer(1000);
                let watchdog = ui.set_timer(5000);
                let flag = Rc::clone(&timed_out);
                ui.on_timer(move |fired| {
                    if fired == open {
                        Some(Msg::Open)
                    } else if fired == switch {
                        Some(Msg::Switch)
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
                HOST.with(|host| host.set(owner.raw() as isize));
                // Watch foreground changes in this process. An out-of-context
                // hook delivers to this thread's message loop.
                // SAFETY: the callback lives for the process; no window or
                // module handle is captured.
                let _ = unsafe {
                    SetWinEventHook(
                        EVENT_SYSTEM_FOREGROUND,
                        EVENT_SYSTEM_FOREGROUND,
                        None,
                        Some(on_foreground),
                        GetCurrentProcessId(),
                        0,
                        WINEVENT_OUTOFCONTEXT,
                    )
                };
                let menu = Menu::bar(ui, Rect::new(0, 0, 260, 28))
                    .expect("create the menu bar")
                    .on_select(|_| None)
                    .build(|m| {
                        m.submenu(MenuId::new(0), "&File", |f| {
                            f.item(MenuId::new(1), "&New");
                        });
                        m.submenu(MenuId::new(2), "&Edit", |e| {
                            e.item(MenuId::new(3), "&Undo");
                        });
                        m.submenu(MenuId::new(4), "&View", |v| {
                            v.item(MenuId::new(5), "&Zoom");
                        });
                    });
                let bar = menu
                    .id()
                    .and_then(|id| backend.node_hwnd(id))
                    .expect("the bar has a window");
                SwitchApp { menu, bar, report }
            },
        )
    };

    if result.is_err() {
        return;
    }
    assert!(!timed_out.get(), "the watchdog fired; the app never quit");
    assert_eq!(
        report.get().losses,
        0,
        "the host lost the foreground while a menu was open; its chrome would flicker"
    );
}
