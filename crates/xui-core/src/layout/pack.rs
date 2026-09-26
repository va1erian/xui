#![forbid(unsafe_code)]

//! Preferred ("pack") sizes: the content-driven size a layout wants to open at.
//!
//! Like [`anchored`](super::anchor::anchored), these are pure functions of
//! device-pixel inputs, so they are scale-equivariant: scaling every input by a
//! factor scales the result by the same factor. A caller converts its [`Dip`]
//! design values once with [`Dip::to_px`](crate::units::Dip::to_px) and the
//! arithmetic never sees a DPI.

use crate::geometry::{Rect, Size};
use crate::layout::{Insets, StackDirection};
use crate::units::Dip;

/// The chrome around a strip of items, already in device pixels.
///
/// A horizontal strip sums its left and right margins into [`main`](Self::main)
/// and its top and bottom into [`cross`](Self::cross); a vertical strip swaps
/// the two.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PackEdges {
    /// Total margin along the strip's main axis.
    pub(crate) main: i32,
    /// Total margin across the main axis.
    pub(crate) cross: i32,
    /// Gap added between adjacent items.
    pub(crate) spacing: i32,
}

/// The preferred size of a stack, in device pixels at `dpi`.
///
/// `naturals[i]` is slot `i`'s natural size in device pixels; `insets` and
/// `spacing` are design values scaled here. This is the `Dip`-aware entry point
/// behind [`Stack::preferred_size`](super::Stack::preferred_size).
pub(crate) fn stack_size(
    direction: StackDirection,
    insets: Insets,
    spacing: Dip,
    naturals: &[Size],
    dpi: u32,
) -> Size {
    let (main, cross) = match direction {
        StackDirection::Horizontal => (insets.left + insets.right, insets.top + insets.bottom),
        StackDirection::Vertical => (insets.top + insets.bottom, insets.left + insets.right),
    };
    let edges = PackEdges {
        main: main.to_px(dpi).value().max(0),
        cross: cross.to_px(dpi).value().max(0),
        spacing: spacing.to_px(dpi).value().max(0),
    };
    strip(naturals, edges, direction == StackDirection::Vertical)
}

/// The preferred size of a strip of `naturals`, in device pixels.
///
/// `naturals[i]` is the natural size of item `i`. When `vertical` is set the
/// main axis is the height, otherwise the width. The main extent is
/// `edges.main + sum(main extents) + spacing * (n - 1)` and the cross extent is
/// `edges.cross + max(cross extents)`, each clamped non-negative. An empty
/// strip is just its margins.
pub(crate) fn strip(naturals: &[Size], edges: PackEdges, vertical: bool) -> Size {
    let mut main = i64::from(edges.main.max(0));
    let mut cross = 0i32;
    for (index, natural) in naturals.iter().enumerate() {
        let (item_main, item_cross) = if vertical {
            (natural.height, natural.width)
        } else {
            (natural.width, natural.height)
        };
        main += i64::from(item_main.max(0));
        if index > 0 {
            main += i64::from(edges.spacing.max(0));
        }
        cross = cross.max(item_cross.max(0));
    }
    cross += edges.cross.max(0);
    let main = i32::try_from(main).unwrap_or(i32::MAX);
    if vertical {
        Size::new(cross, main)
    } else {
        Size::new(main, cross)
    }
}

