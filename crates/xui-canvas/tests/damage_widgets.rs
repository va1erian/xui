//! Damage-driven repainting with the real widgets: a scripted session (the
//! pointer sweeping the window, clicks, typing, wheel, a theme change, value
//! changes) is run twice, once repainting from damage only and once repainting
//! the whole window before every frame, and the frames must be identical
//! pixel for pixel, on the flat light and dark themes and on Midnight, whose
//! rounded cards, gradients and accent glows are what overdraw and clip masks
//! look like in practice.

use std::cell::RefCell;
use std::rc::Rc;

use xui_canvas::OffscreenBackend;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, Event, PlatformSpec, WindowId};
use xui_core::message::{Key, Modifiers, MouseButton};
use xui_core::prelude::*;
use xui_core::widget::Glyph;
use xui_core::{Dip, Theme};

struct Demo {
    /// Widgets that no layout holds are kept alive for the run.
    _alive: Vec<Box<dyn std::any::Any>>,
}

impl App for Demo {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

const W: i32 = 720;
const H: i32 = 560;

/// The window's widgets: a spread of every kind that paints differently on
/// hover, press, focus and selection.
fn layout() -> Layout<()> {
    column().padding(12).gap(8).children((
        label("Damage differential").title(),
        row().gap(10).children((
            column().gap(8).children((
                edit().text("type here"),
                checkbox("Enabled"),
                checkbox("Notifications"),
                toggle_button("Bold"),
                radio_group(&["Small", "Medium", "Large"]),
                combo_box(&["Alpha", "Beta", "Gamma"]),
                slider(0.0, 100.0),
                progress(100).height(8),
                row()
                    .gap(6)
                    .children((button("OK").primary(), button("Cancel"))),
            )),
            column().gap(8).children((
                list().height(90),
                panel(column().padding(10).children((
                    label("In a panel"),
                    button("Inside"),
                    checkbox("Nested check"),
                )))
                .height(110),
                tabs()
                    .page("General", column().child(label("General page")))
                    .page("More", column().child(button("More")))
                    .height(110),
                scroll(column().children((
                    label("Row 1").fixed(34),
                    label("Row 2").fixed(34),
                    label("Row 3").fixed(34),
                    label("Row 4").fixed(34),
                    label("Row 5").fixed(34),
                )))
                .height(96),
            )),
        )),
        top_bar().then(|bar| {
            bar.icon(TopBarId::new(1), Glyph::Menu)
                .toggle(TopBarId::new(2), Glyph::Star)
                .label(TopBarId::new(3), "bar")
        }),
        status_bar(&["Ready", ""]),
    ))
}

/// The pointer positions of the sweep, row by row.
fn sweep() -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    for y in (20..H).step_by(37) {
        for x in (10..W).step_by(61) {
            points.push((x, y));
        }
    }
    points
}

fn mouse_move(x: i32, y: i32) -> Event {
    Event::MouseMove {
        x,
        y,
        modifiers: Modifiers::NONE,
    }
}

