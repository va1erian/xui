use xui_core::geometry::{Point, Rect};
use xui_core::{Canvas, Color};

use crate::{Surface, to_skia};

#[test]
fn a_filled_rectangle_lands_on_the_surface() {
    let mut surface = Surface::new(64, 64);
    surface.fill(Color::rgb(255, 255, 255));
    surface.with_canvas(Rect::new(0, 0, 64, 64), |canvas| {
        canvas.fill_rect(Rect::new(16, 16, 48, 48), Color::rgb(255, 0, 0));
    });

    let image = surface.to_image();
    assert_eq!(image.pixel(32, 32), Some([255, 0, 0, 255]));
    assert_eq!(image.pixel(4, 4), Some([255, 255, 255, 255]));
}

#[test]
fn a_translated_rectangle_moves() {
    let mut surface = Surface::new(64, 64);
    surface.fill(Color::rgb(0, 0, 0));
    surface.with_canvas(Rect::new(0, 0, 64, 64), |canvas| {
        canvas.set_translation(0.0, -8.0);
        canvas.fill_rect(Rect::new(0, 8, 32, 24), Color::rgb(0, 255, 0));
    });

    let image = surface.to_image();
    assert_eq!(image.pixel(10, 8), Some([0, 255, 0, 255]));
}

#[test]
fn a_ellipse_does_not_fill_its_bounding_corners() {
    let mut surface = Surface::new(64, 64);
    surface.fill(Color::rgb(0, 0, 0));
    surface.with_canvas(Rect::new(0, 0, 64, 64), |canvas| {
        canvas.fill_ellipse(Point::new(32, 32), 24.0, 24.0, Color::rgb(0, 128, 255));
    });

    let image = surface.to_image();
    assert_eq!(
        image.pixel(32, 32),
        Some([0, 128, 255, 255]),
        "centre filled"
    );
    assert_eq!(
        image.pixel(9, 9),
        Some([0, 0, 0, 255]),
        "corner stays clear"
    );
}

#[test]
fn a_push_clip_limits_drawing() {
    let mut surface = Surface::new(64, 64);
    surface.fill(Color::rgb(0, 0, 0));
    surface.with_canvas(Rect::new(0, 0, 64, 64), |canvas| {
        canvas.push_clip(Rect::new(0, 0, 32, 64));
        canvas.fill_rect(Rect::new(0, 0, 64, 64), Color::rgb(255, 255, 0));
        canvas.pop_clip();
    });

    let image = surface.to_image();
    assert_eq!(image.pixel(16, 32), Some([255, 255, 0, 255]), "inside clip");
    assert_eq!(image.pixel(48, 32), Some([0, 0, 0, 255]), "outside clip");
}

#[test]
fn skia_colour_is_opaque() {
    let color = to_skia(Color::rgb(0x12, 0x34, 0x56));
    assert_eq!(color.alpha(), 1.0);
}
