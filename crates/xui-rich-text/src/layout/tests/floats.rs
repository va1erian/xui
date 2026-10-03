#![forbid(unsafe_code)]

//! Floating images: geometry beside, below and across paragraphs.

use xui_core::{Color, Dip};

use super::*;
use crate::layout::PlacedKind;
use crate::model::{InlineImage, ObjectId, ParaStyleId, Side, Wrap};

const EPS: f32 = 0.01;
const TEXT: &str = "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq rrrr ssss tttt \
    aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo pppp qqqq rrrr ssss tttt";

fn picture(b: &mut DocBuilder, w: f32, h: f32, wrap: Wrap) -> ObjectId {
    b.object(InlineImage {
        image: crate::layout::sample::gradient_image(
            8,
            8,
            Color::rgb(0, 0, 0),
            Color::rgb(9, 9, 9),
        ),
        size: (Dip(w), Dip(h)),
        wrap,
        alt: String::new(),
    })
}

fn left(margin: f32) -> Wrap {
    Wrap::Square {
        side: Side::Left,
        margin: Dip(margin),
    }
}

fn right(margin: f32) -> Wrap {
    Wrap::Square {
        side: Side::Right,
        margin: Dip(margin),
    }
}

/// The x extent of the text items on `line`.
fn extent(line: &crate::layout::Line) -> Option<(f32, f32)> {
    let items: Vec<_> = line
        .items
        .iter()
        .filter(|i| matches!(i.kind, PlacedKind::Text(_)))
        .collect();
    Some((items.first()?.x, items.last().map(|i| i.x + i.width)?))
}

#[test]
fn text_flows_beside_a_left_float_and_returns_below_it() {
    let mut b = DocBuilder::new();
    let img = picture(&mut b, 60.0, 50.0, left(8.0));
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Object(img), Run::Text(TEXT, CharStyleId::DEFAULT)],
    );
    let doc = b.finish();
    let layout = lay(&doc, 300.0);
    assert_clear_of_floats(&layout);
    let para = &layout.paragraphs()[0];
    let float = para.floats[0].rect;
    assert_eq!(
        (float.left, float.top, float.right, float.bottom),
        (0.0, 0.0, 60.0, 50.0)
    );
    let (beside, below): (Vec<_>, Vec<_>) = para.lines.iter().partition(|l| l.y < 58.0);
    assert_eq!(beside.len(), 4);
    for line in &beside {
        let (x, end) = extent(line).unwrap();
        assert!(x >= 68.0 - EPS, "beside the float: {x}");
        assert!(end <= 300.0 + EPS);
    }
    assert!(
        (extent(below[0]).unwrap().0 - 0.0).abs() < EPS,
        "full width again"
    );
    let widest = |lines: &[&crate::layout::Line]| {
        lines
            .iter()
            .filter_map(|l| extent(l))
            .map(|(a, b)| b - a)
            .fold(0.0, f32::max)
    };
    assert!(
        widest(&beside) <= 232.0 + EPS && widest(&below) > 232.0,
        "narrower beside, full below"
    );
}

#[test]
fn text_flows_beside_a_right_float() {
    let mut b = DocBuilder::new();
    let img = picture(&mut b, 60.0, 50.0, right(8.0));
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Object(img), Run::Text(TEXT, CharStyleId::DEFAULT)],
    );
    let doc = b.finish();
    let layout = lay(&doc, 300.0);
    assert_clear_of_floats(&layout);
    let para = &layout.paragraphs()[0];
    assert!((para.floats[0].rect.left - 240.0).abs() < EPS);
    for line in para.lines.iter().filter(|l| l.y < 58.0) {
        assert!(extent(line).unwrap().1 <= 232.0 + EPS);
    }
    assert!(para.lines.iter().any(|l| extent(l).unwrap().1 > 232.0));
}

#[test]
fn a_float_pushes_into_the_following_paragraph() {
    let mut b = DocBuilder::new();
    let img = picture(&mut b, 60.0, 200.0, left(8.0));
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Object(img), Run::Text("short", CharStyleId::DEFAULT)],
    );
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Text(TEXT, CharStyleId::DEFAULT)],
    );
    let doc = b.finish();
    let layout = lay(&doc, 300.0);
    assert_clear_of_floats(&layout);
    let second = &layout.paragraphs()[1];
    let mut narrowed = 0;
    for line in &second.lines {
        let top = second.y + line.y;
        let x = extent(line).unwrap().0;
        if top < 208.0 {
            narrowed += 1;
            assert!(x >= 68.0 - EPS, "line at {top} starts at {x}");
        } else {
            assert!(x.abs() < EPS);
        }
    }
    assert!(narrowed > 3);
    assert!(layout.height() >= 200.0);
}

