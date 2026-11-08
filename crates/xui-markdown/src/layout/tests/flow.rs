#![forbid(unsafe_code)]

//! Inline flow: the fast path, word packing, links and decorations.

use super::*;

#[test]
fn a_single_style_paragraph_is_one_piece_wrapped_by_the_shaper() {
    let block = one("alpha beta gamma delta", 0, 80.0);
    assert_eq!(texts(&block), ["alpha beta gamma delta"]);
    assert_eq!(block.height, 36.0, "two lines of 18 px");
    assert_eq!(block.pieces[0].x, 0.0);
}

#[test]
fn mixed_styles_produce_word_pieces_at_measured_positions() {
    let block = one("alpha **beta** gamma", 0, 1000.0);
    assert_eq!(texts(&block), ["alpha", "beta", "gamma"]);
    let x: Vec<f32> = block.pieces.iter().map(|p| p.x).collect();
    // A word's width plus one space (7 px) separates neighbours.
    assert_eq!(x, [0.0, 5.0 * 7.0 + 7.0, 5.0 * 7.0 + 7.0 + 4.0 * 7.0 + 7.0]);
    assert_eq!(block.height, 18.0);
}

#[test]
fn words_wrap_greedily_at_the_width() {
    let block = one("aaa **bbb** ccc ddd", 0, 60.0);
    assert_eq!(texts(&block), ["aaa", "bbb", "ccc", "ddd"]);
    let y: Vec<f32> = block.pieces.iter().map(|p| p.y).collect();
    assert_eq!(y, [0.0, 0.0, 18.0, 18.0]);
    assert_eq!(block.pieces[2].x, 0.0);
    assert_eq!(block.pieces[3].x, 28.0);
    assert_eq!(block.height, 36.0);
}

#[test]
fn a_word_wider_than_the_line_is_cut_and_no_line_overflows() {
    let width = 35.0;
    let block = one("supercalifragilistic **x**", 0, width);
    assert_eq!(texts(&block), ["super", "calif", "ragil", "istic", "x"]);
    for p in &block.pieces {
        assert!(p.x + p.layout.width() <= width + 0.5, "{:?}", text(p));
    }
    assert_eq!(block.height, 5.0 * 18.0);
}

#[test]
fn a_hard_break_starts_a_new_line() {
    let block = one("aa\\\nbb **c**", 0, 1000.0);
    assert_eq!(texts(&block), ["aa", "bb", "c"]);
    assert_eq!(block.pieces[0].y, 0.0);
    assert_eq!(block.pieces[1].y, 18.0);
    assert_eq!(block.pieces[2].y, 18.0);
    assert_eq!(block.pieces[1].x, 0.0);
    assert_eq!(block.height, 36.0);
}

#[test]
fn a_hard_break_in_a_single_style_paragraph_keeps_both_lines() {
    let block = one("aa\\\nbb", 0, 1000.0);
    assert_eq!(texts(&block), ["aa", "bb"]);
    assert_eq!(block.height, 36.0);
}

#[test]
fn a_link_is_found_at_its_piece_and_nowhere_beside_it() {
    let mut layout = lay("see [link](u) now", 1000.0);
    layout.ensure(0.0, 100.0);
    // "see " is 28 px, "link" 28 px.
    assert_eq!(layout.link_at(30.0, 5.0), Some(0));
    assert_eq!(layout.link_at(55.0, 5.0), Some(0));
    assert_eq!(layout.link_at(10.0, 5.0), None);
    assert_eq!(layout.link_at(60.0, 5.0), None);
    assert_eq!(layout.link_at(70.0, 5.0), None);
    assert_eq!(layout.link_at(30.0, 25.0), None, "below the line");
    assert_eq!(layout.doc().link(0), "u");
}

#[test]
fn a_whole_paragraph_link_on_the_fast_path_is_clickable_too() {
    let mut layout = lay("[all of it](u)", 1000.0);
    layout.ensure(0.0, 100.0);
    assert_eq!(layout.link_at(3.0, 3.0), Some(0));
    assert_eq!(layout.link_at(10.0 * 7.0 + 1.0, 3.0), None);
}

#[test]
fn links_are_underlined_and_painted_in_the_link_role() {
    let block = one("see [link](u) now", 0, 1000.0);
    let under = fills(&block, Role::Link);
    assert_eq!(under.len(), 1);
    assert_eq!(under[0].h, 1.0);
    assert!(
        close(under[0].y, 13.5 + 1.0),
        "one pixel under the baseline"
    );
    assert!(close(under[0].x, 28.0) && close(under[0].w, 28.0));
    assert_eq!(piece(&block, "link").role, Role::Link);
    assert_eq!(piece(&block, "see").role, Role::Text);
}

#[test]
fn a_link_spanning_words_is_underlined_across_the_space() {
    let block = one("x [two words](u)", 0, 1000.0);
    let under = fills(&block, Role::Link);
    assert_eq!(under.len(), 2);
    assert!(
        close(under[0].w, 3.0 * 7.0 + 7.0),
        "joined to the next word"
    );
    assert!(close(under[1].w, 5.0 * 7.0));
}

#[test]
fn struck_text_gets_a_line_through_it() {
    let block = one("a ~~b~~ c", 0, 1000.0);
    let strike = fills(&block, Role::Text);
    assert_eq!(strike.len(), 1);
    assert_eq!(strike[0].h, 1.0);
    assert!(close(strike[0].y, 13.5 * 0.68));
    assert!(close(strike[0].x, 14.0) && close(strike[0].w, 7.0));
}

#[test]
fn inline_code_is_shaded_and_set_in_the_code_font() {
    let block = one("a `bc` d", 0, 1000.0);
    let shade = fills(&block, Role::Surface);
    assert_eq!(shade.len(), 1);
    // Two code characters (6.5 px each) padded by 2 px either side.
    assert!(close(shade[0].w, 13.0 + 4.0), "{shade:?}");
    assert!(close(shade[0].x, 14.0 - 2.0));
    assert!(close(piece(&block, "bc").layout.width(), 13.0));
}

#[test]
fn an_empty_document_has_no_height() {
    let mut layout = lay("", 100.0);
    layout.ensure(0.0, 100.0);
    assert_eq!(layout.height(), 0.0);
}

#[test]
fn the_alt_text_of_an_image_is_weak() {
    let block = one("![a picture](p.png) is here", 0, 1000.0);
    assert!(block.pieces.iter().any(|p| p.role == Role::Weak));
}
