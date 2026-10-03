#![forbid(unsafe_code)]

//! The page a document is laid out on in page view: paper size and margins.

use xui_core::Dip;

/// Dip per millimetre (96 dip to the inch, 25.4 mm to the inch).
const DIP_PER_MM: f32 = 96.0 / 25.4;

/// The smallest content width or height a page may leave inside its margins.
const MIN_CONTENT: f32 = 48.0;
/// The largest paper side accepted (about 2.6 m).
const MAX_SIDE: f32 = 10_000.0;

/// Converts millimetres to dip.
pub fn mm(value: f32) -> Dip {
    Dip(value * DIP_PER_MM)
}

/// Paper size and margins, in dip. The document's text is laid out in the
/// area the margins leave; in continuous view the setup is kept but unused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSetup {
    /// The paper width.
    pub width: Dip,
    /// The paper height.
    pub height: Dip,
    /// The left margin.
    pub left: Dip,
    /// The top margin.
    pub top: Dip,
    /// The right margin.
    pub right: Dip,
    /// The bottom margin.
    pub bottom: Dip,
}

impl PageSetup {
    /// ISO A4 portrait (210 x 297 mm) with 25 mm margins: the default.
    pub fn a4() -> PageSetup {
        PageSetup::new(mm(210.0), mm(297.0)).with_margins(mm(25.0))
    }

    /// US Letter portrait (8.5 x 11 in) with 1 in margins.
    pub fn letter() -> PageSetup {
        PageSetup::new(Dip(8.5 * 96.0), Dip(11.0 * 96.0)).with_margins(Dip(96.0))
    }

    /// A page of `width` by `height` with no margins.
    pub const fn new(width: Dip, height: Dip) -> PageSetup {
        PageSetup {
            width,
            height,
            left: Dip(0.0),
            top: Dip(0.0),
            right: Dip(0.0),
            bottom: Dip(0.0),
        }
    }

    /// The same paper with all four margins set to `margin`.
    pub fn with_margins(self, margin: Dip) -> PageSetup {
        PageSetup {
            left: margin,
            top: margin,
            right: margin,
            bottom: margin,
            ..self
        }
    }

    /// Whether the paper is wider than it is tall.
    pub fn is_landscape(&self) -> bool {
        self.width.0 > self.height.0
    }

    /// The same page turned a quarter: width and height swapped, the margins
    /// following the paper (the top margin becomes the left one).
    pub fn rotated(self) -> PageSetup {
        PageSetup {
            width: self.height,
            height: self.width,
            left: self.top,
            top: self.right,
            right: self.bottom,
            bottom: self.left,
        }
    }

    /// The same page in portrait (`false`) or landscape (`true`), rotated only
    /// if it is not already.
    pub fn oriented(self, landscape: bool) -> PageSetup {
        if self.is_landscape() == landscape || self.width == self.height {
            self
        } else {
            self.rotated()
        }
    }

    /// The width text is laid out in.
    pub fn content_width(&self) -> Dip {
        Dip(self.width.0 - self.left.0 - self.right.0)
    }

    /// The height of the text area of one page.
    pub fn content_height(&self) -> Dip {
        Dip(self.height.0 - self.top.0 - self.bottom.0)
    }

    /// Checks that every length is finite, the paper is at most about 2.6 m a
    /// side and the margins leave at least half an inch of text each way.
    pub fn check(&self) -> Result<(), String> {
        let all = [
            self.width,
            self.height,
            self.left,
            self.top,
            self.right,
            self.bottom,
        ];
        if all.iter().any(|d| !d.0.is_finite() || d.0 < 0.0) {
            return Err("page lengths must be finite and not negative".into());
        }
        if self.width.0 > MAX_SIDE || self.height.0 > MAX_SIDE {
            return Err("the paper is too large".into());
        }
        if self.content_width().0 < MIN_CONTENT || self.content_height().0 < MIN_CONTENT {
            return Err("the margins leave no room for text".into());
        }
        Ok(())
    }
}

impl Default for PageSetup {
    fn default() -> PageSetup {
        PageSetup::a4()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_valid() {
        for page in [PageSetup::a4(), PageSetup::letter()] {
            assert!(page.check().is_ok());
            assert!(!page.is_landscape());
            assert!(page.rotated().is_landscape());
            assert_eq!(page.rotated().rotated(), page);
        }
        assert!((PageSetup::a4().width.0 - 793.7).abs() < 0.1);
    }

    #[test]
    fn oriented_turns_only_when_needed() {
        let a4 = PageSetup::a4();
        assert_eq!(a4.oriented(false), a4);
        assert_eq!(a4.oriented(true), a4.rotated());
        assert_eq!(a4.rotated().oriented(true), a4.rotated());
    }

    #[test]
    fn check_refuses_bad_pages() {
        let a4 = PageSetup::a4();
        assert!(a4.with_margins(Dip(400.0)).check().is_err());
        assert!(PageSetup::new(Dip(f32::NAN), Dip(100.0)).check().is_err());
        assert!(PageSetup::new(Dip(20_000.0), Dip(1000.0)).check().is_err());
        assert!(
            PageSetup {
                left: Dip(-1.0),
                ..a4
            }
            .check()
            .is_err()
        );
    }
}
