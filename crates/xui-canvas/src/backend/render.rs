#![forbid(unsafe_code)]

//! Compositing a window's nodes into a [`Surface`], and resolving a node's
//! window-absolute bounds from its parent-relative ones.

use xui_core::backend::{Canvas as _, ParentRef, WidgetId};
use xui_core::geometry::Rect;

use super::{Node, Shared};
use crate::Surface;

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
fn intersect(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.max(b.left),
        a.top.max(b.top),
        a.right.min(b.right),
        a.bottom.min(b.bottom),
    )
}

/// The clip on `id`'s painting from its ancestors' [`Node::clip`]s, in window
/// coordinates, or `None` when no ancestor clips it.
///
/// A node's clip is in that node's own coordinate space, so it is lifted to
/// window coordinates with the node's absolute origin; descendants intersect
/// every ancestor's clip.
fn ancestor_clip(nodes: &[(WidgetId, Node)], id: WidgetId) -> Option<Rect> {
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

/// Fills `surface` with the window's background and runs each visible node's
/// painter at its absolute bounds, in creation order (later nodes on top),
/// clipped to its ancestors' clips.
pub(super) fn composite(
    shared: &Shared,
    window: xui_core::backend::WindowId,
    surface: &mut Surface,
) {
    let (background, dpi) = {
        let windows = shared.windows.borrow();
        let Some(state) = windows.get(&window.raw()) else {
            return;
        };
        (state.theme.background, state.dpi)
    };
    surface.fill(background);
    let paints: Vec<(Rect, Option<Rect>, xui_core::backend::Painter)> = {
        let nodes = shared.nodes.borrow();
        nodes
            .iter()
            .filter(|(_, node)| node.window == window && node.visible)
            .filter_map(|(id, node)| {
                let painter = node.painter.clone()?;
                let bounds = absolute_bounds(&nodes, *id)?;
                let clip = ancestor_clip(&nodes, *id);
                if let Some(clip) = clip
                    && intersect(bounds, clip).is_empty()
                {
                    return None;
                }
                Some((bounds, clip, painter))
            })
            .collect()
    };
    for (bounds, clip, painter) in paints {
        surface.with_canvas_at(bounds, dpi, |canvas| {
            if let Some(clip) = clip {
                canvas.push_clip(clip);
            }
            painter(canvas);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Node;
    use xui_core::backend::{WidgetId, WindowId};

    fn node(window: WindowId, parent: ParentRef, bounds: Rect) -> Node {
        Node {
            window,
            parent,
            bounds,
            visible: true,
            enabled: true,
            text: String::new(),
            painter: None,
            drag_region: false,
            clip: None,
        }
    }

    #[test]
    fn nested_offsets_accumulate_to_window_coordinates() {
        let window = WindowId::from_raw(1);
        let panel = WidgetId::from_raw(10);
        let child = WidgetId::from_raw(11);
        let nodes = vec![
            (
                panel,
                node(
                    window,
                    ParentRef::Window(window),
                    Rect::new(100, 50, 200, 90),
                ),
            ),
            (
                child,
                node(window, ParentRef::Widget(panel), Rect::new(5, 7, 25, 17)),
            ),
        ];

        assert_eq!(
            absolute_bounds(&nodes, panel),
            Some(Rect::new(100, 50, 200, 90))
        );
        assert_eq!(
            absolute_bounds(&nodes, child),
            Some(Rect::new(105, 57, 125, 67))
        );
    }

    #[test]
    fn an_ancestors_clip_bounds_a_childs_painted_area() {
        let window = WindowId::from_raw(1);
        let view = WidgetId::from_raw(20);
        let child = WidgetId::from_raw(21);
        let mut view_node = node(window, ParentRef::Window(window), Rect::new(0, 0, 100, 80));
        // The view clips its descendants to a 10px band at its top.
        view_node.clip = Some(Rect::new(0, 0, 100, 10));
        let nodes = vec![
            (view, view_node),
            (
                child,
                node(window, ParentRef::Widget(view), Rect::new(0, 5, 100, 40)),
            ),
        ];

        let clip = ancestor_clip(&nodes, child).expect("the child is clipped");
        let bounds = absolute_bounds(&nodes, child).unwrap();
        assert_eq!(
            intersect(bounds, clip),
            Rect::new(0, 5, 100, 10),
            "the child's bounds are chopped to the view's clip"
        );
    }

    #[test]
    fn a_child_outside_its_ancestors_clip_is_culled() {
        let window = WindowId::from_raw(1);
        let view = WidgetId::from_raw(30);
        let child = WidgetId::from_raw(31);
        let mut view_node = node(window, ParentRef::Window(window), Rect::new(0, 0, 100, 80));
        view_node.clip = Some(Rect::new(0, 0, 100, 10));
        let nodes = vec![
            (view, view_node),
            (
                child,
                node(window, ParentRef::Widget(view), Rect::new(0, 20, 100, 40)),
            ),
        ];

        let clip = ancestor_clip(&nodes, child).expect("the child is clipped");
        let bounds = absolute_bounds(&nodes, child).unwrap();
        assert!(
            intersect(bounds, clip).is_empty(),
            "a child entirely below the clip is culled"
        );
    }
}
