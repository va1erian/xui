//! The sample document laid out with the real cosmic-text shaper: invariants,
//! not exact pixels, since glyph metrics depend on the machine's fonts.

use xui_canvas::OffscreenBackend;
use xui_core::backend::Backend;
use xui_core::geometry::Point;
mod common;

use common::sample_document;
use xui_rich_text::layout::{FRect, PlacedKind};
use xui_rich_text::{DocPos, Layout};

fn laid_out(width: f32, dpi: u32) -> (xui_rich_text::Document, Layout) {
    let doc = sample_document();
    let shaper = OffscreenBackend::new().text_shaper();
    let mut layout = Layout::new();
    layout.set_metrics(width, dpi);
    layout.update(&doc, shaper.as_ref());
    (doc, layout)
}

fn float_rects(layout: &Layout) -> Vec<FRect> {
    layout
        .paragraphs()
        .iter()
        .flat_map(|p| p.floats.iter().map(|f| f.rect.shifted(p.y)))
        .collect()
}

#[test]
fn text_never_enters_a_float_at_any_width() {
    for width in [160.0, 240.0, 320.0, 480.0, 620.0] {
        let (_, layout) = laid_out(width, 96);
        let floats = float_rects(&layout);
        assert_eq!(floats.len(), 2);
        for para in layout.paragraphs() {
            for line in &para.lines {
                for item in &line.items {
                    if !matches!(item.kind, PlacedKind::Text(_) | PlacedKind::Object { .. }) {
                        continue;
                    }
                    let (top, bottom) = (para.y + line.y, para.y + line.y + line.height);
                    for f in &floats {
                        let overlaps = item.x < f.right - 0.5
                            && item.x + item.width > f.left + 0.5
                            && top < f.bottom - 0.5
                            && bottom > f.top + 0.5;
                        assert!(
                            !overlaps,
                            "width {width}: item at {} overlaps {f:?}",
                            item.x
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn lines_tile_every_paragraph_and_stay_inside_the_area() {
    for width in [160.0, 320.0, 620.0] {
        let (doc, layout) = laid_out(width, 96);
        for (index, para) in layout.paragraphs().iter().enumerate() {
            let len = doc.paragraphs()[index].text().len();
            assert_eq!(para.lines[0].range.start, 0);
            assert_eq!(para.lines.last().unwrap().range.end, len);
            for pair in para.lines.windows(2) {
                assert_eq!(pair[0].range.end, pair[1].range.start);
            }
            for line in &para.lines {
                for item in &line.items {
                    if matches!(item.kind, PlacedKind::Text(_)) {
                        assert!(item.x >= -0.5 && item.x + item.width <= width + 0.5);
                    }
                }
            }
        }
    }
}

#[test]
fn caret_positions_round_trip_through_hit_testing() {
    let (doc, layout) = laid_out(420.0, 96);
    let mut total = 0;
    let mut wrong = Vec::new();
    for (index, para) in doc.paragraphs().iter().enumerate() {
        for (byte, _) in para.text().char_indices() {
            let at_anchor = para.objects().any(|(a, id)| {
                (a == byte || a + 3 == byte)
                    && doc.objects().get(id).is_some_and(|o| o.wrap.is_float())
            });
            if at_anchor {
                continue;
            }
            let pos = DocPos::new(index, byte);
            let rect = layout.caret_rect(&doc, pos);
            let back = layout.pos_at(&doc, Point::new(rect.left, (rect.top + rect.bottom) / 2));
            total += 1;
            if back != pos {
                wrong.push((pos, back));
            }
        }
    }
    // Sub-pixel rounding of glyph edges may move a caret by one character in
    // a narrow glyph; anything more than a handful means the geometry is off.
    assert!(
        wrong.len() * 50 <= total,
        "{} of {total} carets did not round-trip: {:?}",
        wrong.len(),
        &wrong[..wrong.len().min(8)]
    );
}

#[test]
fn a_higher_dpi_makes_the_flow_taller() {
    let (_, normal) = laid_out(420.0, 96);
    let (_, big) = laid_out(840.0, 192);
    assert!(big.height() > normal.height() * 1.5);
}
