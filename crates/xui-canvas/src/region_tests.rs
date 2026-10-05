#![forbid(unsafe_code)]

//! [`Surface::paint_region`]: a region repaint is pixel-identical to a whole
//! surface paint inside the region and leaves every other pixel alone.

use xui_core::backend::{
    Canvas, Corner, Dash, GradientStop, LinearGradient, PathPlacement, PathSeg, Rgba, Stroke,
};
use xui_core::geometry::{Point, Rect};
use xui_core::image::Image;
use xui_core::{Color, Dip, TextStyle};

use crate::Surface;

const W: u32 = 160;
const H: u32 = 120;
const PAPER: Color = Color::rgb(250, 250, 250);

/// A checkerboard image, so a scaled or cropped draw shows any misplacement.
fn checker(size: u32) -> Image {
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let on = (x / 2 + y / 2) % 2 == 0;
            pixels.extend_from_slice(if on {
                &[200, 30, 30, 255]
            } else {
                &[30, 30, 200, 160]
            });
        }
    }
    Image::from_rgba(size, size, pixels).expect("image")
}

/// What most widgets draw, crossing every region edge: axis-aligned fills
/// and strokes, a dashed rule, a gradient, text, images scaled and unscaled,
/// a clip and a translation and scale.
fn widgets(canvas: &mut dyn Canvas, image: &Image) {
    canvas.clear(Color::rgb(230, 235, 240));
    canvas.fill_rect_rgba(Rect::new(7, 9, 93, 61), Rgba::with_alpha(40, 120, 200, 200));
    canvas.stroke_rect(Rect::new(11, 13, 151, 107), Color::rgb(20, 20, 20), 1.0);
    canvas.draw_line_stroked(
        Point::new(5, 40),
        Point::new(155, 40),
        Rgba::with_alpha(255, 0, 0, 140),
        &Stroke::new(2.0).dash(Dash::Dashed),
    );
    canvas.fill_rect_linear(
        Rect::new(100, 70, 155, 115),
        &LinearGradient::new(
            Point::new(100, 70),
            Point::new(155, 115),
            vec![
                GradientStop::new(0.0, Rgba::rgb(255, 255, 0)),
                GradientStop::new(1.0, Rgba::with_alpha(0, 0, 255, 128)),
            ],
        ),
    );
    canvas.draw_text(
        "Region text Ag",
        Rect::new(18, 64, 150, 90),
        &TextStyle::new(Color::rgb(0, 0, 0), Dip(13.0)),
    );
    canvas.draw_image(image, Rect::new(66, 3, 82, 19));
    canvas.draw_image(image, Rect::new(84, 2, 113, 31));
    canvas.push_clip(Rect::new(110, 5, 150, 45));
    canvas.stroke_rect(Rect::new(100, 0, 140, 30), Color::rgb(0, 90, 0), 2.0);
    canvas.pop_clip();
    canvas.save();
    canvas.set_scale_translate(1.5, 4.0, 2.0);
    canvas.stroke_rect(Rect::new(60, 40, 90, 70), Color::rgb(60, 60, 60), 2.0);
    canvas.restore();
}

/// Slanted and curved edges crossing region edges: lines, a polygon, a
/// path, rounded rectangles, ellipses and a rounded clip.
fn slanted(canvas: &mut dyn Canvas, _image: &Image) {
    canvas.clear(Color::rgb(230, 235, 240));
    canvas.fill_rounded_rect(Rect::new(7, 9, 93, 61), 9.5, Color::rgb(40, 120, 200));
    canvas.stroke_rounded_rect(
        Rect::new(30, 25, 140, 100),
        12.0,
        Color::rgb(200, 90, 0),
        3.0,
    );
    canvas.fill_ellipse(Point::new(97, 53), 31.3, 17.7, Color::rgb(0, 160, 90));
    canvas.stroke_ellipse(Point::new(60, 80), 22.0, 22.0, Color::rgb(90, 0, 140), 2.5);
    canvas.push_clip_rounded(Rect::new(20, 20, 80, 60), [Corner::uniform(8.0); 4]);
    canvas.fill_rect_rgba(
        Rect::new(0, 0, 160, 120),
        Rgba::with_alpha(255, 255, 255, 90),
    );
    canvas.fill_ellipse(Point::new(20, 20), 25.0, 25.0, Color::rgb(255, 128, 0));
    canvas.pop_clip();
    canvas.save();
    canvas.set_scale_translate(1.5, 4.0, 2.0);
    canvas.stroke_rounded_rect(Rect::new(60, 40, 90, 70), 4.0, Color::rgb(60, 60, 60), 1.0);
    canvas.restore();
    canvas.draw_line(
        Point::new(3, 117),
        Point::new(157, 4),
        Color::rgb(0, 0, 0),
        1.5,
    );
    canvas.draw_line_stroked(
        Point::new(5, 40),
        Point::new(155, 44),
        Rgba::with_alpha(255, 0, 0, 140),
        &Stroke::new(2.0).dash(Dash::Dashed),
    );
    canvas.fill_polygon(
        &[Point::new(15, 100), Point::new(45, 66), Point::new(70, 112)],
        Color::rgb(220, 0, 120),
    );
    let star = [
        PathSeg::MoveTo(0.0, -10.0),
        PathSeg::CubicTo(4.0, -2.0, 10.0, 0.0, 6.0, 8.0),
        PathSeg::LineTo(-6.0, 8.0),
        PathSeg::Close,
    ];
    canvas.fill_path(
        &star,
        PathPlacement::new(1.7, 121.3, 30.6),
        Rgba::rgb(10, 10, 10),
    );
}

