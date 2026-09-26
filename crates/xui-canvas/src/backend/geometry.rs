#![forbid(unsafe_code)]

//! Node geometry traversal shared by the windowed compositor and the offscreen
//! backend: a node's window-absolute bounds and the clip its ancestors impose.

use xui_core::backend::{ParentRef, WidgetId};
use xui_core::geometry::Rect;

use super::Node;

/// The placement facts the geometry traversal needs from a node, so the
/// windowed and offscreen nodes share one walk.
pub(crate) trait GeometryNode {
    /// The node this one is placed relative to.
    fn parent(&self) -> ParentRef;
    /// This node's bounds, relative to its parent.
    fn bounds(&self) -> Rect;
    /// A clip on this node's descendants, in this node's own coordinate space.
    fn clip(&self) -> Option<Rect>;
}

impl GeometryNode for Node {
    fn parent(&self) -> ParentRef {
        self.parent
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn clip(&self) -> Option<Rect> {
        self.clip
    }
}

/// The window-absolute bounds of `id`, walking its parent chain.
pub(crate) fn absolute_bounds<N: GeometryNode>(
    nodes: &[(WidgetId, N)],
    id: WidgetId,
) -> Option<Rect> {
    let (_, node) = nodes.iter().find(|(node_id, _)| *node_id == id)?;
    Some(match node.parent() {
        ParentRef::Window(_) => node.bounds(),
        ParentRef::Widget(parent) => {
            let outer = absolute_bounds(nodes, parent)?;
            node.bounds().offset(outer.left, outer.top)
        }
    })
}

/// The intersection of two rectangles.
pub(crate) fn intersect(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.max(b.left),
        a.top.max(b.top),
        a.right.min(b.right),
        a.bottom.min(b.bottom),
    )
}

/// The clip on `id`'s painting from its ancestors' [`GeometryNode::clip`]s, in
/// window coordinates, or `None` when no ancestor clips it.
///
/// A node's clip is in that node's own coordinate space, so it is lifted to
/// window coordinates with the node's absolute origin; descendants intersect
/// every ancestor's clip.
pub(crate) fn ancestor_clip<N: GeometryNode>(
    nodes: &[(WidgetId, N)],
    id: WidgetId,
) -> Option<Rect> {
    let (_, node) = nodes.iter().find(|(node_id, _)| *node_id == id)?;
    let mut clip: Option<Rect> = None;
    let mut parent = node.parent();
    while let ParentRef::Widget(parent_id) = parent {
        let Some((_, ancestor)) = nodes.iter().find(|(node_id, _)| *node_id == parent_id) else {
            break;
        };
        let Some(origin) = absolute_bounds(nodes, parent_id) else {
            break;
        };
        if let Some(local) = ancestor.clip() {
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
        parent = ancestor.parent();
    }
    clip
}
