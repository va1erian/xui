#![forbid(unsafe_code)]

//! [`Menu`](super::Menu) input: bar and popup navigation, mnemonics and
//! activation. The popup stack itself lives in [`super::open`].

use std::rc::Rc;

use super::model::{self, Kind, Node};
use super::open::{
    close_all, close_levels_from, focus_level, invalidate_level, level_path, open_bar, open_submenu,
};
use super::{MenuId, Runtime, layout};
use crate::backend::Event;
use crate::message::{Key, MouseButton};

/// Handles an event on the menu bar.
pub(super) fn bar<M: 'static>(rt: &Rc<Runtime<M>>, event: &Event) -> Option<M> {
    if rt.ui.is_design_mode() && event.is_input() {
        return None;
    }
    let bar = rt.bar.get()?;
    let dpi = rt.ui.dpi();
    match event {
        Event::MouseMove { x, .. } => {
            let index = layout::title_at(&rt.model.borrow(), *x, dpi);
            if rt.view.borrow().bar_hover != index {
                rt.view.borrow_mut().bar_hover = index;
                rt.ui.invalidate(bar);
            }
            if let Some(index) = index
                && rt.view.borrow().bar_open.is_some()
            {
                open_bar(rt, index);
            }
            None
        }
        Event::MouseLeave | Event::CaptureChanged => {
            if rt.view.borrow_mut().bar_hover.take().is_some() {
                rt.ui.invalidate(bar);
            }
            None
        }
        Event::MouseDown {
            x,
            button: MouseButton::Left,
            ..
        } => {
            if let Some(index) = layout::title_at(&rt.model.borrow(), *x, dpi) {
                rt.ui.focus(bar);
                if rt.view.borrow().bar_open == Some(index) {
                    close_all(rt);
                } else {
                    open_bar(rt, index);
                }
            }
            None
        }
        Event::KeyDown {
            key,
            repeat,
            system,
            ..
        } if *repeat <= 1 && !*system => bar_key(rt, *key),
        Event::Char(character) => bar_mnemonic(rt, *character),
        _ => None,
    }
}

/// Handles an event on the popup at nesting `depth`.
pub(super) fn popup<M: 'static>(rt: &Rc<Runtime<M>>, depth: usize, event: &Event) -> Option<M> {
    if rt.ui.is_design_mode() && event.is_input() {
        return None;
    }
    let _ = rt.view.borrow().levels.get(depth)?;
    match event {
        Event::MouseMove { y, .. } => {
            if let Some(index) = row_at(rt, depth, *y) {
                hover(rt, depth, index);
            }
            None
        }
        Event::MouseDown {
            y,
            button: MouseButton::Left,
            ..
        } => {
            let index = row_at(rt, depth, *y)?;
            activate(rt, depth, index)
        }
        Event::KeyDown {
            key,
            repeat,
            system,
            ..
        } if *repeat <= 1 && !*system => popup_key(rt, depth, *key),
        Event::Char(character) => popup_mnemonic(rt, depth, *character),
        _ => None,
    }
}

/// Handles a key on the popup at `depth`.
fn popup_key<M: 'static>(rt: &Rc<Runtime<M>>, depth: usize, key: Key) -> Option<M> {
    match key {
        Key::UP => {
            move_hover(rt, depth, -1);
            None
        }
        Key::DOWN => {
            move_hover(rt, depth, 1);
            None
        }
        Key::RIGHT => {
            let hovered = rt.view.borrow().levels.get(depth).and_then(|l| l.hover);
            if let Some(index) = hovered
                && node_info(rt, depth, index)
                    .is_some_and(|info| info.kind == Kind::Submenu && info.enabled)
            {
                open_submenu(rt, depth, index);
            }
            None
        }
        Key::LEFT => {
            if depth == 0 {
                close_all(rt);
                if let Some(bar) = rt.bar.get() {
                    rt.ui.focus(bar);
                }
            } else {
                close_levels_from(rt, depth);
                focus_level(rt, depth - 1);
            }
            None
        }
        Key::RETURN => {
            let hovered = rt.view.borrow().levels.get(depth).and_then(|l| l.hover);
            hovered.and_then(|index| activate(rt, depth, index))
        }
        Key::ESCAPE => {
            close_all(rt);
            None
        }
        _ => None,
    }
}

/// Handles a typed character as a popup mnemonic.
fn popup_mnemonic<M: 'static>(rt: &Rc<Runtime<M>>, depth: usize, character: char) -> Option<M> {
    let key = character.to_ascii_lowercase();
    let path = level_path(rt, depth)?;
    let index = {
        let model = rt.model.borrow();
        model::with_entries(&model, &path, |entries| {
            entries
                .iter()
                .position(|node| node.enabled && node.key == Some(key))
        })
        .flatten()
    };
    index.and_then(|index| activate(rt, depth, index))
}

/// Handles a key on the bar.
fn bar_key<M: 'static>(rt: &Rc<Runtime<M>>, key: Key) -> Option<M> {
    let count = rt.model.borrow().len();
    if count == 0 {
        return None;
    }
    let bar = rt.bar.get()?;
    match key {
        Key::LEFT | Key::RIGHT => {
            let current = rt.view.borrow().bar_hover.unwrap_or(0);
            let next = match key {
                Key::LEFT => current.saturating_sub(1),
                _ => (current + 1).min(count - 1),
            };
            rt.view.borrow_mut().bar_hover = Some(next);
            if rt.view.borrow().bar_open.is_some() {
                open_bar(rt, next);
            }
            rt.ui.invalidate(bar);
            None
        }
        Key::DOWN | Key::RETURN => {
            let index = rt.view.borrow().bar_hover.unwrap_or(0);
            open_bar(rt, index);
            None
        }
        Key::ESCAPE => {
            close_all(rt);
            None
        }
        _ => None,
    }
}

