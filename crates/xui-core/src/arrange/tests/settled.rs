//! A re-flow that changes nothing costs the backend nothing: no move batch,
//! no repaint (va1erian/xui#277).

use super::{bounds, settle, setup};
use crate::arrange::{Handle, button, label, row};
use crate::widget::{Button, HasText, Label};

#[test]
fn a_relayout_that_moves_nothing_sends_no_moves() {
    let (backend, window, ui, _runtime) = setup();
    let (name, ok) = (Handle::<Label<u32>>::new(), Handle::<Button<u32>>::new());
    let _mounted = ui
        .mount(
            row()
                .gap(4)
                .children((label("Name").bind(&name), button("OK").bind(&ok))),
        )
        .unwrap();
    let placed = backend.move_calls();
    ui.relayout();
    assert_eq!(backend.move_calls(), placed, "nothing moved, nothing sent");

    // A longer text widens the label and pushes the button along.
    let before = bounds(&ui, &ok);
    name.get().set_text("A much longer name");
    settle(&backend, window);
    assert_eq!(backend.move_calls(), placed + 1);
    assert!(bounds(&ui, &ok).left > before.left);
}

#[test]
fn setting_the_text_a_widget_already_shows_changes_nothing() {
    let (backend, window, ui, _runtime) = setup();
    let (name, ok) = (Handle::<Label<u32>>::new(), Handle::<Button<u32>>::new());
    let _mounted = ui
        .mount(row().children((label("Ready").bind(&name), button("OK").bind(&ok))))
        .unwrap();
    let (moves, repaints) = (backend.move_calls(), backend.invalidations());
    name.get().set_text("Ready");
    ok.get().set_text("OK");
    settle(&backend, window);
    assert_eq!(backend.invalidations(), repaints, "no repaint");
    assert_eq!(backend.move_calls(), moves, "no re-flow moved anything");
}
