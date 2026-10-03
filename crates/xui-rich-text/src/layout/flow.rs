#![forbid(unsafe_code)]

//! The document flow: paragraphs stacked into one continuous area, with the
//! floats of one paragraph carried into the next and relayout limited to what
//! changed.

use std::ops::Range;
use std::sync::Arc;

use xui_core::backend::TextShaper;

use super::floats::FloatCtx;
use super::items::ShapeCtx;
use super::line::{Params, break_lines};
use super::resolve::{para_metrics, quote_rule_x, scale_for};
use super::shape_cache::ShapeCache;
use super::{Marker, Pages, ParaLayout};
use crate::model::{BlockKind, Document, ListKind};

/// The laid-out document: one [`ParaLayout`] per paragraph.
pub struct Layout {
    pub(crate) paras: Vec<ParaLayout>,
    pub(super) width: f32,
    pub(super) dpi: u32,
    cache: ShapeCache,
    pub(super) height: f32,
    pub(super) pages: Option<Pages>,
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
            pages: None,
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
        for p in &mut self.paras {
            // The old heights fit the old width: estimate again.
            p.height = 0.0;
            p.lines.clear();
        }
        true
    }

    /// Paginates the flow (`None` for one continuous column). Returns
    /// whether anything changed, in which case every paragraph is relaid out
    /// by the next [`update`](Layout::update).
    pub fn set_pages(&mut self, pages: Option<Pages>) -> bool {
        if self.pages == pages {
            return false;
        }
        self.pages = pages;
        self.mark_all_dirty();
        true
    }

    /// The pages the flow is cut into, if it is paginated.
    pub fn pages(&self) -> Option<Pages> {
        self.pages
    }

    /// How many pages the flow fills: 1 when it is not paginated.
    pub fn page_count(&self) -> usize {
        self.pages.map_or(1, |p| p.count(self.height))
    }

    /// The page (from 0) holding flow position `y`: 0 when not paginated.
    pub fn page_at(&self, y: f32) -> usize {
        self.pages.map_or(0, |p| p.index_at(y))
    }

    /// Whether paragraph `index`, laid out at `laid_y`, is still right at
    /// `y`: always off pages; on pages when it has not moved, or when it
    /// starts no page and lies inside one page's text area both where it was
    /// laid out and at `y`.
    pub(super) fn fits_at(&self, index: usize, y: f32) -> bool {
        let (Some(pages), Some(p)) = (self.pages, self.paras.get(index)) else {
            return true;
        };
        let extent = p.extent();
        (p.laid_y - y).abs() < 0.01
            || (!p.page_break && pages.fits(p.laid_y, extent) && pages.fits(y, extent))
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
        self.fit(doc);
        let first = self.first_pending()?;
        self.run_from(doc, shaper, first, usize::MAX);
        Some(first)
    }

    /// Makes the paragraph list match the document's length.
    pub(super) fn fit(&mut self, doc: &Document) {
        let count = doc.paragraphs().len();
        if self.paras.len() != count {
            self.paras.resize_with(count, ParaLayout::dirty);
            self.mark_all_dirty();
        }
    }

    /// The first paragraph that is dirty or still unverified.
    pub(super) fn first_pending(&self) -> Option<usize> {
        self.paras.iter().position(|p| p.dirty || p.speculative)
    }

    /// Walks the flow in order from `first` (everything before it is laid out
    /// and verified), reusing paragraphs that still fit and laying out at most
    /// `budget` others. Returns the index it stopped at.
    pub(super) fn run_from(
        &mut self,
        doc: &Document,
        shaper: &dyn TextShaper,
        first: usize,
        budget: usize,
    ) -> usize {
        let count = self.paras.len();
        let pages = self.pages;
        let (mut y, mut ctx) = match first.checked_sub(1).map(|i| &self.paras[i]) {
            Some(prev) => (
                prev.bottom(),
                FloatCtx::from_relative(&prev.exit, prev.bottom()).with_pages(pages),
            ),
            None => (0.0, FloatCtx::default().with_pages(pages)),
        };
        let numbers = list_numbers(doc);
        let (mut index, mut laid) = (first, 0);
        while index < count {
            let entering = ctx.relative(y);
            let reusable = !self.paras[index].dirty
                && self.paras[index].entering == entering
                && self.paras[index].number == numbers[index]
                && self.fits_at(index, y);
            if reusable {
                self.paras[index].y = y;
                self.paras[index].laid_y = y;
                self.paras[index].speculative = false;
            } else if laid < budget {
                laid += 1;
                self.lay_one(doc, shaper, index, y, &mut ctx, numbers[index]);
            } else {
                // Not reusable and out of budget: a later slice must revisit
                // it even if it is not dirty (its entering floats changed).
                self.paras[index].speculative = true;
                break;
            }
            let p = &self.paras[index];
            y = p.bottom();
            ctx = FloatCtx::from_relative(&p.exit, y).with_pages(pages);
            index += 1;
        }
        self.reposition(doc);
        self.cache.trim();
        index
    }

    /// Lays out paragraph `index` with its top at `y` given the floats in
    /// `ctx`, numbering its marker `number` (from [`list_numbers`]).
    pub(super) fn lay_one(
        &mut self,
        doc: &Document,
        shaper: &dyn TextShaper,
        index: usize,
        y: f32,
        ctx: &mut FloatCtx,
        number: Option<usize>,
    ) {
        let env = Env {
            shaper,
            cache: &mut self.cache,
            dpi: self.dpi,
            width: self.width,
        };
        self.paras[index] = lay_out(doc, index, env, y, ctx, number);
    }
}

/// What laying out a paragraph needs besides the document.
struct Env<'a> {
    shaper: &'a dyn TextShaper,
    cache: &'a mut ShapeCache,
    pub(super) dpi: u32,
    pub(super) width: f32,
}

/// Each paragraph's list number: for a numbered item, its place (from 1) among
/// the consecutive numbered items of its level, with deeper items in between
/// skipped; `None` for anything else. One forward pass: a counter per level,
/// cleared by a non-list paragraph, by a shallower item (for the levels below
/// it) and by a bullet at the same level.
pub(super) fn list_numbers(doc: &Document) -> Vec<Option<usize>> {
    let mut counters: Vec<Option<usize>> = Vec::new();
    doc.paragraphs()
        .iter()
        .map(|para| {
            let Some(item) = doc.styles().para(para.style()).list else {
                counters.clear();
                return None;
            };
            let level = usize::from(item.level);
            counters.truncate(level + 1);
            counters.resize(level + 1, None);
            let number =
                (item.kind == ListKind::Numbered).then(|| counters[level].map_or(1, |n| n + 1));
            counters[level] = number;
            number
        })
        .collect()
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
    let start = match ctx.pages() {
        Some(pages) if style.page_break_before && !pages.at_top(y) => pages.break_before(y),
        _ => y + metrics.before,
    };
    let params = Params {
        area: env.width,
        left: metrics.left,
        right: metrics.right,
        first: metrics.first,
        align: style.align,
        spacing: metrics.spacing,
        empty_style: first_style,
    };
    let mut broken = break_lines(para.text(), items, &params, ctx, start, &mut shape);
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
        laid_y: y,
        page_break: style.page_break_before,
        dirty: false,
        speculative: false,
        text: Arc::from(para.text()),
        number,
    }
}