#[test]
fn floats_on_both_sides_leave_the_middle() {
    let mut b = DocBuilder::new();
    let l = picture(&mut b, 60.0, 50.0, left(8.0));
    let r = picture(&mut b, 60.0, 50.0, right(8.0));
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[
            Run::Object(l),
            Run::Object(r),
            Run::Text(TEXT, CharStyleId::DEFAULT),
        ],
    );
    let doc = b.finish();
    let layout = lay(&doc, 300.0);
    assert_clear_of_floats(&layout);
    for line in layout.paragraphs()[0].lines.iter().filter(|l| l.y < 58.0) {
        let (x, end) = extent(line).unwrap();
        assert!(x >= 68.0 - EPS && end <= 232.0 + EPS);
    }
}

#[test]
fn a_second_float_on_a_side_stacks_below_the_first() {
    let mut b = DocBuilder::new();
    let a = picture(&mut b, 60.0, 40.0, left(8.0));
    let c = picture(&mut b, 60.0, 40.0, left(8.0));
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[
            Run::Object(a),
            Run::Object(c),
            Run::Text(TEXT, CharStyleId::DEFAULT),
        ],
    );
    let doc = b.finish();
    let layout = lay(&doc, 300.0);
    assert_clear_of_floats(&layout);
    let floats = &layout.paragraphs()[0].floats;
    assert_eq!(floats.len(), 2);
    assert!(floats[1].rect.top >= floats[0].rect.bottom + 8.0 - EPS);
    assert!((floats[1].rect.left - 0.0).abs() < EPS);
}

#[test]
fn a_float_wider_than_the_room_drops_the_text_below_it() {
    let mut b = DocBuilder::new();
    let img = picture(&mut b, 60.0, 50.0, left(8.0));
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[
            Run::Object(img),
            Run::Text("abcdefgh", CharStyleId::DEFAULT),
        ],
    );
    let doc = b.finish();
    let layout = lay(&doc, 100.0);
    assert_clear_of_floats(&layout);
    let line = &layout.paragraphs()[0].lines[0];
    assert!(
        line.y >= 58.0 - EPS,
        "the line starts below the float: {}",
        line.y
    );
    assert!(extent(line).unwrap().0.abs() < EPS);
}

#[test]
fn top_and_bottom_floats_take_a_band_of_their_own() {
    let mut b = DocBuilder::new();
    let img = picture(&mut b, 100.0, 40.0, Wrap::TopAndBottom { margin: Dip(8.0) });
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[
            Run::Text("before", CharStyleId::DEFAULT),
            Run::Object(img),
            Run::Text(TEXT, CharStyleId::DEFAULT),
        ],
    );
    let doc = b.finish();
    let layout = lay(&doc, 300.0);
    assert_clear_of_floats(&layout);
    let para = &layout.paragraphs()[0];
    let band = para.floats[0].rect;
    assert!(
        band.top >= para.lines[0].y + para.lines[0].height,
        "below the first line"
    );
    let after = para
        .lines
        .iter()
        .find(|l| l.y > band.top - EPS)
        .expect("text after the band");
    assert!(
        after.y >= band.bottom + 8.0 - EPS,
        "text resumes below the band and margin"
    );
    assert!(
        para.lines
            .iter()
            .all(|l| l.y + l.height <= band.top - 8.0 + EPS || l.y >= band.bottom + 8.0 - EPS)
    );
}

#[test]
fn an_oversized_float_is_clamped_to_the_area() {
    let mut b = DocBuilder::new();
    let img = picture(&mut b, 500.0, 100.0, left(0.0));
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Object(img), Run::Text("hello", CharStyleId::DEFAULT)],
    );
    let doc = b.finish();
    let layout = lay(&doc, 200.0);
    let rect = layout.paragraphs()[0].floats[0].rect;
    assert!(rect.right <= 200.0 + EPS);
    assert!(
        (rect.height() - 40.0).abs() < EPS,
        "the aspect ratio is kept"
    );
}