type Scene = fn(&mut dyn Canvas, &Image);

/// `scene` painted over the whole surface.
fn full(scene: Scene, image: &Image) -> Vec<u8> {
    let mut surface = Surface::new(W, H);
    surface.fill(PAPER);
    surface.with_canvas_at(Rect::new(0, 0, W as i32, H as i32), 96, |c| scene(c, image));
    surface.pixels().to_vec()
}

/// Repaints `scene` over a set of regions, each on a fresh surface, and
/// checks every pixel: inside the region within `tolerance` of a whole-surface
/// paint, outside it untouched.
fn check_regions(scene: Scene, tolerance: u8) {
    let image = checker(16);
    let reference = full(scene, &image);
    for region in [
        Rect::new(0, 0, W as i32, H as i32),
        Rect::new(13, 17, 58, 49),
        Rect::new(29, 24, 31, 101),
        Rect::new(96, 50, 141, 77),
        Rect::new(70, 0, 100, 25),
        Rect::new(-20, 90, 40, 200),
        Rect::new(150, 110, 400, 400),
    ] {
        let mut surface = Surface::new(W, H);
        surface.fill(PAPER);
        surface.paint_region(region, |r| {
            r.with_canvas_at(Rect::new(0, 0, W as i32, H as i32), 96, |c| {
                scene(c, &image)
            });
        });
        let pixels = surface.pixels();
        for y in 0..H as i32 {
            for x in 0..W as i32 {
                let at = (y as usize * W as usize + x as usize) * 4;
                let inside =
                    x >= region.left && x < region.right && y >= region.top && y < region.bottom;
                let want = if inside {
                    &reference[at..at + 4]
                } else {
                    &[PAPER.r, PAPER.g, PAPER.b, 255][..]
                };
                let got = &pixels[at..at + 4];
                let off = got.iter().zip(want).map(|(a, b)| a.abs_diff(*b)).max();
                assert!(
                    off <= Some(tolerance),
                    "{region:?} at ({x}, {y}): {got:?}, want {want:?}"
                );
            }
        }
    }
}

#[test]
fn a_region_repaint_of_widgets_is_pixel_identical_inside_and_keeps_the_rest() {
    check_regions(widgets, 0);
}

/// tiny-skia clips a slanted or curved edge that leaves the pixmap by
/// re-sloping it from the clip point, so an anti-aliased pixel on such an edge
/// can differ by a step or two of its 4x4 supersampling (16 levels of 255 a
/// step, times the colour contrast), as it does in tiny-skia's own tiled
/// drawing.
#[test]
fn a_region_repaint_of_slanted_edges_stays_within_two_coverage_steps() {
    check_regions(slanted, 32);
}

#[test]
fn a_region_blends_over_the_surface_it_starts_from() {
    let mut surface = Surface::new(8, 8);
    surface.fill(Color::rgb(0, 0, 200));
    surface.paint_region(Rect::new(2, 2, 6, 6), |r| {
        assert_eq!(r.area(), Rect::new(2, 2, 6, 6));
        r.with_canvas_over_parents(Rect::new(0, 0, 8, 8), 96, |c| {
            assert!(c.composites_parents());
            c.fill_rect_rgba(Rect::new(0, 0, 8, 8), Rgba::with_alpha(200, 0, 0, 128));
        });
    });
    let image = surface.to_image();
    let [r, _, b, _] = image.pixel(3, 3).expect("in bounds");
    assert!(r > 0 && b > 0, "blended over the surface's blue: {r} {b}");
    assert_eq!(image.pixel(1, 1), Some([0, 0, 200, 255]));
}

#[test]
fn a_region_off_the_surface_runs_and_changes_nothing() {
    let mut surface = Surface::new(8, 8);
    surface.fill(PAPER);
    let ran = surface.paint_region(Rect::new(20, 20, 30, 30), |r| {
        assert!(r.area().is_empty());
        r.with_canvas_at(Rect::new(0, 0, 8, 8), 96, |c| c.clear(Color::rgb(0, 0, 0)));
        true
    });
    assert!(ran);
    assert!(
        surface
            .pixels()
            .chunks(4)
            .all(|p| p == [PAPER.r, PAPER.g, PAPER.b, 255])
    );
}
