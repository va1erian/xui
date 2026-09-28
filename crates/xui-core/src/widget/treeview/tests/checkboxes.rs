use super::*;

#[test]
fn checkboxes_toggle_independently() {
    let (runtime, tree, _) = harness_model(false);
    send(&runtime, &tree, &down(10, 11));
    send(&runtime, &tree, &down(44, 33));
    assert_eq!(tree.checked(11), Some(CheckState::Checked));
    assert_eq!(tree.checked(12), Some(CheckState::Unchecked));
    assert_eq!(tree.selected(), None, "a checkbox click does not select");

    send(&runtime, &tree, &down(44, 55));
    assert_eq!(tree.checked(12), Some(CheckState::Checked));
    assert_eq!(
        tree.checked(11),
        Some(CheckState::Checked),
        "rows are independent"
    );
    assert_eq!(
        log(),
        vec![203, 300 + 11 * 2 + 1, 300 + 12 * 2 + 1],
        "the expand and both check messages"
    );
}

#[test]
fn a_tri_state_checkbox_visits_the_mixed_state() {
    let (runtime, tree, _) = harness_model(true);
    send(&runtime, &tree, &down(10, 11));
    send(&runtime, &tree, &down(44, 33));
    assert_eq!(tree.checked(11), Some(CheckState::Checked));
    send(&runtime, &tree, &down(44, 33));
    assert_eq!(tree.checked(11), Some(CheckState::Indeterminate));
    send(&runtime, &tree, &down(44, 33));
    assert_eq!(tree.checked(11), Some(CheckState::Unchecked));
}

#[test]
fn space_toggles_the_selected_rows_checkbox() {
    let (runtime, tree, _) = harness_model(false);
    tree.select(Some(2));
    send(&runtime, &tree, &key(Key::SPACE));
    assert_eq!(tree.checked(2), Some(CheckState::Checked));
    assert_eq!(
        tree.selected(),
        Some(2),
        "Space does not move the selection"
    );
}
