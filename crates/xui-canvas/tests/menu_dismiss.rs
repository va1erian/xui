//! A menu closes when the user clicks outside it, on the backend a custom OS
//! such as LazyOS uses: the press moves the keyboard focus off the open popup,
//! and the popup treats that as a dismissal. The menu's own focus moves (to
//! its popups as it opens, to a submenu) are not.

use std::cell::Cell;
use std::rc::Rc;

use xui_canvas::OffscreenBackend;
use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::arrange::{Handle, LayoutExt, absolute, button, menu_bar};
use xui_core::backend::{Backend, Event, PlatformSpec, WindowId};
use xui_core::message::{Key, Modifiers, MouseButton};
use xui_core::widget::{Menu, MenuId};

struct Demo;

impl App for Demo {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

fn press(backend: &OffscreenBackend, window: WindowId, x: i32, y: i32) {
    for event in [
        Event::MouseMove {
            x,
            y,
            modifiers: Modifiers::NONE,
        },
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
        Event::MouseUp {
            x,
            y,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    ] {
        backend.inject(window, event);
    }
}

fn key(backend: &OffscreenBackend, window: WindowId, key: Key) {
    backend.inject(
        window,
        Event::KeyDown {
            key,
            modifiers: Modifiers::NONE,
            repeat: 1,
            system: false,
        },
    );
}

/// Runs `script` against a window with a File menu (holding a submenu) in a
/// bar at the top and a button below it; the rest of the window is empty.
fn session(script: impl FnOnce(&OffscreenBackend, WindowId, &Menu<()>) + 'static) {
    let backend = Rc::new(OffscreenBackend::new());
    let ran = Rc::new(Cell::new(false));
    let run: Rc<dyn Backend> = backend.clone();
    let (for_make, ran_for_make) = (Rc::clone(&backend), Rc::clone(&ran));
    run_app(
        run,
        PlatformSpec::new("menu dismiss").size(Dip(320.0), Dip(240.0)),
        move |ui| {
            let menu = Handle::new();
            ui.root(
                absolute()
                    .child(
                        menu_bar(|m| {
                            m.submenu(MenuId::new(0), "&File", |f| {
                                f.item(MenuId::new(1), "&New");
                                f.submenu(MenuId::new(2), "&More", |s| {
                                    s.item(MenuId::new(3), "&Deep");
                                });
                            });
                        })
                        .bind(&menu)
                        .at(0, 0, 260, 28),
                    )
                    .child(button("Other").at(20, 120, 100, 30)),
            )
            .expect("create the window");
            script(&for_make, ui.window(), &menu.get());
            ran_for_make.set(true);
            Demo
        },
    )
    .expect("the offscreen app ran");
    assert!(ran.get(), "the script ran");
}

#[test]
fn clicking_another_widget_closes_the_menu() {
    session(|backend, window, menu| {
        press(backend, window, 5, 14);
        assert!(menu.is_open(), "the File title opened its menu");

        press(backend, window, 50, 135);
        assert!(!menu.is_open(), "a click on another widget dismissed it");
    });
}

#[test]
fn opening_a_submenu_does_not_dismiss_the_menu() {
    session(|backend, window, menu| {
        press(backend, window, 5, 14);
        // The popup holds the focus, so the keys reach it: Down to "More",
        // Right to open it. The focus moves to the submenu popup.
        key(backend, window, Key::DOWN);
        key(backend, window, Key::DOWN);
        key(backend, window, Key::RIGHT);
        assert!(
            menu.is_open(),
            "moving the focus to the submenu is not a dismissal"
        );

        press(backend, window, 50, 135);
        assert!(!menu.is_open(), "the submenu level closed with the rest");
    });
}

#[test]
fn clicking_the_open_title_toggles_the_menu_closed() {
    session(|backend, window, menu| {
        press(backend, window, 5, 14);
        assert!(menu.is_open());
        press(backend, window, 5, 14);
        assert!(!menu.is_open(), "the second press closed it");
        press(backend, window, 5, 14);
        assert!(menu.is_open(), "and the third reopened it");
    });
}

#[test]
fn clicking_empty_window_area_closes_the_menu() {
    session(|backend, window, menu| {
        press(backend, window, 5, 14);
        assert!(menu.is_open());

        press(backend, window, 250, 200);
        assert!(!menu.is_open(), "a click on bare window dismissed it");
    });
}
