//! Offscreen checks that the cosmic-text shaper's hit-testing and selection
//! geometry match the text it painted.

use xui_canvas::OffscreenBackend;
use xui_core::backend::{Backend, FontSpec, TextLayout};
use xui_core::units::dip;

/// Shapes `text` at a fixed size, unwrapped.
fn shape(backend: &OffscreenBackend, text: &str) -> Box<dyn TextLayout> {
    backend.layout_text(text, &FontSpec::new(dip(16.0)), f32::INFINITY, 96)
}

#[test]
fn a_hit_at_a_known_offset_maps_to_the_expected_character() {
    let backend = OffscreenBackend::new();
    let layout = shape(&backend, "l");
    let width = layout.width();
    assert!(width > 0.0, "the glyph advanced");
    let y = layout.height() / 2.0;

    assert_eq!(
        layout.hit_test_point(0.5, y).byte_index,
        0,
        "the leading half of a glyph maps before it"
    );
    assert_eq!(
        layout.hit_test_point(width - 0.5, y).byte_index,
        1,
        "the trailing half maps after it"
    );
}

#[test]
fn selecting_a_word_yields_a_box_covering_it() {
    let backend = OffscreenBackend::new();
    let text = "hello world";
    let layout = shape(&backend, text);

    let boxes = layout.selection_rects(6, 11);
    assert_eq!(boxes.len(), 1, "one box on a single line");
    let word = shape(&backend, &text[6..11]).width();
    let covered = boxes[0].width() as f32;
    assert!(
        (covered - word).abs() < 3.0,
        "the box covers the word: {covered} vs {word}"
    );
}

#[test]
fn a_mixed_ltr_rtl_run_is_not_off_by_a_whole_run() {
    let backend = OffscreenBackend::new();
    let text = "abc \u{05d0}\u{05d1}\u{05d2} def";
    let layout = shape(&backend, text);
    let width = layout.width();
    assert!(width > 0.0);

    let y = layout.height() / 2.0;
    let indices: Vec<usize> = (0..=200)
        .map(|step| {
            let x = width * step as f32 / 200.0;
            layout.hit_test_point(x, y).byte_index
        })
        .collect();
    assert!(
        indices.windows(2).any(|pair| pair[1] < pair[0]),
        "the right-to-left run walks byte indices backwards: {indices:?}"
    );
    assert!(
        indices.iter().all(|&index| index <= text.len()),
        "every byte index is in range"
    );
}
