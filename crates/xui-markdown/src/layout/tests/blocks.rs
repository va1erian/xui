#![forbid(unsafe_code)]

//! Block furniture: list markers, quote bars, rules and code blocks.

use super::*;

#[test]
fn bullets_are_discs_at_odd_depths_and_squares_at_even_ones() {
    let mut layout = lay("- a\n  - b", 1000.0);
    layout.ensure(0.0, 1.0e6);
    let outer = layout.block(0).expect("laid out");
    let inner = layout.block(1).expect("laid out");
    assert!(matches!(outer.decor[..], [Decor::Disc(_, Role::Text)]));
    assert!(matches!(inner.decor[..], [Decor::Fill(_, Role::Text)]));
    let Decor::Disc(disc, _) = outer.decor[0] else {
        unreachable!()
    };
    assert!(disc.x + disc.w < 24.0, "in the indent, left of the text");
    assert_eq!(outer.pieces[0].x, 24.0);
    assert_eq!(inner.pieces[0].x, 48.0);
}

#[test]
fn a_number_is_a_piece_left_of_the_text() {
    let block = one("1. one\n2. two", 1, 1000.0);
    let number = piece(&block, "2.");
    let body = piece(&block, "two");
    assert_eq!(number.role, Role::Text);
    assert!(number.x + number.layout.width() < body.x);
    assert!(number.x >= 0.0);
    assert!(close(number.y, 13.5 - number.layout.baseline()));
}

#[test]
fn a_task_box_is_a_frame_filled_when_checked() {
    let mut layout = lay("- [ ] todo\n- [x] done", 1000.0);
    layout.ensure(0.0, 1.0e6);
    let open = layout.block(0).expect("laid out");
    let done = layout.block(1).expect("laid out");
    assert_eq!(frames(open).len(), 1);
    assert!(fills(open, Role::Link).is_empty());
    assert_eq!(frames(done).len(), 1);
    let ticks = fills(done, Role::Link);
    assert_eq!(ticks.len(), 1);
    let frame = frames(done)[0];
    assert!(ticks[0].x > frame.x && ticks[0].x + ticks[0].w < frame.x + frame.w);
    assert!(ticks[0].y > frame.y && ticks[0].y + ticks[0].h < frame.y + frame.h);
}

#[test]
fn each_quote_level_has_a_bar() {
    let mut layout = lay("> a\n>\n> > b", 1000.0);
    layout.ensure(0.0, 1.0e6);
    let bars = |i: usize| fills(layout.block(i).expect("laid out"), Role::Border);
    assert_eq!(bars(0).len(), 1);
    assert_eq!(bars(1).len(), 2);
    assert_eq!(bars(0)[0].w, 3.0);
    assert_eq!(bars(1)[1].x, 16.0);
    assert_eq!(bars(0)[0].h, 18.0);
    assert_eq!(layout.block(0).unwrap().pieces[0].x, 16.0);
    assert_eq!(layout.block(1).unwrap().pieces[0].x, 32.0);
    assert_eq!(layout.block(0).unwrap().pieces[0].role, Role::Weak);
}

#[test]
fn a_quote_bar_runs_up_through_the_gap_only_where_the_levels_are_shared() {
    let mut layout = lay("> a\n>\n> > b", 1000.0);
    layout.ensure(0.0, 1.0e6);
    let bars = fills(layout.block(1).expect("laid out"), Role::Border);
    assert_eq!(bars[0].y, -10.0, "level 1 joins the block above");
    assert_eq!(bars[0].h, 18.0 + 10.0);
    assert_eq!(bars[1].y, 0.0, "level 2 starts here");

    let after = one("plain\n\n> q", 1, 1000.0);
    assert_eq!(fills(&after, Role::Border)[0].y, 0.0);
}

#[test]
fn first_and_second_level_headings_have_a_rule_and_the_rest_do_not() {
    let block = one("# One\n\n### Three", 0, 1000.0);
    let rule = fills(&block, Role::Border);
    assert_eq!(rule.len(), 1);
    // 25.2 px type: a 32 px line, 4 px of padding, a 1 px rule.
    assert_eq!(rule[0].y, 32.0 + 4.0);
    assert_eq!(rule[0].h, 1.0);
    assert_eq!(rule[0].w, 1000.0);
    assert_eq!(block.height, 37.0);
    assert_eq!(fills(&one("## Two", 0, 1000.0), Role::Border).len(), 1);
    assert!(fills(&one("# One\n\n### Three", 1, 1000.0), Role::Border).is_empty());
    assert_eq!(piece(&block, "One").role, Role::Text);
}

#[test]
fn a_thematic_break_is_a_two_pixel_rule() {
    let block = one("a\n\n---\n\nb", 1, 300.0);
    assert_eq!(block.height, 2.0);
    assert_eq!(
        fills(&block, Role::Border),
        [FRect::new(0.0, 0.0, 300.0, 2.0)]
    );
    assert!(block.pieces.is_empty());
}

#[test]
fn without_wrapping_a_code_block_is_one_piece_per_non_empty_line_on_a_shaded_box() {
    let mut layout = lay(
        "```\nfn a() { long_line_that_is_wider_than_the_box }\n\n  b\n```",
        100.0,
    );
    layout.set_style(MarkdownStyle {
        wrap_code: false,
        ..MarkdownStyle::default()
    });
    layout.resize(100.0, 96);
    layout.ensure(0.0, 1.0e6);
    let block = layout.block(0).expect("laid out").clone();
    assert_eq!(block.pieces.len(), 2, "the empty line has no piece");
    assert!(block.pieces[0].layout.width() > 100.0, "not wrapped");
    assert_eq!(block.pieces[0].x, 8.0);
    assert_eq!(block.pieces[0].y, 8.0);
    assert_eq!(block.pieces[1].y, 8.0 + 2.0 * 16.0);
    assert_eq!(block.height, 8.0 + 3.0 * 16.0 + 8.0);
    assert_eq!(
        block.decor[0],
        Decor::Fill(FRect::new(0.0, 0.0, 100.0, 64.0), Role::Surface)
    );
    assert_eq!(block.clip, Some(FRect::new(8.0, 0.0, 84.0, 64.0)));
}

#[test]
fn tabs_in_code_expand_to_four_spaces() {
    let block = one("```\n\tb\n```", 0, 300.0);
    assert_eq!(texts(&block), ["    b"]);
    assert!(close(block.pieces[0].layout.width(), 5.0 * 6.5));
}

#[test]
fn a_quoted_code_block_is_weak_and_indented_past_the_bar() {
    let block = one("> ```\n> x\n> ```", 0, 300.0);
    assert_eq!(block.pieces[0].role, Role::Weak);
    assert_eq!(block.pieces[0].x, 16.0 + 8.0);
}

#[test]
fn a_code_block_of_only_blank_lines_does_not_panic() {
    let mut layout = lay("```\n\n\n```\n\nafter", 100.0);
    layout.ensure(0.0, 1.0e6);
    assert!(layout.block(0).is_some());
    assert!(layout.height() > 0.0);
}

#[test]
fn everything_scales_with_the_dpi() {
    let mut layout = Layout::new(
        Arc::new(Document::parse("# One")),
        MarkdownStyle::default(),
        Box::new(Mono),
        192,
    );
    layout.resize(1000.0, 192);
    layout.ensure(0.0, 1.0e6);
    let block = layout.block(0).expect("laid out");
    // 50.4 px type: a 63 px line, 8 px of padding, a 2 px rule.
    assert_eq!(block.height, 63.0 + 8.0 + 2.0);
}
