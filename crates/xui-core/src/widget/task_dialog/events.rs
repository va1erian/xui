#![forbid(unsafe_code)]

//! The task dialog's keyboard handling and its one-shot dismissal.

use super::{Shared, TaskDialogAction};
use crate::backend::Event;
use crate::message::Key;

/// Handles Escape and Enter while the dialog is open: Escape cancels, Enter
/// picks the first command.
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
        Key::ESCAPE => dismiss(shared, TaskDialogAction::Cancel),
        Key::RETURN if shared.command_count.get() > 0 => {
            dismiss(shared, TaskDialogAction::Command(0))
        }
        _ => None,
    }
}

/// Hides the dialog and raises `action` once. A second dismissal (a listener
/// running after a button already acted) is ignored.
pub(super) fn dismiss<M: 'static>(shared: &Shared<M>, action: TaskDialogAction) -> Option<M> {
    if !shared.open.replace(false) {
        return None;
    }
    for id in shared.nodes.borrow().iter() {
        shared.ui.set_visible(*id, false);
    }
    let mapper = shared.action.borrow();
    mapper.as_ref().and_then(|mapper| mapper(action))
}
