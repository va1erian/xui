#![forbid(unsafe_code)]

//! Compositing a window's nodes into a [`Surface`], and resolving a node's
//! window-absolute bounds from its parent-relative ones.

use xui_core::backend::{ParentRef, WidgetId};
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

/// Fills `surface` with the window's background and runs each visible node's
/// painter at its absolute bounds, in creation order (later nodes on top).
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
    let paints: Vec<(Rect, xui_core::backend::Painter)> = {
        let nodes = shared.nodes.borrow();
        nodes
            .iter()
            .filter(|(_, node)| node.window == window && node.visible)
            .filter_map(|(id, node)| {
                let painter = node.painter.clone()?;
                Some((absolute_bounds(&nodes, *id)?, painter))
            })
            .collect()
    };
    for (bounds, painter) in paints {
        surface.with_canvas_at(bounds, dpi, |canvas| painter(canvas));
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
}
