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

#[test]
fn refresh_rows_keeps_the_scroll_position_and_set_rows_resets_it() {
    let (_runtime, tree) = harness_flat();
    let rows = |count: usize, mark: &str| -> Vec<TreeRow> {
        (0..count)
            .map(|i| TreeRow::new(format!("row {i}{mark}"), 0))
            .collect()
    };
    tree.set_rows(&rows(40, ""));
    tree.state.borrow_mut().offset = 20;

    // A relabelled copy of the same rows: the view stays where it was.
    tree.refresh_rows(&rows(40, " *"));
    assert_eq!(tree.state.borrow().offset, 20);
    assert_eq!(tree.state.borrow().rows[20].label, "row 20 *");

    // Fewer rows: the offset is clamped so the body is never blank.
    tree.refresh_rows(&rows(22, ""));
    let offset = tree.state.borrow().offset;
    assert!(offset > 0 && offset < 22, "clamped, not reset: {offset}");

    tree.set_rows(&rows(40, ""));
    assert_eq!(tree.state.borrow().offset, 0);
}
