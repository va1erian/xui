#![forbid(unsafe_code)]

//! The file dialog's keyboard handling, attached to every node so the keys work
//! wherever the focus sits.
//!
//! Accepting is deliberately split from the keys: the buttons' own click
//! handlers and the list's activate mapping call
//! [`Shared::do_accept`](super::shared::Shared::do_accept), while this module
//! owns Escape, the filename field's Enter, Alt+Up and Backspace-to-parent.

use super::shared::Shared;
use crate::backend::Event;
use crate::message::Key;

/// Which node an [`on_key`] listener was attached to.
#[derive(Clone, Copy)]
pub(super) enum Role {
    /// The scrim (or a label): Escape and Alt+Up only.
    Scrim,
    /// The path bar: Enter navigates to the typed path.
    Path,
    /// The entry list: the list handles Enter through its activate mapping.
    List,
    /// The filename field: Enter accepts; an empty Backspace goes to the parent.
    Name,
    /// A button: its own click handler owns Enter; Escape still cancels.
    Button,
}

/// Handles the dialog's global keys while it is open.
pub(super) fn on_key<M: 'static>(shared: &Shared<M>, role: Role, event: &Event) -> Option<M> {
    if !shared.open.get() {
        return None;
    }
    if shared.ui.is_design_mode() && event.is_input() {
        return None;
    }
    let Event::KeyDown {
        key,
        modifiers,
        repeat,
        system,
    } = event
    else {
        return None;
    };
    if *repeat > 1 {
        return None;
    }
    if shared.state.borrow().is_confirming_overwrite() {
        return match *key {
            Key::ESCAPE if !*system => shared.do_cancel(),
            Key::RETURN if !*system && matches!(role, Role::Name | Role::Path | Role::List) => {
                shared.do_accept()
            }
            _ => None,
        };
    }
    match *key {
        Key::ESCAPE if !*system => shared.do_cancel(),
        Key::UP if modifiers.alt => {
            shared.state.borrow_mut().navigate_parent();
            shared.after_navigate();
            None
        }
        // The filename field keeps the focus while typing; Up/Down move the
        // list selection without leaving it.
        Key::DOWN if matches!(role, Role::Name) => {
            shared.state.borrow_mut().move_selection(1);
            shared.refresh_view();
            None
        }
        Key::UP if matches!(role, Role::Name) => {
            shared.state.borrow_mut().move_selection(-1);
            shared.refresh_view();
            None
        }
        Key::RETURN if !*system => match role {
            Role::Path => {
                shared.state.borrow_mut().navigate_typed();
                shared.after_navigate();
                None
            }
            Role::Name => shared.do_accept(),
            _ => None,
        },
        Key::BACK if matches!(role, Role::Name) => {
            let go_up = {
                let mut state = shared.state.borrow_mut();
                if state.name_empty() {
                    !state.take_just_cleared()
                } else {
                    false
                }
            };
            if go_up {
                shared.state.borrow_mut().navigate_parent();
                shared.after_navigate();
            }
            None
        }
        _ => None,
    }
}