fn click(backend: &OffscreenBackend, window: WindowId, x: i32, y: i32) {
    for event in [
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
        backend.pump(window);
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
    backend.pump(window);
}

/// What the script drives: the window and two widgets it changes from code.
type Driven = (Ui<()>, Handle<ProgressBar<()>>, Handle<CheckBox<()>>);

/// Runs the scripted session and returns the frame after every step. With
/// `reference`, the whole window is marked stale before each frame.
fn session(theme: Theme, reference: bool) -> Vec<Vec<u8>> {
    let backend = Rc::new(OffscreenBackend::new());
    let frames: Rc<RefCell<Vec<Vec<u8>>>> = Rc::new(RefCell::new(Vec::new()));
    let slot: Rc<RefCell<Option<Driven>>> = Rc::new(RefCell::new(None));

    let weak = Rc::downgrade(&backend);
    let hook_frames = Rc::clone(&frames);
    let hook_slot = Rc::clone(&slot);
    backend.set_run_hook(move || {
        let (Some(backend), Some((ui, progress, check))) =
            (weak.upgrade(), hook_slot.borrow_mut().take())
        else {
            return;
        };
        let window = ui.window();
        let snap = |backend: &OffscreenBackend| {
            backend.pump(window);
            if reference {
                backend.invalidate_window(window);
            }
            let image = backend.render(window).expect("a frame");
            hook_frames.borrow_mut().push(image.pixels);
        };
        snap(&backend);

        // The pointer sweeps every widget, a click now and then.
        for (step, (x, y)) in sweep().into_iter().enumerate() {
            backend.inject(window, mouse_move(x, y));
            snap(&backend);
            if step % 9 == 4 {
                click(&backend, window, x, y);
                snap(&backend);
            }
        }
        // Typing into the edit, after focusing it.
        click(&backend, window, 60, 70);
        snap(&backend);
        for ch in "damage".chars() {
            backend.inject(window, Event::Char(ch));
            snap(&backend);
        }
        for k in [Key::LEFT, Key::HOME, Key::END] {
            key(&backend, window, k);
            snap(&backend);
        }
        // Values changed from code.
        for value in [10, 55, 100] {
            progress.get().set_value(value);
            snap(&backend);
        }
        check.get().set_checked(true);
        snap(&backend);
        // The wheel over the scroll area.
        for delta in [-3, -3, 3] {
            backend.inject(
                window,
                Event::MouseWheel {
                    delta,
                    horizontal: false,
                    x: 520,
                    y: 420,
                    modifiers: Modifiers::NONE,
                },
            );
            snap(&backend);
        }
        // A theme change repaints everything, then a few more interactions.
        for next in [Theme::midnight(), Theme::dark(), Theme::light()] {
            ui.set_theme(next);
            snap(&backend);
            backend.inject(window, mouse_move(120, 150));
            snap(&backend);
        }
    });

    let make_slot = Rc::clone(&slot);
    let run: Rc<dyn Backend> = backend.clone();
    run_app(
        run,
        PlatformSpec::new("damage widgets").size(Dip(W as f32), Dip(H as f32)),
        move |ui| {
            ui.set_theme(theme);
            let progress_handle = Handle::new();
            let check = Handle::new();
            ui.root(column().children((
                layout().fill(1),
                row().children((
                    progress(100).bind(&progress_handle).width(200).height(8),
                    checkbox("Driven from code").bind(&check),
                )),
            )))
            .unwrap();
            *make_slot.borrow_mut() = Some((ui.clone(), progress_handle, check));
            Demo { _alive: Vec::new() }
        },
    )
    .expect("the session ran");
    slot.borrow_mut().take();
    frames.take()
}

fn assert_identical(theme: Theme, name: &str) {
    let region = session(theme, false);
    let full = session(theme, true);
    assert_eq!(region.len(), full.len(), "{name}: the same steps ran");
    assert!(region.len() > 100, "{name}: a long session");
    let mut distinct: Vec<&Vec<u8>> = Vec::new();
    for frame in &region {
        if !distinct.contains(&frame) {
            distinct.push(frame);
        }
    }
    assert!(
        distinct.len() >= 20,
        "{name}: the session changed the window ({} distinct frames)",
        distinct.len()
    );
    for (step, (got, want)) in region.iter().zip(&full).enumerate() {
        if got != want {
            let at = got.iter().zip(want).position(|(a, b)| a != b).unwrap() / 4;
            panic!(
                "{name}: step {step} differs from a full repaint at ({}, {})",
                at as i32 % W,
                at as i32 / W
            );
        }
    }
}

#[test]
fn a_real_widget_session_repaints_identically_on_the_light_theme() {
    assert_identical(Theme::light(), "light");
}

#[test]
fn a_real_widget_session_repaints_identically_on_the_dark_theme() {
    assert_identical(Theme::dark(), "dark");
}

#[test]
fn a_real_widget_session_repaints_identically_on_midnight() {
    assert_identical(Theme::midnight(), "midnight");
}
