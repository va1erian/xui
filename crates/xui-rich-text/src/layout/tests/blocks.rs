#![forbid(unsafe_code)]

//! Line spacing, indents, block defaults and list markers.

use xui_core::Dip;

use super::*;
use crate::layout::PlacedKind;
use crate::model::{BlockKind, LineSpacing, ListItem, ListKind, ParaStyle};

const EPS: f32 = 0.01;

#[test]
fn line_spacing_scales_or_fixes_the_line_height() {
    let height = |spacing| {
        let doc = with_style(
            ParaStyle {
                line_spacing: spacing,
                ..ParaStyle::default()
            },
            |_| vec![Run::Text("hi", CharStyleId::DEFAULT)],
        );
        lay(&doc, 400.0).paragraphs()[0].lines[0].height
    };
    assert!((height(LineSpacing::Multiple(2.0)) - 36.0).abs() < EPS);
    assert!((height(LineSpacing::Exactly(Dip(30.0))) - 30.0).abs() < EPS);
}

#[test]
fn space_before_and_after_pad_the_paragraph() {
    let doc = with_style(
        ParaStyle {
            space_before: Dip(10.0),
            space_after: Dip(6.0),
            ..ParaStyle::default()
        },
        |_| vec![Run::Text("hi", CharStyleId::DEFAULT)],
    );
    let p = lay(&doc, 400.0).paragraphs()[0].clone();
    assert!((p.lines[0].y - 10.0).abs() < EPS);
    assert!((p.height - 34.0).abs() < EPS);
}

#[test]
fn indents_narrow_the_lines_and_the_first_line_has_its_own() {
    let style = ParaStyle {
        indent_left: Dip(14.0),
        indent_right: Dip(14.0),
        indent_first: Dip(21.0),
        ..ParaStyle::default()
    };
    let doc = with_style(style, |_| {
        vec![Run::Text(
            "aaaa bbbb cccc dddd eeee ffff gggg hhhh",
            CharStyleId::DEFAULT,
        )]
    });
    let layout = lay(&doc, 140.0);
    let lines = &layout.paragraphs()[0].lines;
    assert!((lines[0].items[0].x - 35.0).abs() < EPS);
    assert!((lines[1].items[0].x - 14.0).abs() < EPS);
    for line in lines {
        let right = line
            .items
            .iter()
            .filter(|i| matches!(i.kind, PlacedKind::Text(_)))
            .map(|i| i.x + i.width)
            .fold(0.0, f32::max);
        assert!(right <= 126.0 + EPS);
    }
}

#[test]
fn a_hanging_indent_pulls_the_first_line_left() {
    let style = ParaStyle {
        indent_left: Dip(28.0),
        indent_first: Dip(-14.0),
        ..ParaStyle::default()
    };
    let doc = with_style(style, |_| {
        vec![Run::Text("hello world", CharStyleId::DEFAULT)]
    });
    let layout = lay(&doc, 400.0);
    assert!((layout.paragraphs()[0].lines[0].items[0].x - 14.0).abs() < EPS);
}

#[test]
fn headings_are_larger_and_quotes_are_indented_and_ruled() {
    let heading = with_style(
        ParaStyle {
            kind: BlockKind::Heading(1),
            ..ParaStyle::default()
        },
        |_| vec![Run::Text("Title", CharStyleId::DEFAULT)],
    );
    let layout = lay(&heading, 400.0);
    let line = &layout.paragraphs()[0].lines[0];
    assert!((line.items[0].width - 5.0 * 7.0 * 1.8).abs() < EPS);
    assert!(line.height >= 31.0);

    let quote = with_style(
        ParaStyle {
            kind: BlockKind::Quote,
            ..ParaStyle::default()
        },
        |_| vec![Run::Text("Quoted", CharStyleId::DEFAULT)],
    );
    let layout = lay(&quote, 400.0);
    let p = &layout.paragraphs()[0];
    assert!((p.lines[0].items[0].x - 24.0).abs() < EPS);
    let (x, top, bottom) = p.rule.expect("a quote has a rule");
    assert!(x < 24.0 && bottom > top);
}

fn list_doc(items: &[(ListKind, u8)]) -> (Document, Layout) {
    let mut b = DocBuilder::new();
    for &(kind, level) in items {
        let id = b.para_style(ParaStyle {
            list: Some(ListItem { kind, level }),
            ..ParaStyle::default()
        });
        b.paragraph(id, &[Run::Text("item", CharStyleId::DEFAULT)]);
    }
    let doc = b.finish();
    let layout = lay(&doc, 400.0);
    (doc, layout)
}

