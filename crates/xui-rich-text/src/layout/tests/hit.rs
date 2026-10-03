#![forbid(unsafe_code)]

//! Hit testing, caret and selection boxes, and the round trip on random
//! documents.

use proptest::prelude::*;
use xui_core::geometry::Point;
use xui_core::{Color, Dip};

use super::*;
use crate::layout::sample::gradient_image;
use crate::model::{
    Affinity, Align, DocPos, InlineImage, ListItem, ListKind, ParaStyleId, Side, Wrap,
};

fn centre(r: xui_core::geometry::Rect) -> Point {
    Point::new((r.left + r.right) / 2, (r.top + r.bottom) / 2)
}

#[test]
fn points_map_to_the_nearest_caret_position() {
    let doc = plain(&["hello world"]);
    let layout = lay(&doc, 400.0);
    let at = |x, y| layout.pos_at(&doc, Point::new(x, y));
    assert_eq!(at(22, 5), DocPos::new(0, 3), "22 px is 3.14 characters in");
    assert_eq!(at(-10, 5), DocPos::new(0, 0));
    assert_eq!(
        at(390, 5),
        DocPos::new(0, 11),
        "right of the text is the end"
    );
    assert_eq!(at(10, -50), DocPos::new(0, 1), "above is the first line");
    assert_eq!(at(10, 500), DocPos::new(0, 1), "below is the last line");
}

#[test]
fn a_click_right_of_a_wrapped_line_stays_before_its_hanging_space() {
    let doc = plain(&["aaaa bbbb cccc dddd eeee"]);
    let layout = lay(&doc, 140.0);
    assert_eq!(layout.pos_at(&doc, Point::new(300, 5)), DocPos::new(0, 19));
    assert_eq!(layout.pos_at(&doc, Point::new(300, 25)), DocPos::new(0, 24));
}

#[test]
fn the_caret_is_a_line_tall_box_at_the_position() {
    let doc = plain(&["hello world", "second"]);
    let layout = lay(&doc, 400.0);
    let r = layout.caret_rect(&doc, DocPos::new(0, 5));
    assert_eq!((r.left, r.top, r.bottom), (35, 0, 18));
    let r = layout.caret_rect(&doc, DocPos::new(1, 2));
    assert_eq!((r.left, r.top, r.bottom), (14, 18, 36));
}

#[test]
fn an_empty_paragraph_has_a_caret() {
    let doc = plain(&[""]);
    let layout = lay(&doc, 400.0);
    let r = layout.caret_rect(&doc, DocPos::new(0, 0));
    assert_eq!((r.left, r.top, r.bottom), (0, 0, 18));
}

#[test]
fn affinity_picks_a_side_of_a_soft_wrap() {
    let doc = plain(&["aaaa bbbb cccc dddd eeee"]);
    let layout = lay(&doc, 140.0);
    let pos = DocPos::new(0, 20);
    assert_eq!(
        layout.caret_rect_with(&doc, pos, Affinity::Downstream).top,
        18
    );
    let up = layout.caret_rect_with(&doc, pos, Affinity::Upstream);
    assert_eq!(up.top, 0);
    assert_eq!(up.left, 140);
}

#[test]
fn selections_have_a_box_per_line_and_a_stub_for_a_paragraph_break() {
    let doc = plain(&["aaaa bbbb cccc dddd eeee", "next"]);
    let layout = lay(&doc, 140.0);
    let rects = layout.selection_rects(&doc, DocPos::new(0, 5), DocPos::new(0, 22));
    assert_eq!(rects.len(), 2);
    assert_eq!((rects[0].left, rects[0].top), (35, 0));
    assert_eq!((rects[1].left, rects[1].right, rects[1].top), (0, 14, 18));

    let across = layout.selection_rects(&doc, DocPos::new(0, 20), DocPos::new(1, 2));
    assert_eq!(across.len(), 2);
    assert!(
        across[0].right > 28,
        "the break shows as a stub past the text"
    );
    assert_eq!(across[1].right, 14);
    assert!(
        layout
            .selection_rects(&doc, DocPos::new(0, 3), DocPos::new(0, 3))
            .is_empty()
    );
}

#[test]
fn images_are_found_by_point() {
    let mut b = DocBuilder::new();
    let image = |w, h, wrap| InlineImage {
        image: gradient_image(8, 8, Color::rgb(0, 0, 0), Color::rgb(9, 9, 9)),
        size: (Dip(w), Dip(h)),
        wrap,
        alt: String::new(),
    };
    let float = b.object(image(60.0, 50.0, Wrap::square(Side::Left)));
    let inline = b.object(image(20.0, 30.0, Wrap::Inline));
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[
            Run::Object(float),
            Run::Text("ab", CharStyleId::DEFAULT),
            Run::Object(inline),
        ],
    );
    let doc = b.finish();
    let layout = lay(&doc, 300.0);
    assert_eq!(layout.object_at(Point::new(10, 10)), Some(float));
    assert_eq!(layout.object_at(Point::new(200, 200)), None);
    // "ab" starts at x = 68; the inline image follows it.
    assert_eq!(layout.object_at(Point::new(68 + 14 + 5, 20)), Some(inline));
}

