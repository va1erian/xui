#![forbid(unsafe_code)]

//! The retained scene: the nodes the front layer created, their parent links
//! and paint order, and the geometry questions asked of them (absolute
//! bounds, clips, visibility, hit testing).

use std::collections::{BTreeMap, HashMap};

use xui_core::backend::{Cursor, Painter, ParentRef, WidgetId, WindowId};
use xui_core::geometry::{Point, Rect};

/// One node the front layer created.
pub(super) struct Node {
    pub(super) window: WindowId,
    pub(super) parent: ParentRef,
    /// Relative to the parent.
    pub(super) bounds: Rect,
    pub(super) visible: bool,
    pub(super) enabled: bool,
    pub(super) text: String,
    pub(super) painter: Option<Painter>,
    /// Clips this node's descendants, in its own coordinates.
    pub(super) clip: Option<Rect>,
    pub(super) cursor: Cursor,
    pub(super) popup: bool,
    pub(super) tab_stop: bool,
    pub(super) wants_tab: bool,
    pub(super) contained: bool,
    seq: u64,
}

impl Node {
    pub(super) fn new(window: WindowId, parent: ParentRef, bounds: Rect) -> Node {
        Node {
            window,
            parent,
            bounds,
            visible: true,
            enabled: true,
            text: String::new(),
            painter: None,
            clip: None,
            cursor: Cursor::Default,
            popup: false,
            tab_stop: false,
            wants_tab: false,
            contained: false,
            seq: 0,
        }
    }
}

/// The nodes of every window, with paint order (later paints on top).
#[derive(Default)]
pub(super) struct Scene {
    nodes: HashMap<WidgetId, Node>,
    order: BTreeMap<u64, WidgetId>,
    children: HashMap<WidgetId, Vec<WidgetId>>,
    next_seq: u64,
}

impl Scene {
    /// Adds `node` as `id`, on top of every node there is.
    pub(super) fn insert(&mut self, id: WidgetId, mut node: Node) {
        if let ParentRef::Widget(parent) = node.parent {
            self.children.entry(parent).or_default().push(id);
        }
        node.seq = self.next_seq;
        self.order.insert(self.next_seq, id);
        self.next_seq += 1;
        self.nodes.insert(id, node);
    }

