#![forbid(unsafe_code)]

//! The retained scene both canvas backends keep: the nodes the front layer
//! created, by id, with their paint order and their parent/child links.
//!
//! A window's nodes paint in creation order (later on top) and
//! [`Backend::raise`](xui_core::backend::Backend::raise) moves a node to the
//! end of that order. The scene keeps the order as a sequence number per node
//! in an ordered map, and the nodes in a hash map, so a lookup by id, a raise,
//! a destroy and a walk down a subtree are logarithmic or linear in what they
//! touch instead of a scan of every node, and a frame can plan its nodes in
//! one pass (see [`crate::compositor`]).
//!
//! It lives at the crate root, not under `backend`, so the offscreen backend
//! (which compiles with the `winit-backend` feature off) shares it.

use std::collections::{BTreeMap, HashMap};

use xui_core::backend::{Painter, ParentRef, WidgetId, WindowId};
use xui_core::geometry::Rect;

/// One node the front layer created.
pub(crate) struct Node {
    /// The window the node belongs to.
    pub(crate) window: WindowId,
    /// What the node is placed relative to: the window or a container node.
    pub(crate) parent: ParentRef,
    /// The bounds, relative to the parent.
    pub(crate) bounds: Rect,
    /// Whether the node itself is shown (ignoring its ancestors).
    pub(crate) visible: bool,
    /// Whether the node itself is enabled (ignoring its ancestors).
    pub(crate) enabled: bool,
    /// The text the front layer stored on the node.
    pub(crate) text: String,
    /// The draw routine, once the widget registered one.
    pub(crate) painter: Option<Painter>,
    /// Whether a press that starts here drags the whole window (the windowed
    /// backend's custom caption).
    #[cfg(feature = "winit-backend")]
    pub(crate) drag_region: bool,
    /// A clip on this node's descendants, in the node's own coordinate space.
    pub(crate) clip: Option<Rect>,
}

impl Node {
    /// A visible, enabled node with no painter, text, clip or drag behaviour.
    pub(crate) fn new(window: WindowId, parent: ParentRef, bounds: Rect) -> Node {
        Node {
            window,
            parent,
            bounds,
            visible: true,
            enabled: true,
            text: String::new(),
            painter: None,
            #[cfg(feature = "winit-backend")]
            drag_region: false,
            clip: None,
        }
    }
}

struct Slot {
    node: Node,
    /// The node's place in the paint order.
    seq: u64,
}

/// The nodes of every window of one backend.
#[derive(Default)]
pub(crate) struct Scene {
    nodes: HashMap<WidgetId, Slot>,
    /// Paint order: ascending sequence number.
    order: BTreeMap<u64, WidgetId>,
    next_seq: u64,
    /// The nodes created directly inside a container node, in no order.
    children: HashMap<WidgetId, Vec<WidgetId>>,
}

impl Scene {
    /// An empty scene.
    pub(crate) fn new() -> Scene {
        Scene::default()
    }

    /// How many nodes the scene holds, across windows.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Adds `node` as `id`, on top of every node there is.
    pub(crate) fn insert(&mut self, id: WidgetId, node: Node) {
        if self.nodes.contains_key(&id) {
            self.unlink(id);
        }
        if let ParentRef::Widget(parent) = node.parent {
            self.children.entry(parent).or_default().push(id);
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.order.insert(seq, id);
        self.nodes.insert(id, Slot { node, seq });
    }

    /// The node `id`, if there is one.
    pub(crate) fn get(&self, id: WidgetId) -> Option<&Node> {
        self.nodes.get(&id).map(|slot| &slot.node)
    }

    /// The node `id`, mutably. Its `parent` must not be changed.
    pub(crate) fn get_mut(&mut self, id: WidgetId) -> Option<&mut Node> {
        self.nodes.get_mut(&id).map(|slot| &mut slot.node)
    }

    /// Whether the node `id` exists.
    pub(crate) fn contains(&self, id: WidgetId) -> bool {
        self.nodes.contains_key(&id)
    }

    /// The window owning the node `id`.
    pub(crate) fn window_of(&self, id: WidgetId) -> Option<WindowId> {
        self.get(id).map(|node| node.window)
    }

    /// Every node in paint order, bottom first; `.rev()` is top first.
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = (WidgetId, &Node)> {
        self.order.values().filter_map(|id| {
            let slot = self.nodes.get(id)?;
            Some((*id, &slot.node))
        })
    }

    /// Every node, mutably, in no particular order.
    #[cfg(feature = "winit-backend")]
    pub(crate) fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.nodes.values_mut().map(|slot| &mut slot.node)
    }

