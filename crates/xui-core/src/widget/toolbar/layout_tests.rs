#![forbid(unsafe_code)]

//! Tests of the toolbar's layout function, with a fixed text measurer so the
//! numbers are exact.

use super::layout::{Layout, Mode, compute, icon_side};
use super::strip::{Entries, Entry, Item};
use crate::icon::{IconRef, Lucide};

/// Seven device pixels per character at 96 DPI, scaled with the DPI.
fn measurer(dpi: u32) -> impl FnMut(&str) -> i32 {
    move |text| (text.chars().count() as i32 * 7 * dpi as i32) / 96
}

/// Builds entries from a picture: `i` icon-only, `l` icon and the label "Run",
/// `t` text-only "Open", `|` separator.
fn entries(picture: &str) -> Entries {
    let mut entries = Entries::default();
    for c in picture.chars() {
        let icon = Some(IconRef::Lucide(Lucide::Save));
        entries.push(match c {
            'i' => Entry::Item(Item {
                icon,
                label: None,
                tooltip: None,
            }),
            'l' => Entry::Item(Item {
                icon,
                label: Some("Run".into()),
                tooltip: None,
            }),
            't' => Entry::Item(Item::text("Open")),
            '|' => Entry::Separator,
            other => panic!("unknown entry {other}"),
        });
    }
    entries
}

fn lay(picture: &str, mode: Mode, size: (i32, i32), dpi: u32) -> Layout {
    compute(&entries(picture), mode, size, dpi, &mut measurer(dpi))
}

fn compact(picture: &str, width: i32) -> Layout {
    lay(picture, Mode::Compact, (width, 40), 96)
}

#[test]
fn icon_only_buttons_are_square_and_packed_from_the_left() {
    let strip = compact("iii", 400);
    assert_eq!(strip.items, vec![(0, 40), (40, 80), (80, 120)]);
    assert_eq!(strip.item_at(0), Some(0), "the first starts at the edge");
    assert_eq!(strip.item_at(120), None, "the tail is empty");
}

#[test]
fn a_label_widens_the_button_by_its_measured_content() {
    // icon-only 40; text-only: 8 + 28 + 8; icon + label: 8 + 16 + 6 + 21 + 8.
    let strip = compact("itl", 400);
    assert_eq!(strip.items, vec![(0, 40), (40, 84), (84, 143)]);
    let widths: Vec<i32> = strip.items.iter().map(|(a, b)| b - a).collect();
    assert!(widths[2] > widths[0] && widths[1] > widths[0]);
}

#[test]
fn a_separator_takes_room_but_no_index() {
    let strip = compact("ii|i", 400);
    assert_eq!(strip.items.len(), 3);
    // 4 margin + 1 line + 4 margin after the second button (ends at 80).
    assert_eq!(strip.separators, vec![84]);
    assert_eq!(strip.items[2], (89, 129));
    for x in 80..89 {
        assert_eq!(strip.item_at(x), None, "the separator gap at {x}");
    }
}

#[test]
fn separators_first_last_and_doubled_keep_the_item_indices() {
    for picture in ["|ii", "ii|", "i||i", "|i||i|"] {
        let strip = compact(picture, 400);
        let items = picture.chars().filter(|&c| c == 'i').count();
        assert_eq!(strip.items.len(), items, "{picture}");
        assert!(strip.items.iter().all(|&(a, b)| b > a), "{picture}");
        for (index, &(start, end)) in strip.items.iter().enumerate() {
            assert_eq!(strip.item_at(start), Some(index), "{picture}");
            assert_eq!(strip.item_at(end - 1), Some(index), "{picture}");
        }
    }
    assert_eq!(compact("|ii", 400).items[0].0, 9, "a leading separator");
    assert_eq!(compact("i||i", 400).separators, vec![44, 53]);
}

#[test]
fn every_pixel_of_a_span_hits_its_item_and_nothing_else_does() {
    let strip = compact("ii|tl||i", 300);
    for x in -2..310 {
        let expected = strip
            .items
            .iter()
            .position(|&(start, end)| start <= x && x < end);
        assert_eq!(strip.item_at(x), expected, "x = {x}");
    }
    for &line in &strip.separators {
        assert_eq!(strip.item_at(line), None, "separator at {line}");
    }
}

#[test]
fn sizes_scale_with_the_dpi() {
    // Items only: every constant is a whole number of pixels at both DPIs.
    let at_96 = lay("itl", Mode::Compact, (600, 40), 96);
    let at_144 = lay("itl", Mode::Compact, (900, 60), 144);
    let scaled: Vec<_> = at_96
        .items
        .iter()
        .map(|&(a, b)| (a * 3 / 2, b * 3 / 2))
        .collect();
    assert_eq!(at_144.items, scaled);
    assert_eq!(icon_side(60, 144), 24);
    // A separator: 6 + 2 + 6 at 144 DPI (1 dip rounds to 2 px).
    let sep = lay("i|i", Mode::Compact, (900, 60), 144);
    assert_eq!(sep.separators, vec![60 + 6]);
    assert_eq!(sep.items[1].0, 60 + 6 + 2 + 6);
}

#[test]
fn a_trailing_item_that_does_not_fit_is_clipped() {
    let strip = compact("iii", 100);
    assert_eq!(strip.items, vec![(0, 40), (40, 80), (0, 0)]);
    for x in 80..100 {
        assert_eq!(strip.item_at(x), None, "the clipped item at {x}");
    }
    // Clipping stops at the first miss: a later, smaller item does not jump it.
    let strip = compact("ilii", 90);
    assert_eq!(strip.items[0], (0, 40));
    assert!(strip.items[1..].iter().all(|&s| s == (0, 0)));
    // A separator that does not fit is dropped too.
    assert!(compact("i|", 44).separators.is_empty());
}

#[test]
fn degenerate_strips_lay_out_nothing_and_do_not_panic() {
    assert!(compact("", 100).items.is_empty());
    assert_eq!(compact("i", 0).items, vec![(0, 0)]);
    assert_eq!(compact("i", -5).item_at(0), None);
    assert_eq!(lay("i", Mode::Compact, (100, 0), 96).items, vec![(0, 0)]);
    let only = compact("|||", 100);
    assert!(only.items.is_empty());
    assert_eq!(only.separators, vec![4, 13, 22]);
    let long = compact("ti", 20);
    assert_eq!(
        long.items,
        vec![(0, 0), (0, 0)],
        "one label wider than the strip"
    );
    assert_eq!(long.item_at(5), None);
    assert!(compact("|", 5).separators.is_empty());
}

#[test]
fn fill_splits_the_width_equally_and_places_separators() {
    let strip = lay("ii|i", Mode::Fill, (90, 40), 96);
    assert_eq!(strip.items, vec![(0, 30), (30, 60), (60, 90)]);
    assert_eq!(strip.separators, vec![60], "on the boundary before item 2");
    assert_eq!(lay("|i", Mode::Fill, (90, 40), 96).separators, vec![0]);
    assert!(lay("i|", Mode::Fill, (90, 40), 96).separators.is_empty());
    assert_eq!(lay("|", Mode::Fill, (90, 40), 96).separators, vec![0]);
}
