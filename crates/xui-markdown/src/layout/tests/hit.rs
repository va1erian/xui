#![forbid(unsafe_code)]

//! Hit-testing: points to document positions and selections to rectangles.

use super::*;

/// [`Mono`] with a hit test that honours the row, so a wrapped paragraph's
/// second line can be hit.
struct Rows;

struct RowsLayout(MonoLayout);

impl TextShaper for Rows {
    fn layout(&self, text: &str, spec: &FontSpec, max: f32, dpi: u32) -> Box<dyn TextLayout> {
        let size = spec.size.0 * dpi as f32 / 96.0;
        Box::new(RowsLayout(MonoLayout {
            lines: wrap(text, size * 0.5, max),
            text: text.to_owned(),
            advance: size * 0.5,
            height: (size * 1.25).round(),
        }))
    }
}

impl TextLayout for RowsLayout {
    fn width(&self) -> f32 {
        self.0.width()
    }

    fn height(&self) -> f32 {
        self.0.height()
    }

    fn baseline(&self) -> f32 {
        self.0.baseline()
    }

    fn hit_test_point(&self, x: f32, y: f32) -> TextHit {
        let m = &self.0;
        let row = ((y / m.height).floor().max(0.0) as usize).min(m.lines.len() - 1);
        let line = m.lines[row].clone();
        let count = m.chars(line.clone());
        let col = (x / m.advance).round().clamp(0.0, count as f32) as usize;
        let byte = m.text[line.clone()]
            .char_indices()
            .nth(col)
            .map_or(line.end, |(at, _)| line.start + at);
        TextHit {
            byte_index: byte,
            inside: false,
        }
    }