    /// The nodes created directly inside `id`, in no particular order.
    pub(crate) fn children(&self, id: WidgetId) -> &[WidgetId] {
        self.children.get(&id).map_or(&[], Vec::as_slice)
    }

    /// `id` and every node created inside it, parents before children.
    pub(crate) fn subtree(&self, id: WidgetId) -> Vec<WidgetId> {
        let mut members = Vec::new();
        if !self.contains(id) {
            return members;
        }
        let mut pending = vec![id];
        while let Some(next) = pending.pop() {
            members.push(next);
            pending.extend_from_slice(self.children(next));
        }
        members
    }

    /// Moves `id` above every other node. Returns whether it moved (it did
    /// not if it was already on top or is not there).
    pub(crate) fn raise(&mut self, id: WidgetId) -> bool {
        let Some(slot) = self.nodes.get_mut(&id) else {
            return false;
        };
        if self.order.last_key_value().map(|(_, top)| *top) == Some(id) {
            return false;
        }
        self.order.remove(&slot.seq);
        slot.seq = self.next_seq;
        self.next_seq += 1;
        self.order.insert(slot.seq, id);
        true
    }

    /// Removes `id` and every node created inside it, and returns their ids
    /// (empty when `id` is not there).
    pub(crate) fn remove(&mut self, id: WidgetId) -> Vec<WidgetId> {
        let removed = self.subtree(id);
        if let Some(first) = removed.first() {
            self.unlink_from_parent(*first);
        }
        for gone in &removed {
            if let Some(slot) = self.nodes.remove(gone) {
                self.order.remove(&slot.seq);
            }
            self.children.remove(gone);
        }
        removed
    }

    /// Removes every node of `window`, and returns their ids.
    pub(crate) fn remove_window(&mut self, window: WindowId) -> Vec<WidgetId> {
        let gone: Vec<WidgetId> = self
            .nodes
            .iter()
            .filter(|(_, slot)| slot.node.window == window)
            .map(|(id, _)| *id)
            .collect();
        for id in &gone {
            if let Some(slot) = self.nodes.remove(id) {
                self.order.remove(&slot.seq);
            }
            self.children.remove(id);
        }
        // A surviving container (another window's) cannot hold one of these,
        // since a node's parent is in its own window.
        gone
    }

    /// Drops `id` from its parent's child list.
    fn unlink_from_parent(&mut self, id: WidgetId) {
        let Some(ParentRef::Widget(parent)) = self.get(id).map(|node| node.parent) else {
            return;
        };
        if let Some(siblings) = self.children.get_mut(&parent) {
            siblings.retain(|sibling| *sibling != id);
            if siblings.is_empty() {
                self.children.remove(&parent);
            }
        }
    }

