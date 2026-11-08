#![forbid(unsafe_code)]

//! Laying out the visible blocks first, then the rest in slices.

use std::time::Duration;

use super::*;

const BLOCKS: usize = 3000;

fn long() -> String {
    (0..BLOCKS)
        .map(|i| format!("Paragraph {i} with some **words** to wrap onto lines.\n\n"))
        .collect()
}

fn laid_out(layout: &Layout) -> usize {
    (0..BLOCKS).filter(|&i| layout.block(i).is_some()).count()
}

/// Works until nothing is left, however many slices that takes.
fn finish(layout: &mut Layout) {
    let mut slices = 0;
    while layout.work(Duration::from_millis(2)) {
        slices += 1;
        assert!(slices < 100_000, "work() never finishes");
    }
}

#[test]
fn ensure_lays_out_only_the_first_screen_and_estimates_the_rest() {
    let mut layout = lay(&long(), 300.0);
    let range = layout.ensure(0.0, 600.0);
    assert!(
        range.start == 0 && range.end >= 10 && range.end < 60,
        "{range:?}"
    );
    assert!(laid_out(&layout) < 60, "laid out {}", laid_out(&layout));
    assert!(layout.block(BLOCKS - 1).is_none());
    assert!(!layout.done());
    let mut last = -1.0;
    for i in 0..BLOCKS {
        let top = layout.top(i);
        assert!(top > last, "block {i} at {top}");
        last = top;
    }
    assert!(layout.height() > last);
}

#[test]
fn work_lays_out_the_rest_and_the_height_becomes_the_real_sum() {
    let mut layout = lay(&long(), 300.0);
    layout.ensure(0.0, 600.0);
    let estimated = layout.height();
    finish(&mut layout);
    assert!(layout.done());
    assert_eq!(laid_out(&layout), BLOCKS);
    let real = layout.height();
    assert_ne!(real, estimated);
    let end = layout.top(BLOCKS - 1) + layout.block(BLOCKS - 1).unwrap().height;
    assert!(close(real, end), "{real} vs {end}");
    // 50 characters at 7 px do not fit 300 px: two lines of 18 px.
    assert_eq!(layout.block(5).unwrap().height, 36.0);
}

#[test]
fn working_with_nothing_to_do_reports_done() {
    let mut layout = lay("", 300.0);
    assert!(layout.done());
    assert!(!layout.work(Duration::from_millis(1)));
}

#[test]
fn a_new_width_clears_the_layouts() {
    let mut layout = lay(&long(), 300.0);
    layout.ensure(0.0, 600.0);
    assert!(layout.block(0).is_some());
    layout.resize(300.0, 96);
    assert!(layout.block(0).is_some(), "the same size keeps them");
    layout.resize(200.0, 96);
    assert!(layout.block(0).is_none());
    assert!(!layout.done());
    layout.ensure(0.0, 600.0);
    assert!(layout.block(0).is_some());
}

#[test]
fn a_new_dpi_starts_over() {
    let mut layout = lay("hello", 300.0);
    layout.ensure(0.0, 100.0);
    let at_96 = layout.height();
    layout.resize(300.0, 192);
    assert!(layout.block(0).is_none());
    layout.ensure(0.0, 100.0);
    assert!(layout.height() > at_96);
}

#[test]
fn visible_returns_the_blocks_overlapping_a_window() {
    let mut layout = lay(&long(), 300.0);
    finish(&mut layout);
    assert_eq!(layout.visible(0.0, 1.0), 0..1);
    let (y0, y1) = (layout.top(10) + 1.0, layout.top(12) + 1.0);
    assert_eq!(layout.visible(y0, y1), 10..13);
    assert_eq!(layout.block_at(y0), 10);
    let end = layout.height();
    assert_eq!(layout.visible(end - 1.0, end + 100.0), BLOCKS - 1..BLOCKS);
}

#[test]
fn ensure_settles_estimates_above_the_window() {
    let mut layout = lay(&long(), 300.0);
    let y = layout.top(1500);
    let range = layout.ensure(y, y + 400.0);
    assert!(range.contains(&1500) || range.start <= 1500);
    for i in range.clone() {
        assert!(layout.block(i).is_some(), "block {i}");
    }
    assert!(laid_out(&layout) < 60);
}

#[test]
fn gaps_separate_blocks_and_headings_get_more() {
    let mut layout = lay("para\n\n# Head\n\nafter", 300.0);
    finish(&mut layout);
    let mut gap = |i: usize| {
        let above = layout.top(i - 1) + layout.block(i - 1).unwrap().height;
        layout.top(i) - above
    };
    assert!(close(gap(1), 16.0), "before a heading");
    assert!(close(gap(2), 10.0), "between paragraphs");
    assert_eq!(layout.top(0), 0.0);
}

#[test]
fn list_items_sit_closer_than_paragraphs() {
    let mut layout = lay("- a\n- b\n\nafter", 300.0);
    finish(&mut layout);
    assert!(close(
        layout.top(1) - layout.top(0) - layout.block(0).unwrap().height,
        4.0
    ));
}

#[test]
fn laying_out_a_long_document_in_slices_does_not_panic() {
    let mut text = long();
    text.push_str("| a | b |\n|---|---|\n| 1 | 2 |\n\n```\ncode\n```\n\n> quote\n");
    let mut layout = lay(&text, 123.0);
    finish(&mut layout);
    assert!(layout.done());
    assert!(layout.height() > 0.0);
    layout.resize(77.0, 96);
    finish(&mut layout);
    assert!(layout.done());
}

#[test]
fn the_first_size_replaces_estimates_made_with_no_width() {
    // A view's layout starts with no width; its estimates must follow the
    // width it is first given, or the scrollbar spans a document many times
    // too long until every block is laid out.
    let mut layout = Layout::new(
        Arc::new(Document::parse(&long())),
        MarkdownStyle::default(),
        Box::new(Mono),
        96,
    );
    layout.resize(300.0, 96);
    let estimated = layout.height();
    finish(&mut layout);
    let real = layout.height();
    assert!(
        estimated < real * 2.0 && estimated > real / 2.0,
        "estimated {estimated} against {real}"
    );
}
