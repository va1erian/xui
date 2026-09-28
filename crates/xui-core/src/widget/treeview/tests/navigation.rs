use super::*;

#[test]
fn keyboard_navigation_skips_hidden_rows() {
    let (runtime, tree, _) = harness_model(false);
    tree.select(Some(2));
    send(&runtime, &tree, &key(Key::UP));
    assert_eq!(tree.selected(), Some(1), "the hidden children are skipped");
    send(&runtime, &tree, &key(Key::DOWN));
    assert_eq!(tree.selected(), Some(2));

    tree.select(Some(1));
    send(&runtime, &tree, &key(Key::RIGHT));
    send(&runtime, &tree, &key(Key::DOWN));
    assert_eq!(tree.selected(), Some(11));
    send(&runtime, &tree, &key(Key::DOWN));
    assert_eq!(tree.selected(), Some(12));
    send(&runtime, &tree, &key(Key::DOWN));
    assert_eq!(tree.selected(), Some(2));
    send(&runtime, &tree, &key(Key::UP));
    assert_eq!(tree.selected(), Some(12));
}

#[test]
fn home_and_end_choose_visible_rows() {
    let (runtime, tree, _) = harness_model(false);
    tree.select(Some(2));
    send(&runtime, &tree, &key(Key::HOME));
    assert_eq!(tree.selected(), Some(1));
    send(&runtime, &tree, &key(Key::END));
    assert_eq!(tree.selected(), Some(2));
}
