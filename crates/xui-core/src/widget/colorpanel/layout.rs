#![forbid(unsafe_code)]

//! The Full tab's geometry: where the preview, SV field, hue track, HEX row and
//! the four read-out boxes go inside a panel of a given size.
//!
//! Every value is derived from the panel's own (local) rectangle, so a resize
//! only re-runs this function; the widget moves the children it owns.

use crate::geometry::Rect;
use crate::units::Dip;

/// Padding around the full page.
const PAD: Dip = Dip(12.0);
/// Gap between related controls.
const GAP: Dip = Dip(8.0);
/// Height of the preview/SV top row.
const TOP_H: Dip = Dip(120.0);
/// Height of the hue track row.
const HUE_H: Dip = Dip(16.0);
/// Height of the HEX and read-out rows.
const BOX_H: Dip = Dip(48.0);
/// Width of the HEX copy button.
const COPY_W: Dip = Dip(64.0);
/// Inset of an edit inside its labelled group frame.
const FRAME_PAD: Dip = Dip(4.0);
/// How far below a group's top its edit starts, clearing the border title.
const FRAME_TOP: Dip = Dip(18.0);

/// The rectangles of the Full tab's children, in the panel's own coordinates.
pub(crate) struct FullLayout {
    /// The flat preview square.
    pub preview: Rect,
    /// The saturation/value field.
    pub field: Rect,
    /// The hue track.
    pub hue: Rect,
    /// The five group frames: HEX, RGB, CMYK, HSV, HSL.
    pub groups: [Rect; 5],
    /// The five edit boxes inside those frames.
    pub edits: [Rect; 5],
    /// The HEX copy button.
    pub copy: Rect,
}

/// Lays the Full tab out inside a panel of `size`.
pub(crate) fn full(size: Rect, dpi: u32) -> FullLayout {
    let pad = PAD.to_px(dpi).value().max(0);
    let gap = GAP.to_px(dpi).value().max(0);
    let top_h = TOP_H.to_px(dpi).value().max(1);
    let hue_h = HUE_H.to_px(dpi).value().max(1);
    let box_h = BOX_H.to_px(dpi).value().max(1);
    let copy_w = COPY_W.to_px(dpi).value().max(1);

    let inner = Rect::new(
        size.left + pad,
        size.top + pad,
        (size.right - pad).max(size.left + pad + 1),
        (size.bottom - pad).max(size.top + pad + 1),
    );

    let preview_w = (inner.width() / 3).max(1);
    let preview = Rect::new(
        inner.left,
        inner.top,
        inner.left + preview_w,
        inner.top + top_h,
    );
    let field = Rect::new(
        (preview.right + gap).min(inner.right),
        inner.top,
        inner.right,
        inner.top + top_h,
    );

    let hue_top = preview.bottom + gap;
    let hue = Rect::new(
        inner.left,
        hue_top,
        inner.right,
        (hue_top + hue_h).min(inner.bottom),
    );

    let hex_top = hue.bottom + gap;
    let hex_bottom = hex_top + box_h;
    let hex_group = Rect::new(
        inner.left,
        hex_top,
        (inner.right - copy_w - gap).max(inner.left + 1),
        hex_bottom,
    );
    let copy = Rect::new(inner.right - copy_w, hex_top, inner.right, hex_bottom);

    let mut groups = [Rect::default(); 5];
    let mut edits = [Rect::default(); 5];
    groups[0] = hex_group;
    edits[0] = edit_of(hex_group, dpi);

    let row_top = hex_bottom + gap;
    let slot = ((inner.width() - gap * 3) / 4).max(1);
    for index in 0..4 {
        let left = inner.left + index as i32 * (slot + gap);
        let group = Rect::new(left, row_top, left + slot, row_top + box_h);
        groups[index + 1] = group;
        edits[index + 1] = edit_of(group, dpi);
    }

    FullLayout {
        preview,
        field,
        hue,
        groups,
        edits,
        copy,
    }
}

/// The edit rectangle inside a labelled group frame.
fn edit_of(group: Rect, dpi: u32) -> Rect {
    let side = FRAME_PAD.to_px(dpi).value().max(0);
    let top = FRAME_TOP.to_px(dpi).value().max(0);
    Rect::new(
        group.left + side,
        group.top + top,
        (group.right - side).max(group.left + side + 1),
        (group.bottom - side).max(group.top + top + 1),
    )
}
