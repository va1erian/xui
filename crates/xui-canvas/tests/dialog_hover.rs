//! Moving the pointer over an open modal dialog repaints what changed, not
//! the window.
//!
//! The dialog's scrim spans the window and only catches input. The backends
//! repaint the target of a `MouseLeave` as a safety net, so before the scrim
//! was `NodeSpec::steady` every move from the scrim onto one of the dialog's
//! buttons repainted the whole window under a translucent layer.

use std::cell::RefCell;
use std::rc::Rc;

use xui_canvas::OffscreenBackend;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, Event, PlatformSpec};
use xui_core::message::Modifiers;
use xui_core::prelude::*;
use xui_core::widget::Dialog;
use xui_core::{Dip, Rect};

const SIZE: (i32, i32) = (800, 600);

struct Nothing;

impl App for Nothing {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// The window and the dialog the app built, handed to the run hook.
type Built = (Ui<()>, Dialog<()>);

/// The damage of every frame of a pointer sweep over the window, taken with
/// `dialog` open.
fn sweep(make: fn(&Ui<()>) -> Dialog<()>) -> Vec<Vec<Rect>> {
    let backend = Rc::new(OffscreenBackend::new());
    let built: Rc<RefCell<Option<Built>>> = Rc::new(RefCell::new(None));
    let frames: Rc<RefCell<Vec<Vec<Rect>>>> = Rc::new(RefCell::new(Vec::new()));
    let (weak, slot, out) = (
        Rc::downgrade(&backend),
        Rc::clone(&built),
        Rc::clone(&frames),
    );
    backend.set_run_hook(move || {
        let backend = weak.upgrade().expect("the backend");
        let (ui, dialog) = slot.borrow_mut().take().expect("the app was built");
        let window = ui.window();
        dialog.open();
        backend.pump(window);
        backend.present(window, |_| {});
        for y in (4..SIZE.1).step_by(8) {
            for x in (4..SIZE.0).step_by(8) {
                backend.inject(
                    window,
                    Event::MouseMove {
                        x,
                        y,
                        modifiers: Modifiers::NONE,
                    },
                );
                backend.pump(window);
                backend.present(window, |frame| {
                    out.borrow_mut().push(frame.damage.to_vec());
                });
            }
        }
    });
    let slot = Rc::clone(&built);
    let run: Rc<dyn Backend> = backend.clone();
    run_app(
        run,
        PlatformSpec::new("dialog hover").size(Dip(SIZE.0 as f32), Dip(SIZE.1 as f32)),
        move |ui| {
            ui.root(column().padding(16).gap(8).children((
                label("behind the dialog"),
                button("One"),
                button("Two"),
            )))
            .expect("the page mounts");
            *slot.borrow_mut() = Some((ui.clone(), make(ui)));
            Nothing
        },
    )
    .expect("the app ran");
    frames.take()
}

fn assert_local(frames: &[Vec<Rect>]) {
    let window = i64::from(SIZE.0) * i64::from(SIZE.1);
    let area = |frame: &Vec<Rect>| -> i64 {
        frame
            .iter()
            .map(|rect| i64::from(rect.width()) * i64::from(rect.height()))
            .sum()
    };
    let hovered = frames.iter().filter(|frame| !frame.is_empty()).count();
    assert!(hovered > 0, "no button showed a hover during the sweep");
    let largest = frames.iter().map(area).max().unwrap_or(0);
    assert!(
        largest * 4 < window,
        "a pointer move repainted {largest} px of a {window} px window"
    );
}

#[test]
fn hovering_a_confirm_dialogs_buttons_does_not_repaint_the_window() {
    assert_local(&sweep(|ui| {
        Dialog::confirm(ui, "Save changes?", "Your edits will be lost otherwise.")
            .expect("the dialog builds")
    }));
}

#[test]
fn hovering_a_prompt_dialogs_buttons_does_not_repaint_the_window() {
    assert_local(&sweep(|ui| {
        Dialog::prompt(ui, "Rename", "The new name:", "untitled").expect("the dialog builds")
    }));
}