/// The preferred size of a free (anchored) layout: the largest right and bottom
/// edge among `bounds`, in device pixels, clamped non-negative.
///
/// A free layout keeps absolute design bounds, so its content extent is the
/// furthest edge any child reaches rather than a sum of natural sizes. An empty
/// slice has no extent.
pub fn free_preferred(bounds: &[Rect]) -> Size {
    let mut right = 0i32;
    let mut bottom = 0i32;
    for rect in bounds {
        right = right.max(rect.right.max(0));
        bottom = bottom.max(rect.bottom.max(0));
    }
    Size::new(right, bottom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;

    fn size(width: i32, height: i32) -> Size {
        Size::new(width, height)
    }

    #[test]
    fn a_column_sums_heights_and_takes_the_widest() {
        let naturals = [size(30, 20), size(10, 40), size(25, 15)];
        let edges = PackEdges {
            main: 8,
            cross: 8,
            spacing: 5,
        };
        // Heights 20 + 40 + 15, two gaps of 5, top+bottom margins 8.
        // Width is the widest item (30) plus left+right margins.
        assert_eq!(strip(&naturals, edges, true), size(38, 93));
    }

    #[test]
    fn a_row_sums_widths_and_takes_the_tallest() {
        let naturals = [size(30, 20), size(10, 40), size(25, 15)];
        let edges = PackEdges {
            main: 4,
            cross: 6,
            spacing: 2,
        };
        // Widths 30 + 10 + 25, two gaps of 2, left+right margins 4.
        // Height is the tallest item (40) plus top+bottom margins.
        assert_eq!(strip(&naturals, edges, false), size(73, 46));
    }

    #[test]
    fn one_item_has_no_spacing_and_an_empty_strip_is_its_margins() {
        let edges = PackEdges {
            main: 10,
            cross: 20,
            spacing: 7,
        };
        assert_eq!(strip(&[size(5, 6)], edges, false), size(15, 26));
        assert_eq!(strip(&[], edges, true), size(20, 10));
    }

    #[test]
    fn negative_naturals_never_shrink_the_result() {
        let edges = PackEdges {
            main: 0,
            cross: 0,
            spacing: 0,
        };
        assert_eq!(
            strip(&[size(-100, -100), size(10, 4)], edges, false),
            size(10, 4)
        );
    }

    #[test]
    fn a_free_layout_reports_the_content_extent() {
        let bounds = [Rect::new(0, 0, 100, 40), Rect::new(10, 10, 60, 200)];
        assert_eq!(free_preferred(&bounds), size(100, 200));
        assert_eq!(free_preferred(&[]), size(0, 0));
        assert_eq!(free_preferred(&[Rect::new(-50, -50, -10, -10)]), size(0, 0));
    }

    #[test]
    fn the_pack_arithmetic_is_scale_equivariant() {
        // Scaling every input by the same factor scales the result, which is
        // what lets a caller convert design values once and keep the arithmetic
        // DPI-independent.
        let naturals = [size(30, 20), size(10, 40), size(25, 15)];
        let edges = PackEdges {
            main: 8,
            cross: 8,
            spacing: 5,
        };
        let bounds = [Rect::new(0, 0, 100, 40), Rect::new(10, 10, 60, 200)];
        for factor in [2, 3, 4] {
            assert_eq!(
                strip(&naturals, edges, true).width * factor,
                strip(&scale(&naturals, factor), scale_edges(edges, factor), true).width
            );
            assert_eq!(
                strip(&naturals, edges, false).height * factor,
                strip(&scale(&naturals, factor), scale_edges(edges, factor), false).height
            );
            assert_eq!(
                free_preferred(&bounds).width * factor,
                free_preferred(&scale_rects(&bounds, factor)).width
            );
            assert_eq!(
                free_preferred(&bounds).height * factor,
                free_preferred(&scale_rects(&bounds, factor)).height
            );
        }
    }

    fn scale(sizes: &[Size], factor: i32) -> Vec<Size> {
        sizes
            .iter()
            .map(|size| Size::new(size.width * factor, size.height * factor))
            .collect()
    }

    fn scale_edges(edges: PackEdges, factor: i32) -> PackEdges {
        PackEdges {
            main: edges.main * factor,
            cross: edges.cross * factor,
            spacing: edges.spacing * factor,
        }
    }

    fn scale_rects(rects: &[Rect], factor: i32) -> Vec<Rect> {
        rects
            .iter()
            .map(|rect| {
                Rect::new(
                    rect.left * factor,
                    rect.top * factor,
                    rect.right * factor,
                    rect.bottom * factor,
                )
            })
            .collect()
    }
}