/// Handles a typed character as a bar mnemonic.
fn bar_mnemonic<M: 'static>(rt: &Rc<Runtime<M>>, character: char) -> Option<M> {
    let key = character.to_ascii_lowercase();
    let index = {
        let model = rt.model.borrow();
        model
            .iter()
            .position(|node| node.enabled && node.key == Some(key))
    };
    if let Some(index) = index {
        open_bar(rt, index);
    }
    None
}

/// Moves the popup's highlight `delta` selectable entries.
fn move_hover<M: 'static>(rt: &Rc<Runtime<M>>, depth: usize, delta: i32) {
    let Some(path) = level_path(rt, depth) else {
        return;
    };
    let current = rt.view.borrow().levels.get(depth).and_then(|l| l.hover);
    let next = {
        let model = rt.model.borrow();
        model::with_entries(&model, &path, |entries| {
            model::step(entries, current, delta)
        })
        .flatten()
    };
    if let Some(next) = next {
        if let Some(level) = rt.view.borrow_mut().levels.get_mut(depth) {
            level.hover = Some(next);
        }
        invalidate_level(rt, depth);
    }
}

/// Highlights entry `index`, opening a submenu or closing deeper levels.
fn hover<M: 'static>(rt: &Rc<Runtime<M>>, depth: usize, index: usize) {
    if let Some(level) = rt.view.borrow_mut().levels.get_mut(depth) {
        level.hover = Some(index);
    }
    invalidate_level(rt, depth);
    match node_info(rt, depth, index) {
        Some(info) if info.kind == Kind::Submenu && info.enabled => open_submenu(rt, depth, index),
        _ => {
            let had_submenu = rt.view.borrow().levels.len() > depth + 1;
            close_levels_from(rt, depth + 1);
            // The closed submenu held the host's keyboard focus (a native popup
            // is focused logically, not by taking activation); hand it back to
            // this level, or the open menu would stop receiving keys.
            if had_submenu {
                focus_level(rt, depth);
            }
        }
    }
}

/// Activates the entry `index` at `depth`, raising a message for a command.
fn activate<M: 'static>(rt: &Rc<Runtime<M>>, depth: usize, index: usize) -> Option<M> {
    let info = node_info(rt, depth, index)?;
    if !info.enabled || info.kind == Kind::Separator {
        return None;
    }
    match info.kind {
        Kind::Submenu => {
            open_submenu(rt, depth, index);
            None
        }
        Kind::Command => {
            close_all(rt);
            emit_select(rt, info.id)
        }
        Kind::Check => {
            let checked = !info.checked;
            set_checked_at(rt, depth, index, checked);
            close_all(rt);
            emit_toggle(rt, info.id, checked)
        }
        Kind::Radio => {
            set_checked_at(rt, depth, index, true);
            close_all(rt);
            emit_toggle(rt, info.id, true)
        }
        Kind::Separator => None,
    }
}

/// Toggles the check state of the entry at `depth`/`index`.
fn set_checked_at<M: 'static>(rt: &Runtime<M>, depth: usize, index: usize, checked: bool) {
    let Some(path) = level_path(rt, depth) else {
        return;
    };
    let mut model = rt.model.borrow_mut();
    let _ = model::with_entries_mut(&mut model, &path, |entries| {
        if checked && entries.get(index).map(|node| node.kind) == Some(Kind::Radio) {
            for node in entries.iter_mut() {
                if node.kind == Kind::Radio {
                    node.checked = false;
                }
            }
        }
        if let Some(node) = entries.get_mut(index) {
            node.checked = checked;
        }
    });
}

/// Raises a command through [`Menu::on_select`](super::Menu::on_select).
fn emit_select<M: 'static>(rt: &Runtime<M>, id: MenuId) -> Option<M> {
    let mapper = rt.on_select.borrow();
    mapper.as_ref().and_then(|mapper| mapper(id))
}

/// Raises a check/radio toggle through
/// [`Menu::on_toggle`](super::Menu::on_toggle), falling back to `on_select`.
fn emit_toggle<M: 'static>(rt: &Runtime<M>, id: MenuId, checked: bool) -> Option<M> {
    if rt.on_toggle.borrow().is_some() {
        let mapper = rt.on_toggle.borrow();
        return mapper.as_ref().and_then(|mapper| mapper(id, checked));
    }
    emit_select(rt, id)
}

/// The row under the popup-local `y` at `depth`.
fn row_at<M: 'static>(rt: &Runtime<M>, depth: usize, y: i32) -> Option<usize> {
    let path = level_path(rt, depth)?;
    let model = rt.model.borrow();
    model::with_entries(&model, &path, |entries| {
        layout::row_at(entries, rt.ui.dpi(), y)
    })
    .flatten()
}

/// The plain facts of the entry at `depth`/`index`.
fn node_info<M: 'static>(rt: &Runtime<M>, depth: usize, index: usize) -> Option<model::Info> {
    let path = level_path(rt, depth)?;
    let model = rt.model.borrow();
    model::with_entries(&model, &path, |entries| entries.get(index).map(Node::info)).flatten()
}
