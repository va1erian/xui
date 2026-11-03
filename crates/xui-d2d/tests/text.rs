#![cfg(windows)]

//! DirectWrite shaping behind the portable `TextShaper`: no window needed.

use xui_core::Dip;
use xui_core::backend::{Backend, FontSpec};
use xui_d2d::D2dBackend;

fn layout(text: &str, max_width: f32) -> Box<dyn xui_core::backend::TextLayout> {
    D2dBackend::new().layout_text(text, &FontSpec::new(Dip(14.0)), max_width, 96)
}

#[test]
fn a_longer_text_is_wider_and_a_wrapped_one_is_taller() {
    let short = layout("ab", f32::INFINITY);
    let long = layout("abcdefgh", f32::INFINITY);
    assert!(long.width() > short.width());
    let wrapped = layout("alpha beta gamma delta", 60.0);
    assert!(wrapped.height() > short.height());
}

#[test]
fn hit_testing_maps_a_point_to_a_utf8_byte_offset() {
    // "é" is two bytes and "𝄞" four (a surrogate pair in UTF-16).
    let text = "é𝄞x";
    let shaped = layout(text, f32::INFINITY);
    let past_end = shaped.hit_test_point(shaped.width() + 20.0, 2.0);
    assert_eq!(past_end.byte_index, text.len());
    assert!(!past_end.inside);
    let start = shaped.hit_test_point(0.0, 2.0);
    assert_eq!(start.byte_index, 0);
    assert!(text.is_char_boundary(shaped.hit_test_point(shaped.width() / 2.0, 2.0).byte_index));
}

#[test]
fn selection_rects_cover_the_selected_text_only() {
    let shaped = layout("hello world", f32::INFINITY);
    let rects = shaped.selection_rects(0, 5);
    assert_eq!(rects.len(), 1);
    assert!(rects[0].width() > 0 && (rects[0].width() as f32) < shaped.width());
    assert!(shaped.selection_rects(3, 3).is_empty());
}
