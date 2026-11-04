//! The offscreen backend has no native child windows: a node that asks for one
//! gets none and keeps drawing through its painter.

use super::*;

#[test]
fn a_node_that_asks_for_a_native_surface_gets_none_offscreen() {
    let backend = OffscreenBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("t"))
        .expect("window");
    let spec = NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 40, 30));
    let node = backend
        .create(ParentRef::Window(window), &spec)
        .expect("node");
    backend.set_native_surface(node, true);
    assert!(backend.native_surface(node).is_none());
    backend.set_native_surface(node, false);
    assert!(backend.native_surface(node).is_none());
}
