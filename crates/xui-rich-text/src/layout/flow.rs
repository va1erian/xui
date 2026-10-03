#![forbid(unsafe_code)]

//! The document flow: paragraphs stacked into one continuous area, with the
//! floats of one paragraph carried into the next and relayout limited to what
//! changed.

use std::ops::Range;

use xui_core::backend::TextShaper;

use super::floats::FloatCtx;
use super::items::ShapeCtx;
use super::line::{Params, break_lines};
use super::resolve::{para_metrics, quote_rule_x, scale_for};
use super::shape_cache::ShapeCache;
use super::{Marker, ParaLayout};
use crate::model::{BlockKind, Document, ListKind};

/// The laid-out document: one [`ParaLayout`] per paragraph.
pub struct Layout {
    pub(crate) paras: Vec<ParaLayout>,
    width: f32,
    dpi: u32,
    cache: ShapeCache,
    height: f32,
}

impl Default for Layout {
    fn default() -> Layout {
        Layout::new()
    }
}

impl Layout {
    /// An empty layout; call [`set_metrics`](Layout::set_metrics) and
    /// [`update`](Layout::update).
    pub fn new() -> Layout {
        Layout {
            paras: Vec::new(),
            width: 0.0,
            dpi: 96,
            cache: ShapeCache::default(),
            height: 0.0,
        }
    }

    /// Sets the area width and DPI (device pixels). Returns whether anything
    /// changed, in which case every paragraph is relaid out by the next
    /// [`update`](Layout::update).
    pub fn set_metrics(&mut self, width: f32, dpi: u32) -> bool {
        if self.width == width && self.dpi == dpi {
            return false;
        }
        self.width = width;
        self.dpi = dpi;
        self.mark_all_dirty();
        true
    }

    /// The area width in pixels.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// The DPI the layout was made at.
    pub fn dpi(&self) -> u32 {
        self.dpi
    }

    /// Marks paragraph `index` for relayout.
    pub fn mark_dirty(&mut self, index: usize) {
        if let Some(p) = self.paras.get_mut(index) {
            p.dirty = true;
        }
    }

    /// Marks every paragraph for relayout and forgets shaped words (the
    /// document, and so its style table, may have been replaced).
    pub fn mark_all_dirty(&mut self) {
        self.cache.clear();
        for p in &mut self.paras {
            p.dirty = true;
        }
    }

    /// Records that `removed` paragraphs at `first` were replaced by
    /// `inserted` new ones, all of them needing layout.
    pub fn splice(&mut self, first: usize, removed: usize, inserted: usize) {
        let first = first.min(self.paras.len());
        let end = (first + removed).min(self.paras.len());
        self.paras
            .splice(first..end, (0..inserted).map(|_| ParaLayout::dirty()));
    }

    /// The paragraphs, top to bottom.
    pub fn paragraphs(&self) -> &[ParaLayout] {
        &self.paras
    }

    /// The height of the whole flow, floats included.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// The paragraphs that intersect the vertical span `top..bottom`.
    pub fn visible(&self, top: f32, bottom: f32) -> Range<usize> {
        let first = self.paras.partition_point(|p| p.bottom() <= top);
        let last = self.paras.partition_point(|p| p.y < bottom);
        first..last.max(first)
    }

    /// Relays out the dirty paragraphs. A paragraph below the first dirty one
    /// is reused (moved if need be) when it is clean and the floats entering
    /// it are the same relative to its top, so an edit usually relays out one
    /// paragraph. Returns the first paragraph that was laid out again.
    pub fn update(&mut self, doc: &Document, shaper: &dyn TextShaper) -> Option<usize> {
        let count = doc.paragraphs().len();
        if self.paras.len() != count {
            self.paras.resize_with(count, ParaLayout::dirty);
            self.mark_all_dirty();
        }
        let first = self.paras.iter().position(|p| p.dirty)?;
        let (mut y, mut ctx) = match first.checked_sub(1).map(|i| &self.paras[i]) {
            Some(prev) => (
                prev.bottom(),
                FloatCtx::from_relative(&prev.exit, prev.bottom()),
            ),
            None => (0.0, FloatCtx::default()),
        };
        for index in first..count {
            let entering = ctx.relative(y);
            let reusable = !self.paras[index].dirty && self.paras[index].entering == entering;
            if reusable {
                self.paras[index].y = y;
            } else {
                let number = list_number(doc, index);
                self.paras[index] = lay_out(
                    doc,
                    index,
                    Env {
                        shaper,
                        cache: &mut self.cache,
                        dpi: self.dpi,
                        width: self.width,
                    },
                    y,
                    &mut ctx,
                    number,
                );
            }
            let p = &self.paras[index];
            y = p.bottom();
            ctx = FloatCtx::from_relative(&p.exit, y);
        }
        self.height = y.max(ctx.bottom());
        self.cache.trim();
        Some(first)
    }
}

