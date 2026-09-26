#![forbid(unsafe_code)]

//! The dialog's keyboard handling: Escape cancels, Enter accepts.
//!
//! The same handler is attached to the scrim and every control, so the keys
//! work wherever the focus sits; a second dismissal after a button already
//! acted is ignored by [`dismiss`].

use super::{DialogAction, Shared};
use crate::backend::Event;
use crate::message::Key;

/// Handles Escape and Enter while the dialog is open.
pub(super) fn on_key<M: 'static>(shared: &Shared<M>, event: &Event) -> Option<M> {
    if !shared.open.get() {
        return None;
    }
    let Event::KeyDown {
        key,
        repeat,
        system,
        ..
    } = event
    else {
        return None;
    };
    if *repeat > 1 || *system {
        return None;
    }
    match *key {
        Key::ESCAPE => dismiss(shared, DialogAction::Cancel),
        Key::RETURN => accept_action(shared),
        _ => None,
    }
}

/// Dismisses with the affirmative action, carrying the prompt's text.
pub(super) fn accept_action<M: 'static>(shared: &Shared<M>) -> Option<M> {
    let text = shared.field.borrow().clone();
    dismiss(shared, DialogAction::Accept(text))
}

/// Dismisses with the cancel action.
pub(super) fn cancel_action<M: 'static>(shared: &Shared<M>) -> Option<M> {
    dismiss(shared, DialogAction::Cancel)
}

/// Hides the dialog and raises `action` once. A second dismissal (a listener
/// running after a button already acted) is ignored.
fn dismiss<M: 'static>(shared: &Shared<M>, action: DialogAction) -> Option<M> {
    if !shared.open.replace(false) {
        return None;
    }
    for id in shared.nodes.borrow().iter() {
        shared.ui.set_visible(*id, false);
    }
    let mapper = shared.action.borrow();
    mapper.as_ref().and_then(|mapper| mapper(action))
}