    /// Forgets a node that is about to be replaced by an insert of its id.
    fn unlink(&mut self, id: WidgetId) {
        self.unlink_from_parent(id);
        if let Some(slot) = self.nodes.remove(&id) {
            self.order.remove(&slot.seq);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> WindowId {
        WindowId::from_raw(1)
    }

    fn node(parent: ParentRef) -> Node {
        Node::new(window(), parent, Rect::new(0, 0, 10, 10))
    }

    fn ids(scene: &Scene) -> Vec<u64> {
        scene.iter().map(|(id, _)| id.raw()).collect()
    }

    #[test]
    fn nodes_paint_in_creation_order_and_a_raise_moves_one_to_the_top() {
        let mut scene = Scene::new();
        for raw in 1..=4 {
            scene.insert(WidgetId::from_raw(raw), node(ParentRef::Window(window())));
        }
        assert_eq!(ids(&scene), [1, 2, 3, 4]);

        assert!(scene.raise(WidgetId::from_raw(2)));
        assert_eq!(ids(&scene), [1, 3, 4, 2]);
        assert!(
            !scene.raise(WidgetId::from_raw(2)),
            "already on top: nothing moves"
        );
        assert!(!scene.raise(WidgetId::from_raw(99)), "no such node");
    }

    #[test]
    fn removing_a_container_removes_everything_created_inside_it() {
        let mut scene = Scene::new();
        let (panel, row, label, other) = (
            WidgetId::from_raw(1),
            WidgetId::from_raw(2),
            WidgetId::from_raw(3),
            WidgetId::from_raw(4),
        );
        scene.insert(panel, node(ParentRef::Window(window())));
        scene.insert(row, node(ParentRef::Widget(panel)));
        scene.insert(label, node(ParentRef::Widget(row)));
        scene.insert(other, node(ParentRef::Window(window())));

        let mut gone = scene.remove(panel);
        gone.sort_by_key(|id| id.raw());
        assert_eq!(gone, [panel, row, label]);
        assert_eq!(ids(&scene), [4]);
        assert!(!scene.contains(label));
        assert!(
            scene.children(panel).is_empty(),
            "no dangling child list for a removed container"
        );
    }

    #[test]
    fn removing_a_child_unlinks_it_from_its_parent() {
        let mut scene = Scene::new();
        let (panel, a, b) = (
            WidgetId::from_raw(1),
            WidgetId::from_raw(2),
            WidgetId::from_raw(3),
        );
        scene.insert(panel, node(ParentRef::Window(window())));
        scene.insert(a, node(ParentRef::Widget(panel)));
        scene.insert(b, node(ParentRef::Widget(panel)));

        scene.remove(a);
        assert_eq!(scene.children(panel), [b]);
        assert_eq!(scene.subtree(panel).len(), 2);
    }

    #[test]
    fn a_raised_parent_keeps_its_children_and_a_subtree_lists_them() {
        let mut scene = Scene::new();
        let (panel, child) = (WidgetId::from_raw(1), WidgetId::from_raw(2));
        scene.insert(panel, node(ParentRef::Window(window())));
        scene.insert(child, node(ParentRef::Widget(panel)));
        scene.raise(panel);
        assert_eq!(ids(&scene), [2, 1]);
        assert_eq!(scene.subtree(panel), [panel, child]);
    }

    #[test]
    fn closing_a_window_removes_only_its_nodes() {
        let mut scene = Scene::new();
        let second = WindowId::from_raw(2);
        scene.insert(WidgetId::from_raw(1), node(ParentRef::Window(window())));
        scene.insert(
            WidgetId::from_raw(2),
            Node::new(second, ParentRef::Window(second), Rect::new(0, 0, 5, 5)),
        );
        let gone = scene.remove_window(window());
        assert_eq!(gone, [WidgetId::from_raw(1)]);
        assert_eq!(scene.len(), 1);
        assert_eq!(ids(&scene), [2]);
    }

    /// The old storage: a `Vec` in paint order, searched linearly, with the
    /// destroy cascade as a fixed point over "parent is gone".
    #[derive(Default)]
    struct Model(Vec<(WidgetId, ParentRef)>);

    impl Model {
        fn destroy(&mut self, id: WidgetId) {
            self.0.retain(|(node, _)| *node != id);
            loop {
                let gone: Vec<WidgetId> = self
                    .0
                    .iter()
                    .filter(|(_, parent)| match parent {
                        ParentRef::Window(_) => false,
                        ParentRef::Widget(parent) => !self.0.iter().any(|(id, _)| id == parent),
                    })
                    .map(|(id, _)| *id)
                    .collect();
                if gone.is_empty() {
                    return;
                }
                self.0.retain(|(id, _)| !gone.contains(id));
            }
        }

        fn raise(&mut self, id: WidgetId) {
            if let Some(at) = self.0.iter().position(|(node, _)| *node == id) {
                let entry = self.0.remove(at);
                self.0.push(entry);
            }
        }
    }

    /// Random creates, raises and destroys keep the scene's paint order and
    /// membership equal to the linear-scan storage it replaces.
    #[test]
    fn the_scene_matches_the_linear_scan_storage_over_random_operations() {
        for seed in 1..=60u64 {
            let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            let mut next = move |bound: u64| {
                state ^= state >> 12;
                state ^= state << 25;
                state ^= state >> 27;
                state.wrapping_mul(0x2545_F491_4F6C_DD1D) % bound
            };
            let mut scene = Scene::new();
            let mut model = Model::default();
            let mut created = 0u64;
            for _ in 0..120 {
                match next(10) {
                    0..=4 => {
                        created += 1;
                        let id = WidgetId::from_raw(created);
                        let alive: Vec<WidgetId> = model.0.iter().map(|(id, _)| *id).collect();
                        let parent = if alive.is_empty() || next(3) == 0 {
                            ParentRef::Window(window())
                        } else {
                            ParentRef::Widget(alive[next(alive.len() as u64) as usize])
                        };
                        scene.insert(id, node(parent));
                        model.0.push((id, parent));
                    }
                    5..=6 if !model.0.is_empty() => {
                        let id = model.0[next(model.0.len() as u64) as usize].0;
                        scene.raise(id);
                        model.raise(id);
                    }
                    _ if !model.0.is_empty() => {
                        let id = model.0[next(model.0.len() as u64) as usize].0;
                        scene.remove(id);
                        model.destroy(id);
                    }
                    _ => {}
                }
                let want: Vec<u64> = model.0.iter().map(|(id, _)| id.raw()).collect();
                assert_eq!(ids(&scene), want, "seed {seed}");
            }
        }
    }
}
