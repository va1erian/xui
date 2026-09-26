use xui_core::geometry::{Point, Rect};
use xui_core::{Canvas, Color, Dip, TextStyle};

use crate::{RgbaImage, Surface, measure_text, to_skia};

/// Writes `image` to `path` as a PNG under `target/ui`.
fn save(path: &str, image: &RgbaImage) {
    let path = std::path::Path::new("target/ui").join(path);
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let Ok(file) = std::fs::File::create(&path) else {
        return;
    };
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    if let Ok(mut writer) = encoder.write_header() {
        let _ = writer.write_image_data(&image.pixels);
    }
}

fn dark_pixels(image: &RgbaImage) -> usize {
    image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] < 128)
        .count()
}

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

#[test]
fn text_rasterises_and_writes_a_png() {
    let mut surface = Surface::new(360, 96);
    surface.fill(Color::hex(0xF3F3F3));
    surface.with_canvas(Rect::new(0, 0, 360, 96), |canvas| {
        let style = TextStyle::new(Color::hex(0x1B_1B_1B), Dip(24.0)).middle();
        canvas.draw_text("Hello xui", Rect::new(16, 16, 344, 80), &style);
    });

    let image = surface.to_image();
    assert!(
        dark_pixels(&image) > 100,
        "the text painted dark pixels: {}",
        dark_pixels(&image)
    );
    save("canvas-text.png", &image);
}

#[test]
fn text_scales_with_dpi() {
    let style = TextStyle::new(Color::hex(0x1B_1B_1B), Dip(16.0));
    let at_96 = measure_text("Hello xui", &style, 96, 1000).width;
    let at_192 = measure_text("Hello xui", &style, 192, 1000).width;
    assert!(at_96 > 0, "the text measured a width");
    assert!(
        at_192 > at_96,
        "200% text is wider than 100%: {at_96} vs {at_192}"
    );

    // A side-by-side capture for visual inspection.
    let mut surface = Surface::new(480, 128);
    surface.fill(Color::hex(0xF3F3F3));
    surface.with_canvas_at(Rect::new(0, 0, 480, 64), 96, |canvas| {
        canvas.draw_text("Hello 100%", Rect::new(12, 8, 468, 56), &style);
    });
    surface.with_canvas_at(Rect::new(0, 64, 480, 128), 192, |canvas| {
        canvas.draw_text("Hello 200%", Rect::new(12, 72, 468, 120), &style);
    });
    save("canvas-hidpi.png", &surface.to_image());
}
