use super::*;

#[test]
fn indent_and_chevron_geometry_is_consistent() {
    assert_eq!(level_x(96, 0, 0), 4, "a root starts after the padding");
    assert_eq!(level_x(96, 0, 1), 20, "a level adds one indent step");
    assert_eq!(level_x(96, 0, 2), 36);

    assert!(chevron_hit(96, 0, 0, true, 10), "inside the chevron slot");
    assert!(!chevron_hit(96, 0, 0, true, 2), "left of the slot");
    assert!(!chevron_hit(96, 0, 0, true, 20), "right of the slot");
    assert!(
        !chevron_hit(96, 0, 0, false, 10),
        "a leaf has no chevron to hit"
    );

    assert_eq!(
        label_x(96, 0, 1, false, false),
        36,
        "the label clears the chevron"
    );
    assert_eq!(
        label_x(96, 0, 1, true, false),
        36 + 16 + 6,
        "a checkbox and its gap push the label right"
    );
    assert_eq!(
        label_x(96, 0, 1, false, true),
        36 + 16 + 6,
        "an icon and its gap push the label right"
    );
    assert_eq!(
        label_x(96, 0, 1, true, true),
        36 + 16 + 6 + 16 + 6,
        "a checkbox then an icon stack before the label"
    );

    let square = checkbox_rect(96, 0, 22, 1);
    assert_eq!((square.left, square.top), (36, 25));
    assert!(square.contains(Point::new(44, 33)), "inside the checkbox");

    let icon = icon_rect(96, 0, 22, 1, false);
    assert_eq!(
        (icon.left, icon.top),
        (36, 25),
        "without a checkbox the icon starts past the chevron"
    );
    assert_eq!(icon_x(96, 0, 1, false), 36);
    let checked_icon = icon_rect(96, 0, 22, 1, true);
    assert_eq!(
        checked_icon.left,
        36 + 16 + 6,
        "a checkbox pushes the icon right"
    );

    assert_eq!(guide_x(96, 0, 1), 20 + 8, "a guide is under its chevron");
}

#[test]
fn an_indent_guide_stops_after_the_last_child() {
    let rows = [
        TreeRow::new("Inbox", 0).expandable(true).expanded(true),
        TreeRow::new("Work", 1),
        TreeRow::new("Home", 1),
        TreeRow::new("Archive", 0).expandable(true).expanded(true),
        TreeRow::new("Old", 1),
    ];
    let state = super::super::flatten::State::flat(&rows);
    assert!(
        guide_continues(&state.rows, 1, 0),
        "Inbox has a sibling after"
    );
    assert!(guide_continues(&state.rows, 2, 0), "so does Archive");
    assert!(
        !guide_continues(&state.rows, 4, 0),
        "Archive is the last root, so Old ends the guide"
    );
    assert!(
        !guide_continues(&state.rows, 2, 1),
        "Home is the last child at its level"
    );
}
