#![forbid(unsafe_code)]

//! Layout tests against a deterministic monospace shaper: every character is
//! half an em wide and a line is 1.25 em tall, so wrap points and float
//! geometry can be asserted exactly. A default-style character is 7 px wide
//! and a line is 18 px tall at 96 dpi.

mod blocks;
mod floats;
mod flow;
mod hit;
mod lines;
mod nav;
mod pages;
mod partial;

use std::any::Any;

use xui_core::backend::{FontSpec, TextHit, TextLayout, TextShaper};
use xui_core::geometry::Rect;

use super::Layout;
#[path = "../../tests/common/mod.rs"]
mod common;

use crate::model::{CharStyleId, Document, ParaStyle};
use common::{DocBuilder, Obj, Run, gradient_image};

/// A fixed-advance shaper that never wraps (the layout wraps).
pub(super) struct Mono;

struct MonoLayout {
    text: String,
    advance: f32,
    height: f32,
}

impl TextShaper for Mono {
    fn layout(&self, text: &str, spec: &FontSpec, _max: f32, dpi: u32) -> Box<dyn TextLayout> {
        let size = spec.size.0 * dpi as f32 / 96.0;
        Box::new(MonoLayout {
            text: text.to_owned(),
            advance: size * 0.5,
            height: (size * 1.25).round(),
        })
    }
}

impl TextLayout for MonoLayout {
    fn width(&self) -> f32 {
        self.text.chars().count() as f32 * self.advance
    }

    fn height(&self) -> f32 {
        self.height
    }

    fn baseline(&self) -> f32 {
        self.height * 0.75
    }

    fn hit_test_point(&self, x: f32, _y: f32) -> TextHit {
        let count = self.text.chars().count();
        let chars = (x / self.advance).round().clamp(0.0, count as f32) as usize;
        let byte = self
            .text
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(at, _)| at);
        TextHit {
            byte_index: byte,
            inside: x >= 0.0 && x <= self.width(),
        }
    }

    fn selection_rects(&self, start: usize, end: usize) -> Vec<Rect> {
        let chars = |upto: usize| self.text[..upto.min(self.text.len())].chars().count();
        let (a, b) = (chars(start), chars(end));
        if b <= a {
            return Vec::new();
        }
        vec![Rect::new(
            (a as f32 * self.advance).round() as i32,
            0,
            (b as f32 * self.advance).round() as i32,
            self.height as i32,
        )]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Lays `doc` out `width` pixels wide at 96 dpi.
pub(super) fn lay(doc: &Document, width: f32) -> Layout {
    let mut layout = Layout::new();
    layout.set_metrics(width, 96);
    layout.update(doc, &Mono);
    layout
}

/// A document of default-styled paragraphs.
pub(super) fn plain(texts: &[&str]) -> Document {
    let mut b = DocBuilder::new();
    for text in texts {
        b.paragraph(
            crate::model::ParaStyleId::DEFAULT,
            &[Run::Text(text, CharStyleId::DEFAULT)],
        );
    }
    b.finish()
}

/// A document of one paragraph in `style` with `runs`.
pub(super) fn with_style(
    style: ParaStyle,
    f: impl FnOnce(&mut DocBuilder) -> Vec<Run<'static>>,
) -> Document {
    let mut b = DocBuilder::new();
    let id = b.para_style(style);
    let runs = f(&mut b);
    b.paragraph(id, &runs);
    b.finish()
}

/// The text of each line of paragraph `index`, without hanging spaces.
pub(super) fn line_texts(doc: &Document, layout: &Layout, index: usize) -> Vec<String> {
    let text = doc.paragraphs()[index].text();
    layout.paragraphs()[index]
        .lines
        .iter()
        .map(|l| text[l.range.clone()].trim_end().to_owned())
        .collect()
}

/// Asserts that no word or inline image of `layout` overlaps a floating image.
pub(super) fn assert_clear_of_floats(layout: &Layout) {
    use super::PlacedKind;

    let floats: Vec<_> = layout
        .paragraphs()
        .iter()
        .flat_map(|p| p.floats.iter().map(|f| f.rect.shifted(p.y)))
        .collect();
    for para in layout.paragraphs() {
        for line in &para.lines {
            for item in &line.items {
                if !matches!(item.kind, PlacedKind::Text(_) | PlacedKind::Object { .. }) {
                    continue;
                }
                let (top, bottom) = (para.y + line.y, para.y + line.y + line.height);
                for f in &floats {
                    let overlaps = item.x < f.right - 0.01
                        && item.x + item.width > f.left + 0.01
                        && top < f.bottom - 0.01
                        && bottom > f.top + 0.01;
                    assert!(
                        !overlaps,
                        "item at x {} y {top} overlaps the float {f:?}",
                        item.x
                    );
                }
            }
        }
    }
}

#[test]
fn the_shape_cache_stays_bounded() {
    use super::shape_cache::ShapeCache;
    use crate::model::BlockKind;

    let mut cache = ShapeCache::default();
    cache.set_capacity(50);
    for i in 0..400 {
        let tick = cache.begin();
        let entry = cache.entry(96, CharStyleId::DEFAULT, BlockKind::Body, || {
            FontSpec::new(xui_core::Dip(14.0))
        });
        entry.word(&Mono, &format!("word{i}"), tick);
        cache.trim();
        assert!(cache.len() <= 50, "{} words after {i}", cache.len());
    }
    assert!(cache.len() > 0);
}
