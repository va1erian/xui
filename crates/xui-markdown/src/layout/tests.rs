#![forbid(unsafe_code)]

//! Layout tests against a deterministic monospace shaper: every character is
//! half an em wide and a line is 1.25 em tall (rounded), so wrap points and
//! float geometry can be asserted exactly. At 96 dpi a body character is 7 px
//! wide and a line 18 px tall; a code character is 6.5 px and its line 16 px.

mod blocks;
mod flow;
mod tables;
mod viewport;

use std::any::Any;
use std::ops::Range;
use std::sync::Arc;

use xui_core::Rect;
use xui_core::backend::{FontSpec, TextHit, TextLayout, TextShaper};

use super::geom::Piece;
use super::{BlockLayout, Decor, FRect, Layout};
use crate::model::Document;
use crate::style::{MarkdownStyle, Role};

/// Slack for float comparisons.
pub(super) const EPS: f32 = 0.01;

/// A fixed-advance shaper that wraps greedily at spaces.
pub(super) struct Mono;

struct MonoLayout {
    text: String,
    advance: f32,
    height: f32,
    lines: Vec<Range<usize>>,
}

impl MonoLayout {
    fn chars(&self, range: Range<usize>) -> usize {
        self.text[range].chars().count()
    }
}

fn wrap(text: &str, advance: f32, max: f32) -> Vec<Range<usize>> {
    let chars = |r: Range<usize>| text[r].chars().count() as f32 * advance;
    let mut words: Vec<Range<usize>> = Vec::new();
    let mut from = None;
    for (at, c) in text.char_indices() {
        match (c == ' ', from) {
            (false, None) => from = Some(at),
            (true, Some(start)) => {
                words.push(start..at);
                from = None;
            }
            _ => {}
        }
    }
    words.extend(from.map(|start| start..text.len()));
    let (mut lines, mut start, mut end) = (Vec::new(), 0, 0);
    for word in words {
        if end > start && chars(start..word.end) > max + EPS {
            lines.push(start..end);
            start = word.start;
        }
        end = word.end;
    }
    lines.push(start..end.max(start));
    lines
}

impl TextShaper for Mono {
    fn layout(&self, text: &str, spec: &FontSpec, max: f32, dpi: u32) -> Box<dyn TextLayout> {
        let size = spec.size.0 * dpi as f32 / 96.0;
        let advance = size * 0.5;
        Box::new(MonoLayout {
            lines: wrap(text, advance, max),
            text: text.to_owned(),
            advance,
            height: (size * 1.25).round(),
        })
    }
}

impl TextLayout for MonoLayout {
    fn width(&self) -> f32 {
        let widest = self.lines.iter().map(|l| self.chars(l.clone())).max();
        widest.unwrap_or(0) as f32 * self.advance
    }

    fn height(&self) -> f32 {
        self.lines.len() as f32 * self.height
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
        let mut rects = Vec::new();
        for (row, line) in self.lines.iter().enumerate() {
            let (a, b) = (start.max(line.start), end.min(line.end));
            if b <= a {
                continue;
            }
            let left = self.chars(line.start..a) as f32 * self.advance;
            let right = self.chars(line.start..b) as f32 * self.advance;
            rects.push(Rect::new(
                left.round() as i32,
                (row as f32 * self.height) as i32,
                right.round() as i32,
                ((row + 1) as f32 * self.height) as i32,
            ));
        }
        rects
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A layout of `markdown` for `width` device pixels at 96 dpi.
pub(super) fn lay(markdown: &str, width: f32) -> Layout {
    let mut layout = Layout::new(
        Arc::new(Document::parse(markdown)),
        MarkdownStyle::default(),
        Box::new(Mono),
        96,
    );
    layout.resize(width, 96);
    layout
}

/// Block `index` of `markdown` laid out `width` wide.
pub(super) fn one(markdown: &str, index: usize, width: f32) -> BlockLayout {
    let mut layout = lay(markdown, width);
    layout.ensure(0.0, 1.0e6);
    layout.block(index).expect("laid out").clone()
}

/// The text a piece shows.
pub(super) fn text(piece: &Piece) -> &str {
    &piece
        .layout
        .as_any()
        .downcast_ref::<MonoLayout>()
        .expect("a mono layout")
        .text
}

pub(super) fn texts(block: &BlockLayout) -> Vec<&str> {
    block.pieces.iter().map(text).collect()
}

pub(super) fn piece<'a>(block: &'a BlockLayout, word: &str) -> &'a Piece {
    block
        .pieces
        .iter()
        .find(|p| text(p) == word)
        .unwrap_or_else(|| panic!("no piece {word:?} in {:?}", texts(block)))
}

pub(super) fn fills(block: &BlockLayout, role: Role) -> Vec<FRect> {
    block
        .decor
        .iter()
        .filter_map(|d| match d {
            Decor::Fill(r, r_role) if *r_role == role => Some(*r),
            _ => None,
        })
        .collect()
}

pub(super) fn frames(block: &BlockLayout) -> Vec<FRect> {
    block
        .decor
        .iter()
        .filter_map(|d| match d {
            Decor::Frame(r, _) => Some(*r),
            _ => None,
        })
        .collect()
}

pub(super) fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= EPS
}
