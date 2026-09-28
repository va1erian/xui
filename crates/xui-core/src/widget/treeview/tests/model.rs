use super::*;

#[test]
fn a_model_loads_children_only_on_expand() {
    let (runtime, tree, loads) = harness_model(false);
    assert_eq!(loads.get(), 1, "only the roots load when the tree is built");
    assert_eq!(tree.len(), 2);

    send(&runtime, &tree, &down(10, 11));
    assert_eq!(loads.get(), 2, "expanding reads the branch once");
    assert_eq!(tree.len(), 4, "the two children are now materialized");
    assert_eq!(tree.selected(), None, "a chevron click does not select");

    send(&runtime, &tree, &down(10, 11));
    assert!(!is_visible(&tree.state.borrow().rows, 1));
    send(&runtime, &tree, &down(10, 11));
    assert_eq!(loads.get(), 2, "re-expanding reuses the loaded children");
    assert!(is_visible(&tree.state.borrow().rows, 1));
}

#[test]
fn a_collapsed_branch_hides_its_descendants() {
    let (runtime, tree, _) = harness_model(false);
    send(&runtime, &tree, &down(10, 11));
    {
        let state = tree.state.borrow();
        assert!(is_visible(&state.rows, 2), "an expanded chain shows a leaf");
        assert_eq!(slot_to_index(&state.rows, 2), Some(2));
    }

    send(&runtime, &tree, &down(10, 11));
    let state = tree.state.borrow();
    assert!(!is_visible(&state.rows, 1), "collapsing hides the children");
    assert!(!is_visible(&state.rows, 2));
    assert_eq!(
        slot_to_index(&state.rows, 1),
        Some(3),
        "Sent moves up a slot"
    );
    assert_eq!(slot_to_index(&state.rows, 2), None, "no third visible row");
}

#[test]
fn a_model_node_keeps_its_icon() {
    let (_runtime, tree, _) = harness_model(false);
    let state = tree.state.borrow();
    assert_eq!(
        state.rows[0].icon,
        Some(RowIcon::Glyph(Glyph::Folder)),
        "the model's icon reaches the materialized row"
    );
}