    pub(super) fn get(&self, id: WidgetId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub(super) fn get_mut(&mut self, id: WidgetId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    pub(super) fn window_of(&self, id: WidgetId) -> Option<WindowId> {
        self.get(id).map(|node| node.window)
    }

    /// Every node in paint order, bottom first.
    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = (WidgetId, &Node)> {
        self.order
            .values()
            .filter_map(|id| self.nodes.get(id).map(|node| (*id, node)))
    }

    /// Moves `id` above every other node; whether it moved.
    pub(super) fn raise(&mut self, id: WidgetId) -> bool {
        let top = self.order.last_key_value().map(|(_, top)| *top);
        let Some(node) = self.nodes.get_mut(&id).filter(|_| top != Some(id)) else {
            return false;
        };
        self.order.remove(&node.seq);
        node.seq = self.next_seq;
        self.order.insert(self.next_seq, id);
        self.next_seq += 1;
        true
    }

    /// Removes `id` and everything inside it; returns the removed ids.
    pub(super) fn remove(&mut self, id: WidgetId) -> Vec<WidgetId> {
        let mut removed = Vec::new();
        let mut pending = vec![id];
        while let Some(next) = pending.pop() {
            if self.nodes.contains_key(&next) {
                removed.push(next);
                pending.extend(self.children.remove(&next).unwrap_or_default());
            }
        }
        if let Some(ParentRef::Widget(parent)) = self.get(id).map(|node| node.parent)
            && let Some(siblings) = self.children.get_mut(&parent)
        {
            siblings.retain(|sibling| *sibling != id);
        }
        for gone in &removed {
            if let Some(node) = self.nodes.remove(gone) {
                self.order.remove(&node.seq);
            }
        }
        removed
    }

    /// Removes every node of `window`; returns their ids.
    pub(super) fn remove_window(&mut self, window: WindowId) -> Vec<WidgetId> {
        let roots: Vec<WidgetId> = self
            .nodes
            .iter()
            .filter(|(_, node)| {
                node.window == window && matches!(node.parent, ParentRef::Window(_))
            })
            .map(|(id, _)| *id)
            .collect();
        roots
            .into_iter()
            .flat_map(|root| self.remove(root))
            .collect()
    }

    /// `id`'s bounds in window coordinates.
    pub(super) fn absolute_bounds(&self, id: WidgetId) -> Option<Rect> {
        let node = self.get(id)?;
        let (mut left, mut top) = (0, 0);
        let mut parent = node.parent;
        while let ParentRef::Widget(parent_id) = parent {
            let ancestor = self.get(parent_id)?;
            left += ancestor.bounds.left;
            top += ancestor.bounds.top;
            parent = ancestor.parent;
        }
        Some(node.bounds.offset(left, top))
    }

    /// The window-coordinate rectangle `id`'s ancestors clip it to.
    pub(super) fn ancestor_clip(&self, id: WidgetId) -> Option<Rect> {
        let mut clip: Option<Rect> = None;
        let mut parent = self.get(id)?.parent;
        while let ParentRef::Widget(parent_id) = parent {
            let ancestor = self.get(parent_id)?;
            if let Some(local) = ancestor.clip {
                let origin = self.absolute_bounds(parent_id)?;
                let rect = Rect::new(
                    origin.left + local.left,
                    origin.top + local.top,
                    origin.left + local.right,
                    origin.top + local.bottom,
                );
                clip = Some(clip.map_or(rect, |current| intersect(current, rect)));
            }
            parent = ancestor.parent;
        }
        clip
    }

    /// Whether `id` and every ancestor satisfy `flag`.
    fn all_ancestry(&self, id: WidgetId, flag: impl Fn(&Node) -> bool) -> bool {
        let mut current = Some(id);
        while let Some(node_id) = current {
            let Some(node) = self.get(node_id) else {
                return false;
            };
            if !flag(node) {
                return false;
            }
            current = match node.parent {
                ParentRef::Window(_) => None,
                ParentRef::Widget(parent) => Some(parent),
            };
        }
        true
    }

    pub(super) fn effectively_visible(&self, id: WidgetId) -> bool {
        self.all_ancestry(id, |node| node.visible)
    }

    /// The topmost visible, enabled node of `window` under `(x, y)`, with the
    /// point in that node's own coordinates.
    pub(super) fn hit_test(
        &self,
        window: WindowId,
        x: i32,
        y: i32,
    ) -> Option<(WidgetId, i32, i32)> {
        let point = Point::new(x, y);
        self.iter().rev().find_map(|(id, node)| {
            if node.window != window || !self.all_ancestry(id, |n| n.visible && n.enabled) {
                return None;
            }
            let bounds = self.absolute_bounds(id)?;
            let inside = bounds.contains(point)
                && self
                    .ancestor_clip(id)
                    .is_none_or(|clip| clip.contains(point));
            inside.then_some((id, x - bounds.left, y - bounds.top))
        })
    }
}

pub(super) fn intersect(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.max(b.left),
        a.top.max(b.top),
        a.right.min(b.right),
        a.bottom.min(b.bottom),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: u64) -> WidgetId {
        WidgetId::from_raw(raw)
    }

    fn scene() -> (Scene, WindowId) {
        let window = WindowId::from_raw(1);
        let mut scene = Scene::default();
        scene.insert(
            id(1),
            Node::new(
                window,
                ParentRef::Window(window),
                Rect::new(10, 10, 110, 110),
            ),
        );
        scene.insert(
            id(2),
            Node::new(window, ParentRef::Widget(id(1)), Rect::new(5, 5, 45, 45)),
        );
        scene.insert(
            id(3),
            Node::new(window, ParentRef::Widget(id(1)), Rect::new(60, 60, 90, 90)),
        );
        (scene, window)
    }

    #[test]
    fn a_child_is_placed_relative_to_its_parent() {
        let (scene, _) = scene();
        assert_eq!(
            scene.absolute_bounds(id(2)),
            Some(Rect::new(15, 15, 55, 55))
        );
    }

    #[test]
    fn the_topmost_node_under_the_point_gets_the_hit_in_its_own_coordinates() {
        let (scene, window) = scene();
        assert_eq!(scene.hit_test(window, 20, 20), Some((id(2), 5, 5)));
        assert_eq!(scene.hit_test(window, 100, 100), Some((id(1), 90, 90)));
        assert_eq!(scene.hit_test(window, 200, 200), None);
    }

    #[test]
    fn a_hidden_ancestor_hides_its_descendants_from_hits() {
        let (mut scene, window) = scene();
        scene.get_mut(id(1)).unwrap().visible = false;
        assert!(!scene.effectively_visible(id(2)));
        assert_eq!(scene.hit_test(window, 20, 20), None);
    }

    #[test]
    fn an_ancestor_clip_limits_hits_and_is_reported_in_window_coordinates() {
        let (mut scene, window) = scene();
        scene.get_mut(id(1)).unwrap().clip = Some(Rect::new(0, 0, 30, 30));
        assert_eq!(scene.ancestor_clip(id(2)), Some(Rect::new(10, 10, 40, 40)));
        assert_eq!(scene.hit_test(window, 50, 50), Some((id(1), 40, 40)));
    }

    #[test]
    fn raising_a_node_paints_it_last() {
        let (mut scene, _) = scene();
        assert!(scene.raise(id(1)));
        assert_eq!(scene.iter().last().map(|(id, _)| id), Some(id(1)));
        assert!(!scene.raise(id(1)));
    }

    #[test]
    fn removing_a_node_removes_everything_inside_it() {
        let (mut scene, window) = scene();
        let mut removed = scene.remove(id(1));
        removed.sort_by_key(|id| id.raw());
        assert_eq!(removed, vec![id(1), id(2), id(3)]);
        assert_eq!(scene.iter().count(), 0);
        assert!(scene.remove_window(window).is_empty());
    }
}