/// What laying out a paragraph needs besides the document.
struct Env<'a> {
    shaper: &'a dyn TextShaper,
    cache: &'a mut ShapeCache,
    dpi: u32,
    width: f32,
}

/// The number of the numbered item `index` among the consecutive items of its
/// level (1 for the first), or `None` when it is not numbered.
fn list_number(doc: &Document, index: usize) -> Option<usize> {
    let item = doc.styles().para(doc.paragraphs()[index].style()).list?;
    if item.kind != ListKind::Numbered {
        return None;
    }
    let mut number = 1;
    for para in doc.paragraphs()[..index].iter().rev() {
        match doc.styles().para(para.style()).list {
            Some(prev) if prev.level > item.level => {}
            Some(prev) if prev.level == item.level && prev.kind == ListKind::Numbered => {
                number += 1;
            }
            _ => break,
        }
    }
    Some(number)
}

/// Lays out paragraph `index` with its top at `y`, given the floats in `ctx`.
fn lay_out(
    doc: &Document,
    index: usize,
    env: Env<'_>,
    y: f32,
    ctx: &mut FloatCtx,
    number: Option<usize>,
) -> ParaLayout {
    let para = &doc.paragraphs()[index];
    let style = doc.styles().para(para.style());
    let scale = scale_for(env.dpi);
    let metrics = para_metrics(style, scale);
    let first_style = para.style_at(0);
    let tick = env.cache.begin();
    let mut shape = ShapeCtx {
        shaper: env.shaper,
        cache: env.cache,
        doc,
        dpi: env.dpi,
        scale,
        kind: style.kind,
        tick,
    };
    let items = shape.build(para);
    let entering = ctx.relative(y);
    let params = Params {
        area: env.width,
        left: metrics.left,
        right: metrics.right,
        first: metrics.first,
        align: style.align,
        spacing: metrics.spacing,
        empty_style: first_style,
    };
    let mut broken = break_lines(
        para.text(),
        items,
        &params,
        ctx,
        y + metrics.before,
        &mut shape,
    );
    for line in &mut broken.lines {
        line.y -= y;
        line.baseline -= y;
    }
    let first_baseline = broken.lines.first().map_or(0.0, |l| l.baseline);
    let marker = style.list.map(|item| {
        let text = match (item.kind, number) {
            (ListKind::Numbered, Some(n)) => format!("{n}."),
            _ => "\u{2022}".to_owned(),
        };
        let shaped = shape.text_item(&text, 0..text.len(), first_style, super::segment::Brk::None);
        let super::items::ItemKind::Text(layout) = shaped.kind else {
            unreachable!("a marker is shaped text")
        };
        Marker {
            x: (metrics.left - metrics.marker_gap - shaped.width).max(0.0),
            y: first_baseline - layout.baseline(),
            layout,
            style: first_style,
        }
    });
    let rule = (style.kind == BlockKind::Quote).then(|| {
        let top = broken.lines.first().map_or(0.0, |l| l.y);
        let bottom = broken.lines.last().map_or(top, |l| l.y + l.height);
        (quote_rule_x(metrics.left, scale), top, bottom)
    });
    let bottom = broken.end + metrics.after;
    ParaLayout {
        y,
        height: bottom - y,
        lines: broken.lines,
        floats: broken
            .floats
            .into_iter()
            .map(|f| super::PlacedFloat {
                rect: f.rect.shifted(-y),
                ..f
            })
            .collect(),
        marker,
        rule,
        entering,
        exit: ctx.relative(bottom),
        dirty: false,
    }
}
