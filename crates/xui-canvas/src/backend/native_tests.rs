use super::*;

fn at(bounds: Rect, clip: Option<Rect>) -> Placement {
    Placement {
        bounds,
        visible: true,
        clip,
        below: clip,
    }
}

const PANE: Rect = Rect::new(10, 20, 110, 220);

#[test]
fn a_whole_visible_node_shows_at_its_bounds() {
    assert_eq!(native_place(&at(PANE, None), &[]), (PANE, true));
    let roomy = Rect::new(0, 0, 500, 500);
    assert_eq!(native_place(&at(PANE, Some(roomy)), &[]), (PANE, true));
}

#[test]
fn a_clipped_node_hides_rather_than_show_a_part_the_app_did_not_draw() {
    let half = Rect::new(0, 0, 60, 500);
    assert_eq!(native_place(&at(PANE, Some(half)), &[]), (PANE, false));
}

#[test]
fn a_popup_over_the_node_hides_it_and_one_beside_it_does_not() {
    let over = Rect::new(100, 100, 200, 150);
    let beside = Rect::new(110, 20, 200, 60);
    assert_eq!(native_place(&at(PANE, None), &[over]), (PANE, false));
    assert_eq!(native_place(&at(PANE, None), &[beside]), (PANE, true));
}

#[test]
fn a_hidden_or_empty_node_is_not_shown() {
    let mut hidden = at(PANE, None);
    hidden.visible = false;
    assert!(!native_place(&hidden, &[]).1);
    let empty = Rect::new(10, 20, 10, 220);
    assert!(!native_place(&at(empty, None), &[]).1);
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
    let gone = Rect::default();
    assert_eq!(
        placement_change(Some((PANE, true)), gone, false),
        (None, Some(false))
    );
}

#[test]
fn a_moved_node_moves_without_showing_again() {
    let (a, b) = (Rect::new(0, 0, 50, 50), Rect::new(5, 0, 60, 50));
    assert_eq!(placement_change(Some((a, true)), b, true), (Some(b), None));
}

#[test]
fn a_point_in_the_child_lands_at_its_place_in_the_parent() {
    assert_eq!(to_parent((3.5, 4.0), (10, 20)), (13.5, 24.0));
    assert_eq!(to_parent((0.0, 0.0), (-5, 7)), (-5.0, 7.0));
}
