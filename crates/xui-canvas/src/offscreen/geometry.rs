#![forbid(unsafe_code)]

//! Node geometry for the offscreen backend: window-absolute bounds, the
//! ancestor clip chain, and event coordinate translation.

use xui_core::backend::{Event, ParentRef, WidgetId};
use xui_core::geometry::Rect;

use super::Node;

/// The window-absolute bounds of `id`, walking its parent chain.
pub(super) fn absolute_bounds(nodes: &[(WidgetId, Node)], id: WidgetId) -> Option<Rect> {
    let (_, node) = nodes.iter().find(|(node_id, _)| *node_id == id)?;
    Some(match node.parent {
        ParentRef::Window(_) => node.bounds,
        ParentRef::Widget(parent) => {
            let outer = absolute_bounds(nodes, parent)?;
            node.bounds.offset(outer.left, outer.top)
        }
    })
}

/// The intersection of two rectangles.
pub(super) fn intersect(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.max(b.left),
        a.top.max(b.top),
        a.right.min(b.right),
        a.bottom.min(b.bottom),
    )
}

/// The clip on `id` from its ancestors' clips, in window coordinates.
pub(super) fn ancestor_clip(nodes: &[(WidgetId, Node)], id: WidgetId) -> Option<Rect> {
    let (_, node) = nodes.iter().find(|(node_id, _)| *node_id == id)?;
    let mut clip: Option<Rect> = None;
    let mut parent = node.parent;
    while let ParentRef::Widget(parent_id) = parent {
        let Some((_, ancestor)) = nodes.iter().find(|(node_id, _)| *node_id == parent_id) else {
            break;
        };
        let Some(origin) = absolute_bounds(nodes, parent_id) else {
            break;
        };
        if let Some(local) = ancestor.clip {
            let rect = Rect::new(
                origin.left + local.left,
                origin.top + local.top,
                origin.left + local.right,
                origin.top + local.bottom,
            );
            clip = Some(match clip {
                Some(current) => intersect(current, rect),
                None => rect,
            });
        }
        parent = ancestor.parent;
    }
    clip
}

/// Rebuilds `event` with its cursor position at `(x, y)`.
pub(super) fn translate(event: Event, x: i32, y: i32) -> Event {
    match event {
        Event::MouseDown {
            button, modifiers, ..
        } => Event::MouseDown {
            x,
            y,
            button,
            modifiers,
        },
        Event::MouseUp {
            button, modifiers, ..
        } => Event::MouseUp {
            x,
            y,
            button,
            modifiers,
        },
        Event::MouseMove { modifiers, .. } => Event::MouseMove { x, y, modifiers },
        Event::MouseDoubleClick {
            button, modifiers, ..
        } => Event::MouseDoubleClick {
            x,
            y,
            button,
            modifiers,
        },
        Event::MouseWheel {
            delta,
            horizontal,
            modifiers,
            ..
        } => Event::MouseWheel {
            delta,
            horizontal,
            x,
            y,
            modifiers,
        },
        other => other,
    }
}
