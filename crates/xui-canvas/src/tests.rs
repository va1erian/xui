use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec, TextAlign};
use xui_core::geometry::{Point, Rect};
use xui_core::image::Image;
use xui_core::widget::{Button, CheckBox, Label, ProgressBar, Slider};
use xui_core::{Canvas, Color, Dip, TextStyle};

use crate::{OffscreenBackend, RgbaImage, Surface, measure_text, to_skia};

/// Keeps the widgets alive for the duration of a `run_app` call.
struct Widgets {
    _button: Button<u32>,
    _check: CheckBox<u32>,
    _bar: ProgressBar<u32>,
    _slider: Slider<u32>,
    _label: Label<u32>,
}

impl App for Widgets {
    type Msg = u32;
    fn update(&mut self, _msg: u32, _ui: &mut Ui<u32>) {}
}

fn contains(image: &RgbaImage, color: [u8; 3]) -> bool {
    image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| pixel[..3] == color)
}

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

/// The text and rect the alignment tests render into.
const ALIGN_TEXT: &str = "Alignment";
const ALIGN_RECT: Rect = Rect::new(20, 20, 300, 60);

/// Renders `ALIGN_TEXT` in `ALIGN_RECT` with `align`, on a white surface.
fn render_alignment(align: TextAlign) -> RgbaImage {
    let style = TextStyle {
        align,
        ..TextStyle::new(Color::hex(0x1B_1B_1B), Dip(20.0))
    };
    let mut surface = Surface::new(320, 80);
    surface.fill(Color::hex(0xFFFFFF));
    surface.with_canvas(Rect::new(0, 0, 320, 80), |canvas| {
        canvas.draw_text(ALIGN_TEXT, ALIGN_RECT, &style);
    });
    surface.to_image()
}

/// The leftmost pixel column containing dark (text) pixels.
fn leftmost_dark(image: &RgbaImage) -> i32 {
    for x in 0..image.width {
        for y in 0..image.height {
            if image.pixel(x, y).is_some_and(|pixel| pixel[0] < 128) {
                return x as i32;
            }
        }
    }
    panic!("no dark text pixels");
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
fn a_drawn_image_lands_on_the_surface() {
    let image = Image::from_rgba(1, 1, vec![255, 0, 0, 255]).expect("image");

    let mut surface = Surface::new(32, 32);
    surface.fill(Color::rgb(0, 0, 0));
    surface.with_canvas(Rect::new(0, 0, 32, 32), |canvas| {
        canvas.draw_image(&image, Rect::new(8, 8, 24, 24));
    });

    let rendered = surface.to_image();
    assert_eq!(
        rendered.pixel(16, 16),
        Some([255, 0, 0, 255]),
        "image lands"
    );
    assert_eq!(
        rendered.pixel(2, 2),
        Some([0, 0, 0, 255]),
        "outside stays clear"
    );
}

#[test]
fn portable_widgets_render_on_the_software_backend() {
    let backend = Rc::new(OffscreenBackend::new());
    let backend_for_run: Rc<dyn Backend> = backend.clone();
    let accent = Color::hex(0x00_5F_B8);
    let mut captured: Option<RgbaImage> = None;

    let _ = run_app(
        backend_for_run,
        PlatformSpec::new("canvas widgets").size(Dip(420.0), Dip(200.0)),
        |ui| {
            let button = Button::new(ui, Rect::new(20, 20, 180, 52), "Click").unwrap();
            let check = CheckBox::new(ui, Rect::new(20, 64, 240, 92), "Enabled").unwrap();
            check.set_checked(true);
            let bar = ProgressBar::new(ui, Rect::new(20, 104, 380, 112), 100).unwrap();
            bar.set_value(60);
            let slider = Slider::new(ui, Rect::new(20, 128, 380, 156), 0.0, 100.0).unwrap();
            slider.set_value(40.0);
            let label = Label::new(ui, Rect::new(20, 168, 380, 196), "xui on tiny-skia").unwrap();

            let image = backend.render(ui.window()).expect("a rendered window");
            assert!(
                contains(&image, [accent.r, accent.g, accent.b]),
                "the accent colour is on the surface"
            );
            save("canvas-widgets.png", &image);
            captured = Some(image);

            Widgets {
                _button: button,
                _check: check,
                _bar: bar,
                _slider: slider,
                _label: label,
            }
        },
    );

    assert!(captured.is_some(), "the surface was rendered");
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

#[test]
fn start_aligned_text_sits_at_the_left_of_the_rect() {
    let left = leftmost_dark(&render_alignment(TextAlign::Start));
    assert!(
        (ALIGN_RECT.left..=ALIGN_RECT.left + 3).contains(&left),
        "start-aligned text should hug the left edge, got {left}"
    );
}

#[test]
fn center_aligned_text_is_centred_in_the_rect() {
    let sample = TextStyle::new(Color::hex(0x1B_1B_1B), Dip(20.0));
    let measured = measure_text(ALIGN_TEXT, &sample, 96, 1000).width;
    let start = leftmost_dark(&render_alignment(TextAlign::Start));
    let center = leftmost_dark(&render_alignment(TextAlign::Center));
    let expected = (ALIGN_RECT.width() - measured) / 2;
    assert!(
        (center - start - expected).abs() <= 2,
        "centre should shift the run by {expected}px: start {start}, centre {center}"
    );
}

#[test]
fn end_aligned_text_sits_against_the_right_edge() {
    let sample = TextStyle::new(Color::hex(0x1B_1B_1B), Dip(20.0));
    let measured = measure_text(ALIGN_TEXT, &sample, 96, 1000).width;
    let start = leftmost_dark(&render_alignment(TextAlign::Start));
    let end = leftmost_dark(&render_alignment(TextAlign::End));
    let expected = ALIGN_RECT.width() - measured;
    assert!(
        (end - start - expected).abs() <= 2,
        "end should shift the run by {expected}px: start {start}, end {end}"
    );
    assert!(
        end + measured >= ALIGN_RECT.right - 2,
        "end-aligned text should reach the right edge: end {end}, measured {measured}"
    );
}
