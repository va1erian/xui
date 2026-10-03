//! The painter places the document at the canvas's origin: on the portable
//! software backend every node paints into one window surface, so a view's
//! canvas starts at its place in the window, not at (0, 0).

use xui_canvas::Surface;
use xui_core::Color;
use xui_core::geometry::Rect as PxRect;

use super::Painter;
use crate::geom::{Radius, Rect, Rgba};
use crate::list::{Cmd, DisplayList};
use crate::text::TextSystem;

const RED: Rgba = Rgba {
    r: 255,
    g: 0,
    b: 0,
    a: 255,
};

fn block(rect: Rect) -> DisplayList {
    DisplayList {
        cmds: vec![Cmd::Rect {
            rect,
            radii: [Radius::default(); 4],
            fill: RED,
        }],
        size: (rect.right, rect.bottom),
        fonts: Vec::new(),
        images: Vec::new(),
    }
}

/// Paints `list` into a canvas at `bounds` of a 640 x 400 surface and returns
/// the surface.
fn paint_at(list: &DisplayList, bounds: PxRect, dpi: u32, scroll: f32) -> Surface {
    let mut surface = Surface::new(640, 400);
    let mut painter = Painter::new(TextSystem::for_tests());
    let scale = dpi as f32 / 96.0;
    let viewport = Rect::new(
        0.0,
        0.0,
        bounds.width() as f32 / scale,
        bounds.height() as f32 / scale,
    );
    surface.with_canvas_at(bounds, dpi, |canvas| {
        painter.paint(list, canvas, viewport, scroll, Color::rgb(255, 255, 255));
    });
    surface
}

fn pixel(surface: &Surface, x: u32, y: u32) -> [u8; 4] {
    surface.to_image().pixel(x, y).expect("in the surface")
}

const RED_PX: [u8; 4] = [255, 0, 0, 255];
const WHITE_PX: [u8; 4] = [255, 255, 255, 255];
const CLEAR_PX: [u8; 4] = [0, 0, 0, 0];

#[test]
fn the_document_is_painted_at_the_canvas_origin() {
    let list = block(Rect::new(0.0, 0.0, 100.0, 50.0));
    let surface = paint_at(&list, PxRect::new(300, 100, 600, 300), 96, 0.0);
    // The block sits at the view's top-left corner...
    assert_eq!(pixel(&surface, 301, 101), RED_PX);
    assert_eq!(pixel(&surface, 399, 149), RED_PX);
    // ...and ends 100 x 50 pixels later, on the view's background.
    assert_eq!(pixel(&surface, 401, 120), WHITE_PX);
    assert_eq!(pixel(&surface, 320, 151), WHITE_PX);
    // Nothing is drawn at the window's origin, outside the view.
    assert_eq!(pixel(&surface, 10, 10), CLEAR_PX);
    assert_eq!(pixel(&surface, 299, 101), CLEAR_PX);
}

#[test]
fn scroll_and_scale_apply_from_the_canvas_origin() {
    // 40 DIPs scrolled off the top at 2x: the block's top 80 device pixels are
    // hidden and its rest starts at the view's top.
    let list = block(Rect::new(10.0, 0.0, 60.0, 100.0));
    let surface = paint_at(&list, PxRect::new(200, 50, 600, 350), 192, 40.0);
    assert_eq!(pixel(&surface, 221, 51), RED_PX);
    assert_eq!(pixel(&surface, 319, 50 + 119), RED_PX);
    assert_eq!(pixel(&surface, 219, 60), WHITE_PX);
    assert_eq!(pixel(&surface, 250, 50 + 121), WHITE_PX);
    // Clipped to the view: nothing above it.
    assert_eq!(pixel(&surface, 250, 49), CLEAR_PX);
}

#[test]
fn a_canvas_at_the_window_origin_is_unchanged() {
    let list = block(Rect::new(0.0, 0.0, 100.0, 50.0));
    let surface = paint_at(&list, PxRect::new(0, 0, 300, 200), 96, 0.0);
    assert_eq!(pixel(&surface, 0, 0), RED_PX);
    assert_eq!(pixel(&surface, 99, 49), RED_PX);
    assert_eq!(pixel(&surface, 100, 49), WHITE_PX);
    assert_eq!(pixel(&surface, 301, 10), CLEAR_PX);
}