    fn selection_rects(&self, start: usize, end: usize) -> Vec<Rect> {
        self.0.selection_rects(start, end)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// `markdown` laid out `width` wide with row-aware hit testing.
fn rows(markdown: &str, width: f32) -> Layout {
    let mut layout = Layout::new(
        Arc::new(Document::parse(markdown)),
        MarkdownStyle::default(),
        Box::new(Rows),
        96,
    );
    layout.resize(width, 96);
    layout.ensure(0.0, 1.0e6);
    layout
}

/// `markdown` laid out `width` wide, every block ready.
fn ready(markdown: &str, width: f32) -> Layout {
    let mut layout = lay(markdown, width);
    layout.ensure(0.0, 1.0e6);
    layout
}

fn band(r: &FRect) -> (f32, f32, f32, f32) {
    (r.x, r.y, r.w, r.h)
}

#[test]
fn an_empty_document_has_position_zero_everywhere() {
    let mut layout = ready("", 200.0);
    assert_eq!(layout.pos_at(0.0, 0.0), 0);
    assert_eq!(layout.pos_at(50.0, 500.0), 0);
    assert_eq!(layout.pos_at(-5.0, -5.0), 0);
}

#[test]
fn a_point_in_a_word_maps_to_the_nearest_character_boundary() {
    let mut layout = ready("alpha beta", 200.0);
    assert_eq!(layout.pos_at(0.0, 5.0), 0, "start");
    assert_eq!(layout.pos_at(20.0, 5.0), 3, "middle: 20 px is 2.86 chars");
    assert_eq!(layout.pos_at(35.0, 5.0), 5, "end of alpha");
    assert_eq!(layout.pos_at(42.0, 5.0), 6, "start of beta");
    assert_eq!(layout.pos_at(70.0, 5.0), 10, "end of beta");
}

#[test]
fn a_point_beside_a_line_goes_to_its_nearest_end() {
    let mut layout = ready("alpha beta", 200.0);
    assert_eq!(layout.pos_at(-30.0, 5.0), 0);
    assert_eq!(layout.pos_at(500.0, 5.0), 10);
}

#[test]
fn a_point_beside_the_second_block_goes_to_its_ends() {
    let mut layout = ready("alpha beta\n\ngamma delta", 200.0);
    assert_eq!(layout.pos_at(-30.0, 33.0), 10);
    assert_eq!(layout.pos_at(500.0, 33.0), 21);
}

#[test]
fn above_the_first_text_is_the_start_and_below_the_last_the_end() {
    let mut layout = ready("alpha beta\n\ngamma delta", 200.0);
    assert_eq!(layout.pos_at(30.0, -20.0), 0);
    assert_eq!(layout.pos_at(30.0, 1000.0), 21);
}

#[test]
fn the_gap_between_blocks_belongs_to_the_end_of_the_block_above() {
    let mut layout = ready("alpha beta\n\ngamma delta", 200.0);
    // The first block's text ends at y = 18 and the second starts at y = 28.
    assert_eq!(layout.top(1), 28.0);
    assert_eq!(layout.pos_at(20.0, 23.0), 10);
    assert_eq!(layout.pos_at(60.0, 19.0), 10);
    assert_eq!(layout.pos_at(20.0, 28.0), 13, "the next block's first line");
}

#[test]
fn the_second_line_of_a_wrapped_paragraph_is_found_by_its_row() {
    let mut layout = rows("alpha beta gamma delta", 80.0);
    // "alpha beta" fills the first line; "gamma delta" starts at byte 11.
    assert_eq!(layout.pos_at(21.0, 5.0), 3);
    assert_eq!(layout.pos_at(21.0, 23.0), 14);
    assert_eq!(layout.pos_at(500.0, 23.0), 22, "past the second line's end");
    assert_eq!(layout.pos_at(-5.0, 23.0), 11, "before the second line");
}

#[test]
fn across_the_pieces_of_a_mixed_paragraph_each_hits_its_own_text() {
    let mut layout = ready("alpha **beta** gamma", 1000.0);
    let block = layout.block(0).unwrap().clone();
    let (alpha, beta, gamma) = (
        piece(&block, "alpha").range.clone(),
        piece(&block, "beta").range.clone(),
        piece(&block, "gamma").range.clone(),
    );
    // alpha is at x 0..35, beta at 42..70, gamma at 77..112.
    assert_eq!(layout.pos_at(14.0, 5.0), alpha.start + 2);
    assert_eq!(layout.pos_at(42.0 + 14.0, 5.0), beta.start + 2);
    assert_eq!(layout.pos_at(77.0 + 21.0, 5.0), gamma.start + 3);
    assert_eq!(
        layout.pos_at(38.0, 5.0),
        alpha.end,
        "in the gap, the nearer word"
    );
    assert_eq!(layout.pos_at(74.0, 5.0), gamma.start);
    assert_eq!(layout.pos_at(500.0, 5.0), gamma.end);
}

#[test]
fn a_point_in_a_table_cell_hits_that_cell() {
    let mut layout = ready("| a | bb |\n|---|---|\n| cccc | d |", 300.0);
    let (header, body) = (layout.top(0), layout.top(1));
    // Cells are 44 and 58 px wide with 8 px of padding.
    let cell_a = layout.block(0).unwrap().pieces[0].range.clone();
    let cell_bb = layout.block(0).unwrap().pieces[1].range.clone();
    assert_eq!(layout.pos_at(8.0, header + 14.0), cell_a.start);
    assert_eq!(
        layout.pos_at(44.0 + 8.0 + 7.0, header + 14.0),
        cell_bb.start + 1
    );
    let cccc = layout.block(1).unwrap().pieces[0].range.clone();
    let d = layout.block(1).unwrap().pieces[1].range.clone();
    assert_eq!(layout.pos_at(8.0 + 14.0, body + 14.0), cccc.start + 2);
    assert_eq!(layout.pos_at(44.0 + 8.0 + 20.0, body + 14.0), d.end);
}

#[test]
fn a_point_on_a_list_marker_goes_to_the_start_of_the_item_text() {
    let mut layout = ready("- one\n- two", 200.0);
    // The marker is left of the text, which starts at x = 24.
    assert_eq!(layout.pos_at(5.0, 5.0), 0);
    let second = layout.top(1) + 5.0;
    assert_eq!(layout.pos_at(5.0, second), 3);
    assert_eq!(layout.pos_at(24.0 + 7.0, 5.0), 1);
    let block = layout.block(0).unwrap();
    assert!(block.pieces.iter().all(|p| !p.range.is_empty()));
}

#[test]
fn both_halves_of_a_tab_in_code_map_to_the_nearer_edge_of_the_tab() {
    let mut layout = ready("```\na\tb\n```", 300.0);
    // Shaped as "a    b": a code character is 6.5 px, the text starts at x = 8.
    let at = |n: f32| 8.0 + n * 6.5;
    let y = 16.0;
    assert_eq!(layout.pos_at(at(0.0), y), 0, "before a");
    assert_eq!(layout.pos_at(at(1.0), y), 1, "start of the tab");
    assert_eq!(layout.pos_at(at(2.0), y), 1, "first half of the tab");
    assert_eq!(layout.pos_at(at(3.0), y), 2, "second half of the tab");
    assert_eq!(layout.pos_at(at(4.0), y), 2);
    assert_eq!(layout.pos_at(at(5.0), y), 2, "before b");
    assert_eq!(layout.pos_at(at(6.0), y), 3, "after b");
}

#[test]
fn a_selection_across_words_is_one_band_per_line() {
    let layout = ready("alpha **beta** gamma", 1000.0);
    let rects = layout.selection_rects(0, &(0..16));
    assert_eq!(rects.len(), 1, "{rects:?}");
    assert_eq!(
        band(&rects[0]),
        (0.0, 0.0, 112.0, 18.0),
        "over the spaces too"
    );
}

#[test]
fn a_selection_on_a_wrapped_paragraph_is_a_band_for_each_line() {
    let layout = rows("alpha beta gamma delta", 80.0);
    let rects = layout.selection_rects(0, &(6..16));
    assert_eq!(rects.len(), 2, "{rects:?}");
    assert_eq!(band(&rects[0]), (42.0, 0.0, 28.0, 18.0));
    assert_eq!(band(&rects[1]), (0.0, 18.0, 35.0, 18.0));
}

#[test]
fn a_selection_ending_in_a_word_stops_at_that_character() {
    let layout = ready("alpha **beta** gamma", 1000.0);
    let block = layout.block(0).unwrap();
    let beta = piece(block, "beta").range.start;
    let rects = layout.selection_rects(0, &(0..beta + 2));
    assert_eq!(rects.len(), 1);
    assert_eq!(band(&rects[0]), (0.0, 0.0, 42.0 + 14.0, 18.0));
}

#[test]
fn a_selection_starting_mid_word_starts_at_that_character() {
    let layout = ready("alpha beta", 200.0);
    let rects = layout.selection_rects(0, &(2..4));
    assert_eq!(rects.len(), 1);
    assert_eq!(band(&rects[0]), (14.0, 0.0, 14.0, 18.0));
}

#[test]
fn selected_table_cells_are_separate_bands() {
    let layout = ready("| a | bb |\n|---|---|\n| cccc | d |", 300.0);
    let rects = layout.selection_rects(0, &(0..3));
    assert_eq!(rects.len(), 2, "{rects:?}");
    assert_eq!(band(&rects[0]), (8.0, 8.0, 7.0, 18.0));
    assert_eq!(band(&rects[1]), (52.0, 8.0, 14.0, 18.0));
    let one = layout.selection_rects(1, &(5..7));
    assert_eq!(one.len(), 1);
    assert_eq!(band(&one[0]), (8.0 + 14.0, 8.0, 14.0, 18.0));
}

#[test]
fn a_code_line_with_a_tab_covers_the_shaped_width_of_the_selection() {
    let layout = ready("```\na\tb\n```", 300.0);
    let whole = layout.selection_rects(0, &(0..3));
    assert_eq!(whole.len(), 1);
    // Six shaped characters of 6.5 px, the text at x = 8, y = 8.
    assert!(
        close(whole[0].x, 8.0) && close(whole[0].w, 39.0),
        "{whole:?}"
    );
    assert!(
        close(whole[0].y, 8.0) && close(whole[0].h, 16.0),
        "{whole:?}"
    );
    // Just the tab: four shaped spaces after the "a". Mono rounds edges.
    let tab = layout.selection_rects(0, &(1..2));
    assert_eq!(tab.len(), 1);
    assert!((tab[0].x - (8.0 + 6.5)).abs() <= 1.0, "{tab:?}");
    assert!((tab[0].w - 26.0).abs() <= 1.0, "{tab:?}");
}

#[test]
fn a_block_outside_the_selection_has_no_rectangles() {
    let layout = ready("alpha beta\n\ngamma delta", 200.0);
    assert!(layout.selection_rects(1, &(0..10)).is_empty());
    assert!(layout.selection_rects(0, &(10..21)).is_empty());
    assert!(layout.selection_rects(0, &(4..4)).is_empty());
    assert!(
        layout.selection_rects(9, &(0..21)).is_empty(),
        "no such block"
    );
    let inside = layout.selection_rects(1, &(10..15));
    assert_eq!(inside.len(), 1);
    assert_eq!(band(&inside[0]), (0.0, 0.0, 35.0, 18.0));
}

#[test]
fn a_selection_on_a_block_not_laid_out_has_no_rectangles() {
    let layout = lay("alpha beta", 200.0);
    assert!(layout.selection_rects(0, &(0..5)).is_empty());
}