#[derive(Clone, Debug)]
struct ParaSpec {
    align: u8,
    list: u8,
    words: Vec<(usize, usize)>,
    big: bool,
    float: Option<(u8, u32, u32, usize)>,
}

fn para_spec() -> impl Strategy<Value = ParaSpec> {
    (
        0u8..4,
        0u8..3,
        prop::collection::vec((1usize..14, 1usize..4), 0..14),
        any::<bool>(),
        prop::option::of((0u8..4, 12u32..90, 12u32..70, 0usize..14)),
    )
        .prop_map(|(align, list, words, big, float)| ParaSpec {
            align,
            list,
            words,
            big,
            float,
        })
}

fn build(specs: &[ParaSpec]) -> Document {
    let mut b = DocBuilder::new();
    let big = b.char_style(crate::model::CharStyle {
        size: Dip(20.0),
        ..Default::default()
    });
    for spec in specs {
        let style = b.para_style(crate::model::ParaStyle {
            align: [Align::Left, Align::Center, Align::Right, Align::Justify][spec.align as usize],
            list: match spec.list {
                0 => None,
                1 => Some(ListItem {
                    kind: ListKind::Bullet,
                    level: 0,
                }),
                _ => Some(ListItem {
                    kind: ListKind::Numbered,
                    level: 0,
                }),
            },
            ..Default::default()
        });
        let object = spec.float.map(|(kind, w, h, at)| {
            let wrap = match kind {
                0 => Wrap::Inline,
                1 => Wrap::square(Side::Left),
                2 => Wrap::square(Side::Right),
                _ => Wrap::TopAndBottom { margin: Dip(8.0) },
            };
            let id = b.object(InlineImage {
                image: gradient_image(4, 4, Color::rgb(0, 0, 0), Color::rgb(9, 9, 9)),
                size: (Dip(w as f32), Dip(h as f32)),
                wrap,
                alt: String::new(),
            });
            (at, id)
        });
        let texts: Vec<String> = spec
            .words
            .iter()
            .map(|&(len, gap)| format!("{}{}", "w".repeat(len), " ".repeat(gap)))
            .collect();
        let mut runs: Vec<Run<'_>> = Vec::new();
        for (i, text) in texts.iter().enumerate() {
            if let Some((at, id)) = object
                && at == i
            {
                runs.push(Run::Object(id));
            }
            let style = if spec.big && i % 2 == 1 {
                big
            } else {
                CharStyleId::DEFAULT
            };
            runs.push(Run::Text(text, style));
        }
        if let Some((at, id)) = object
            && at >= texts.len()
        {
            runs.push(Run::Object(id));
        }
        b.paragraph(style, &runs);
    }
    b.finish()
}

/// Whether `byte` is just before or after the anchor of a floating image: the
/// anchor takes no room, so both look like the same caret position.
fn next_to_float_anchor(doc: &Document, para: usize, byte: usize) -> bool {
    let p = &doc.paragraphs()[para];
    p.objects().any(|(at, id)| {
        (at == byte || at + 3 == byte) && doc.objects().get(id).is_some_and(|o| o.wrap.is_float())
    })
}

proptest! {
    #[test]
    fn a_caret_position_hits_back_to_itself(
        specs in prop::collection::vec(para_spec(), 1..6),
        width in 80.0f32..420.0,
    ) {
        let doc = build(&specs);
        doc.check().unwrap();
        let layout = lay(&doc, width);
        assert_clear_of_floats(&layout);
        for (index, para) in doc.paragraphs().iter().enumerate() {
            let lines = &layout.paragraphs()[index].lines;
            prop_assert_eq!(lines[0].range.start, 0);
            prop_assert_eq!(lines.last().unwrap().range.end, para.text().len());
            for pair in lines.windows(2) {
                prop_assert_eq!(pair[0].range.end, pair[1].range.start);
            }
            let boundaries = para
                .text()
                .char_indices()
                .map(|(at, _)| at)
                .chain(std::iter::once(para.text().len()));
            for byte in boundaries {
                if next_to_float_anchor(&doc, index, byte) {
                    continue;
                }
                let pos = DocPos::new(index, byte);
                let rect = layout.caret_rect(&doc, pos);
                let back = layout.pos_at(&doc, centre(rect));
                prop_assert_eq!(back, pos, "caret {:?} at {:?} in {:?}", pos, rect, para.text());
            }
        }
    }
}