/// The marker's width in characters.
fn marker_chars(layout: &Layout, index: usize) -> usize {
    (layout.paragraphs()[index]
        .marker
        .as_ref()
        .unwrap()
        .layout
        .width()
        / 7.0)
        .round() as usize
}

#[test]
fn numbers_count_consecutive_items_of_a_level_across_nested_ones() {
    use ListKind::Numbered;
    let mut items = vec![(Numbered, 0); 8];
    items.push((Numbered, 1));
    items.push((Numbered, 0));
    items.push((Numbered, 0));
    let (_, layout) = list_doc(&items);
    assert_eq!(marker_chars(&layout, 7), 2, "8. is two characters");
    assert_eq!(
        marker_chars(&layout, 8),
        2,
        "the nested item restarts at 1."
    );
    assert_eq!(marker_chars(&layout, 9), 2, "9. after the nested item");
    assert_eq!(
        marker_chars(&layout, 10),
        3,
        "10. continues the outer count"
    );
}

#[test]
fn a_bullet_or_plain_paragraph_restarts_numbering() {
    use ListKind::{Bullet, Numbered};
    let mut items = vec![(Numbered, 0); 9];
    items.push((Bullet, 0));
    items.push((Numbered, 0));
    let (_, layout) = list_doc(&items);
    assert_eq!(marker_chars(&layout, 10), 2, "1. after a bullet");
    assert_eq!(marker_chars(&layout, 9), 1, "a bullet is one character");
}

#[test]
fn list_text_is_indented_by_level_and_the_marker_hangs_left_of_it() {
    use ListKind::Bullet;
    let (_, layout) = list_doc(&[(Bullet, 0), (Bullet, 1)]);
    let text_x = |i: usize| layout.paragraphs()[i].lines[0].items[0].x;
    assert!((text_x(0) - 24.0).abs() < EPS);
    assert!((text_x(1) - 48.0).abs() < EPS);
    for i in 0..2 {
        let m = layout.paragraphs()[i].marker.as_ref().unwrap();
        assert!(m.x + m.layout.width() < text_x(i));
    }
}

#[test]
fn changing_one_item_renumbers_the_clean_items_after_it() {
    use crate::model::{EditOp, ParaStylePatch};
    let (mut doc, mut layout) = list_doc(&[(ListKind::Numbered, 0); 10]);
    assert_eq!(marker_chars(&layout, 9), 3, "10.");
    let patch = ParaStylePatch::list(Some(ListItem {
        kind: ListKind::Bullet,
        level: 0,
    }));
    doc.apply(EditOp::SetParaStyle { paras: 0..1, patch })
        .unwrap();
    layout.mark_dirty(0);
    layout.update(&doc, &Mono);
    assert_eq!(
        marker_chars(&layout, 9),
        2,
        "only paragraph 0 was dirty, yet the last item is now 9."
    );
}

/// The definition `list_numbers` computes in one pass (the previous
/// per-paragraph implementation): scan back from each item, skip deeper items,
/// count same-level numbered ones, stop at anything else.
fn numbers_by_scanning_back(items: &[Option<(ListKind, u8)>]) -> Vec<Option<usize>> {
    (0..items.len())
        .map(|i| {
            let (kind, level) = items[i]?;
            if kind != ListKind::Numbered {
                return None;
            }
            let mut number = 1;
            for prev in items[..i].iter().rev() {
                match *prev {
                    Some((_, l)) if l > level => {}
                    Some((ListKind::Numbered, l)) if l == level => number += 1,
                    _ => break,
                }
            }
            Some(number)
        })
        .collect()
}

proptest::proptest! {
    #[test]
    fn list_numbers_match_scanning_back(
        raw in proptest::collection::vec(proptest::option::of((proptest::bool::ANY, 0u8..3)), 0..40)
    ) {
        use crate::model::{EditOp, ParaStylePatch};
        let items: Vec<Option<(ListKind, u8)>> = raw
            .iter()
            .map(|o| o.map(|(numbered, level)| {
                (if numbered { ListKind::Numbered } else { ListKind::Bullet }, level)
            }))
            .collect();
        let text = vec!["x"; items.len().max(1)].join("\n");
        let mut doc = Document::from_plain_text(&text);
        for (i, item) in items.iter().enumerate() {
            let list = item.map(|(kind, level)| ListItem { kind, level });
            let patch = ParaStylePatch::list(list);
            doc.apply(EditOp::SetParaStyle { paras: i..i + 1, patch }).unwrap();
        }
        let mut expected = numbers_by_scanning_back(&items);
        expected.resize(doc.paragraphs().len(), None);
        proptest::prop_assert_eq!(crate::layout::flow::list_numbers(&doc), expected);
    }
}
