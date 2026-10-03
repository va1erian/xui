#![forbid(unsafe_code)]

//! Pagination: the flow cut into pages.
//!
//! The flow stays one vertical space. Page `k`'s text area runs from
//! `k * pitch` to `k * pitch + content`; the strip below it, up to the next
//! page's text (the bottom margin, the gap between sheets and the top
//! margin), is that page's *gap*. Text and floats treat a gap as a full-width
//! exclusion, so the line breaker moves whatever would cross one to the next
//! page with no second breaking pass.

/// How far a position may sit past a page's top and still count as at it.
const TOP_SLACK: f32 = 0.5;
/// Slack for float rounding when testing whether content fits on a page.
const FIT_SLACK: f32 = 0.1;

/// How the flow is cut into pages, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pages {
    /// The height of a page's text area.
    pub content: f32,
    /// The distance from one page's text top to the next one's (at least
    /// `content`).
    pub pitch: f32,
}

impl Pages {
    /// Pages of `content` pixels of text, `pitch` apart. `None` when either is
    /// not a positive, finite length or the pitch is shorter than the content.
    pub fn new(content: f32, pitch: f32) -> Option<Pages> {
        (content.is_finite() && pitch.is_finite() && content > 1.0 && pitch >= content)
            .then_some(Pages { content, pitch })
    }

    /// The page holding flow position `y` (a gap belongs to the page above).
    pub fn index_at(&self, y: f32) -> usize {
        ((y + TOP_SLACK) / self.pitch).floor().max(0.0) as usize
    }

    /// The top of page `index`'s text area.
    pub fn top(&self, index: usize) -> f32 {
        index as f32 * self.pitch
    }

    /// The bottom of page `index`'s text area.
    pub fn bottom(&self, index: usize) -> f32 {
        self.top(index) + self.content
    }

    /// Whether `y` is at the top of a page's text area.
    pub fn at_top(&self, y: f32) -> bool {
        (y - self.top(self.index_at(y))).abs() <= TOP_SLACK
    }

    /// The top of the page after the one holding `y`.
    pub fn next_top(&self, y: f32) -> f32 {
        self.top(self.index_at(y) + 1)
    }

    /// Where content starting at `y` begins when it must start a page: `y`
    /// itself at a page top, else the next page's top.
    pub fn break_before(&self, y: f32) -> f32 {
        if self.at_top(y) { y } else { self.next_top(y) }
    }

    /// The gap a span `y..y + height` runs into, as `(top, bottom)`: the one
    /// `y` is in, or the one below it when the span crosses the page's
    /// bottom. A span starting at a page's top never does, so a line or a
    /// picture taller than a page overflows it instead of moving forever.
    pub fn gap(&self, y: f32, height: f32) -> Option<(f32, f32)> {
        let index = self.index_at(y);
        let (bottom, next) = (self.bottom(index), self.top(index + 1));
        if y >= bottom || (y + height > bottom + FIT_SLACK && !self.at_top(y)) {
            Some((bottom, next))
        } else {
            None
        }
    }

    /// Whether the span `y..y + height` lies inside one page's text area.
    pub fn fits(&self, y: f32, height: f32) -> bool {
        let index = self.index_at(y);
        y >= self.top(index) - TOP_SLACK && y + height <= self.bottom(index) + FIT_SLACK
    }

    /// How many pages a flow `height` pixels tall fills (at least one).
    pub fn count(&self, height: f32) -> usize {
        self.index_at((height - TOP_SLACK).max(0.0)) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages() -> Pages {
        Pages::new(100.0, 150.0).unwrap()
    }

    #[test]
    fn index_and_gaps() {
        let p = pages();
        assert_eq!(p.index_at(0.0), 0);
        assert_eq!(p.index_at(120.0), 0);
        assert_eq!(p.index_at(149.9), 1);
        assert_eq!(p.index_at(150.0), 1);
        assert_eq!(p.gap(10.0, 20.0), None);
        assert_eq!(p.gap(90.0, 20.0), Some((100.0, 150.0)));
        assert_eq!(p.gap(110.0, 1.0), Some((100.0, 150.0)));
        // At a page top nothing is pushed, however tall.
        assert_eq!(p.gap(150.0, 500.0), None);
        assert_eq!(p.gap(150.0 - 1e-4, 500.0), None);
    }

    #[test]
    fn breaks_and_counts() {
        let p = pages();
        assert_eq!(p.break_before(0.0), 0.0);
        assert_eq!(p.break_before(40.0), 150.0);
        assert_eq!(p.break_before(150.0), 150.0);
        assert_eq!(p.break_before(120.0), 150.0);
        assert_eq!(p.count(0.0), 1);
        assert_eq!(p.count(100.0), 1);
        assert_eq!(p.count(140.0), 1);
        assert_eq!(p.count(151.0), 2);
        assert!(p.fits(150.0, 100.0));
        assert!(!p.fits(149.0 - 50.0, 10.0));
        assert!(!p.fits(120.0, 1.0));
    }

    #[test]
    fn new_refuses_bad_geometry() {
        assert!(Pages::new(100.0, 50.0).is_none());
        assert!(Pages::new(0.0, 50.0).is_none());
        assert!(Pages::new(f32::NAN, 50.0).is_none());
    }
}
