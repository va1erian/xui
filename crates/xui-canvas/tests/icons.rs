//! Every generated Lucide icon renders offscreen: the software canvas paints
//! each variant without panicking and leaves a mark, at two sizes and DPI
//! scales (a 16 and a 20 device-pixel box, at 96 and 144 DPI).

use xui_canvas::Surface;
use xui_core::icon::{Lucide, draw_icon};
use xui_core::{Color, Rect};

/// Whether `icon` leaves at least one non-transparent pixel in a `side` box.
fn paints(icon: Lucide, side: i32, dpi: u32) -> bool {
    let bounds = Rect::new(0, 0, side, side);
    let mut surface = Surface::new(side as u32, side as u32);
    surface.with_canvas_at(bounds, dpi, |canvas| {
        draw_icon(canvas, icon, bounds, Color::hex(0xFF_FF_FF), dpi);
    });
    surface
        .to_image()
        .pixels
        .chunks(4)
        .any(|pixel| pixel[3] > 0 && (pixel[0] | pixel[1] | pixel[2]) != 0)
}

#[test]
fn every_lucide_variant_draws_offscreen() {
    for &icon in Lucide::ALL {
        for side in [16, 20] {
            for dpi in [96, 144] {
                assert!(
                    paints(icon, side, dpi),
                    "{icon:?} painted nothing in a {side}px box at {dpi} dpi"
                );
            }
        }
    }
}

#[test]
fn a_too_small_box_draws_nothing_rather_than_panicking() {
    assert!(!paints(Lucide::Save, 2, 96));
}
