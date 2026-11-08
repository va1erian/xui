use super::*;

const PANE: Rect = Rect::new(10, 20, 110, 220);

#[test]
fn a_whole_visible_node_shows_at_its_bounds() {
    assert_eq!(place(PANE, None, true, &[]), (PANE, true));
    let roomy = Rect::new(0, 0, 500, 500);
    assert_eq!(place(PANE, Some(roomy), true, &[]), (PANE, true));
}

#[test]
fn a_clipped_node_hides_rather_than_show_a_part_the_app_did_not_draw() {
    let half = Rect::new(0, 0, 60, 500);
    assert_eq!(place(PANE, Some(half), true, &[]), (PANE, false));
}

#[test]
fn a_popup_over_the_node_hides_it_and_one_beside_it_does_not() {
    let over = Rect::new(100, 100, 200, 150);
    let beside = Rect::new(110, 20, 200, 60);
    assert!(!place(PANE, None, true, &[over]).1);
    assert!(place(PANE, None, true, &[beside]).1);
}

#[test]
fn a_hidden_or_empty_node_is_not_shown() {
    assert!(!place(PANE, None, false, &[]).1);
    assert!(!place(Rect::new(10, 20, 10, 220), None, true, &[]).1);
}

#[test]
fn a_first_placement_moves_and_shows() {
    assert_eq!(placement_change(None, PANE, true), (Some(PANE), Some(true)));
}

#[test]
fn an_unchanged_placement_changes_nothing() {
    assert_eq!(
        placement_change(Some((PANE, true)), PANE, true),
        (None, None)
    );
}

#[test]
fn a_hidden_node_hides_and_keeps_its_place() {
    assert_eq!(
        placement_change(Some((PANE, true)), Rect::default(), false),
        (None, Some(false))
    );
}

#[test]
fn a_moved_node_moves_without_showing_again() {
    let (a, b) = (Rect::new(0, 0, 50, 50), Rect::new(5, 0, 60, 50));
    assert_eq!(placement_change(Some((a, true)), b, true), (Some(b), None));
}
