#![forbid(unsafe_code)]

//! Exclusion rectangles: the space floating images take from the text.

use xui_core::geometry::Rect;

use super::pages::Pages;
use crate::model::Side;

/// A rectangle in `f32` pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FRect {
    /// Left edge.
    pub left: f32,
    /// Top edge.
    pub top: f32,
    /// Right edge.
    pub right: f32,
    /// Bottom edge.
    pub bottom: f32,
}

impl FRect {
    /// A rectangle from its edges.
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> FRect {
        FRect {
            left,
            top,
            right,
            bottom,
        }
    }

    /// The width.
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    /// The height.
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// The rectangle moved down by `dy`.
    pub fn shifted(&self, dy: f32) -> FRect {
        FRect::new(self.left, self.top + dy, self.right, self.bottom + dy)
    }

    /// The rectangle rounded to whole pixels.
    pub fn to_rect(&self) -> Rect {
        Rect::new(
            self.left.round() as i32,
            self.top.round() as i32,
            self.right.round() as i32,
            self.bottom.round() as i32,
        )
    }
}

/// Which part of the line a float takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExclKind {
    /// The left edge; text flows to its right.
    Left,
    /// The right edge; text flows to its left.
    Right,
    /// The whole width.
    Band,
}

/// Space text must stay out of: a float's image and its margin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Excl {
    /// The excluded rectangle.
    pub rect: FRect,
    /// Which part of the line it takes.
    pub kind: ExclKind,
}

/// The horizontal band a line may use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Interval {
    pub lo: f32,
    pub hi: f32,
}

impl Interval {
    pub fn width(&self) -> f32 {
        (self.hi - self.lo).max(0.0)
    }
}

/// The exclusions active at the current flow position, in area pixels, and
/// the page gaps when the flow is paginated.
#[derive(Clone, Debug, Default)]
pub(crate) struct FloatCtx {
    excl: Vec<Excl>,
    pages: Option<Pages>,
}

impl FloatCtx {
    /// The context holding `rel` (exclusions relative to `origin`).
    pub fn from_relative(rel: &[Excl], origin: f32) -> FloatCtx {
        FloatCtx {
            excl: rel
                .iter()
                .map(|e| Excl {
                    rect: e.rect.shifted(origin),
                    kind: e.kind,
                })
                .collect(),
            pages: None,
        }
    }

    /// The same context on `pages`.
    pub fn with_pages(mut self, pages: Option<Pages>) -> FloatCtx {
        self.pages = pages;
        self
    }

    /// The pages the flow is cut into.
    pub fn pages(&self) -> Option<Pages> {
        self.pages
    }

    /// The top of the next page when a span `y..y + h` would run into a page
    /// gap (see [`Pages::gap`]).
    pub fn page_push(&self, y: f32, h: f32) -> Option<f32> {
        self.pages?.gap(y, h).map(|(_, next)| next)
    }

    /// The page gap a span `y..y + h` runs into, as a full-width exclusion.
    fn gap_excl(&self, y: f32, h: f32) -> Option<Excl> {
        let (top, bottom) = self.pages?.gap(y, h)?;
        Some(Excl {
            rect: FRect::new(f32::MIN, top, f32::MAX, bottom),
            kind: ExclKind::Band,
        })
    }

    /// Where content that must clear every float starts when the flow is at
    /// `y`: below the lowest exclusion still active there.
    pub fn clear_below(&self, y: f32) -> f32 {
        self.excl.iter().map(|e| e.rect.bottom).fold(y, f32::max)
    }

    /// The exclusions still active below `y`, relative to `y`.
    pub fn relative(&self, y: f32) -> Vec<Excl> {
        self.excl
            .iter()
            .filter(|e| e.rect.bottom > y)
            .map(|e| Excl {
                rect: e.rect.shifted(-y),
                kind: e.kind,
            })
            .collect()
    }

    /// The exclusions a line of height `h` at `y` overlaps, the page gap it
    /// would cross included.
    fn overlapping(&self, y: f32, h: f32) -> impl Iterator<Item = Excl> + '_ {
        self.excl
            .iter()
            .filter(move |e| e.rect.top < y + h.max(0.01) && e.rect.bottom > y)
            .copied()
            .chain(self.gap_excl(y, h.max(0.01)))
    }

    /// The free interval for a line of height `h` at `y` inside `[lo, hi]`.
    pub fn interval(&self, y: f32, h: f32, lo: f32, hi: f32) -> Interval {
        let mut iv = Interval { lo, hi };
        for e in self.overlapping(y, h) {
            match e.kind {
                ExclKind::Left => iv.lo = iv.lo.max(e.rect.right),
                ExclKind::Right => iv.hi = iv.hi.min(e.rect.left),
                ExclKind::Band => return Interval { lo: hi, hi },
            }
        }
        iv.hi = iv.hi.max(iv.lo);
        iv
    }

    /// The next edge below `y` where the interval of a line of height `h`
    /// could widen: the smallest bottom among the overlapping exclusions.
    pub fn next_bottom(&self, y: f32, h: f32) -> Option<f32> {
        self.overlapping(y, h)
            .map(|e| e.rect.bottom)
            .min_by(f32::total_cmp)
    }

    /// Places a square float of `w` by `h` at or below `y` against `side` of
    /// an area `area` wide, stacking under earlier floats on that side.
    /// Returns the image rectangle.
    pub fn place_square(
        &mut self,
        area: f32,
        y: f32,
        size: (f32, f32),
        margin: f32,
        side: Side,
    ) -> FRect {
        let (w, h) = size;
        let kind = match side {
            Side::Left => ExclKind::Left,
            Side::Right => ExclKind::Right,
        };
        let mut top = y;
        loop {
            let hit = self
                .excl
                .iter()
                .filter(|e| e.kind == kind || e.kind == ExclKind::Band)
                .filter(|e| e.rect.top < top + h + margin && e.rect.bottom > top)
                .map(|e| e.rect.bottom)
                .chain(self.page_push(top, h + margin))
                .fold(None, |acc: Option<f32>, b| {
                    Some(acc.map_or(b, |a| a.max(b)))
                });
            match hit {
                Some(bottom) if bottom > top => top = bottom,
                _ => break,
            }
        }
        let (image, excl) = match side {
            Side::Left => (
                FRect::new(0.0, top, w, top + h),
                FRect::new(0.0, top, w + margin, top + h + margin),
            ),
            Side::Right => (
                FRect::new(area - w, top, area, top + h),
                FRect::new(area - w - margin, top, area, top + h + margin),
            ),
        };
        self.excl.push(Excl { rect: excl, kind });
        image
    }

    /// Places a band float of `w` by `h` at or below `y`, its image at `x`.
    /// Returns the image rectangle.
    pub fn place_band(
        &mut self,
        area: f32,
        y: f32,
        size: (f32, f32),
        margin: f32,
        x: f32,
    ) -> FRect {
        let (w, h) = size;
        let mut top = y;
        while let Some(bottom) = self
            .excl
            .iter()
            .filter(|e| e.rect.top < top + h + 2.0 * margin && e.rect.bottom > top)
            .map(|e| e.rect.bottom)
            .chain(self.page_push(top, h + 2.0 * margin))
            .min_by(f32::total_cmp)
        {
            top = bottom;
        }
        let image = FRect::new(x, top + margin, x + w, top + margin + h);
        self.excl.push(Excl {
            rect: FRect::new(0.0, top, area, image.bottom + margin),
            kind: ExclKind::Band,
        });
        image
    }
}
