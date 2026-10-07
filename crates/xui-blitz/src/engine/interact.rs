//! The user's input to the page, and what the pointer is over.

use blitz_dom::{BaseDocument, local_name};
use xui_core::message::Key;

use super::Engine;
use super::input::{self, Input, PointerAction};
use crate::view::BlitzViewEvent;

/// CSS pixels one arrow key scrolls.
const KEY_LINE: f64 = 40.0;

impl Engine {
    pub(super) fn input(&mut self, input: Input) {
        let Some(d) = &mut self.doc else {
            return;
        };
        if let Input::Key { key, .. } = &input
            && !text_input_focused(&d.doc)
            && let Some(dy) = key_scroll(*key, self.size.1 as f64 / self.scale as f64)
        {
            d.doc.scroll_viewport_by(0.0, dy);
            self.dirty = true;
            return;
        }
        let scroll = d.doc.viewport_scroll();
        let Some(event) = input::to_ui_event(&input, (scroll.x, scroll.y)) else {
            return;
        };
        blitz_dom::Document::handle_ui_event(&mut d.doc, event);
        if matches!(input, Input::Pointer { .. }) {
            let link = hovered_link(&d.doc).unwrap_or_default();
            self.set_status(link);
        }
        if !matches!(
            input,
            Input::Pointer {
                action: PointerAction::Move,
                ..
            }
        ) {
            self.dirty = true;
        }
    }

    pub(super) fn set_status(&mut self, status: String) {
        if status != self.status {
            self.status.clone_from(&status);
            self.report.event(BlitzViewEvent::StatusChanged(status));
        }
    }
}

/// Whether the focused element takes typed text (so the movement keys are
/// its, not the page's).
fn text_input_focused(doc: &BaseDocument) -> bool {
    doc.get_focussed_node_id()
        .and_then(|id| doc.get_node(id))
        .and_then(|n| n.element_data())
        .is_some_and(|e| e.text_input_data().is_some())
}

/// How far a movement key scrolls the page, given the viewport's height in
/// CSS pixels.
fn key_scroll(key: Key, page: f64) -> Option<f64> {
    Some(match key {
        Key::UP => -KEY_LINE,
        Key::DOWN => KEY_LINE,
        Key::PAGE_UP => -page * 0.9,
        Key::PAGE_DOWN => page * 0.9,
        Key::HOME => f64::MIN / 4.0,
        Key::END => f64::MAX / 4.0,
        _ => return None,
    })
}

/// The URL of the link under the pointer, if any.
fn hovered_link(doc: &BaseDocument) -> Option<String> {
    let mut id = doc.get_hover_node_id();
    while let Some(node) = id.and_then(|i| doc.get_node(i)) {
        if node
            .element_data()
            .is_some_and(|e| e.name.local == local_name!("a"))
            && let Some(href) = node.attr(local_name!("href"))
        {
            return doc.url().join(href).ok().map(|u| u.to_string());
        }
        id = node.parent;
    }
    None
}
