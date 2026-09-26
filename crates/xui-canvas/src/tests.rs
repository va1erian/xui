#![forbid(unsafe_code)]

use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, NodeKind, NodeSpec, ParentRef, PlatformSpec};
use xui_core::geometry::{Point, Rect};
use xui_core::image::Image;
use xui_core::widget::{Button, CheckBox, Label, ProgressBar, Slider};
use xui_core::{
    Canvas, Color, Corner, Dash, Dip, GradientStop, LinearGradient, RadialGradient, Rgba, Stroke,
    TextStyle,
};

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
pub(crate) fn save(path: &str, image: &RgbaImage) {
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

pub(crate) fn dark_pixels(image: &RgbaImage) -> usize {
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
fn an_alpha_fill_blends_with_the_background() {
    let mut surface = Surface::new(64, 64);
    surface.fill(Color::rgb(255, 255, 255));
    surface.with_canvas(Rect::new(0, 0, 64, 64), |canvas| {
        canvas.fill_rect_rgba(Rect::new(0, 0, 64, 64), Rgba::with_alpha(255, 0, 0, 128));
    });

    let image = surface.to_image();
    let [r, g, b, a] = image.pixel(32, 32).unwrap();
    assert_eq!((r, a), (255, 255), "red stays red and opaque: {image:?}");
    for (channel, value) in [("green", g), ("blue", b)] {
        assert!(
            (100..=170).contains(&value),
            "the white background lightened the {channel} channel: {value}"
        );
    }
}

#[test]
fn a_rounded_corner_is_clipped() {
    let mut surface = Surface::new(64, 64);
    surface.fill(Color::rgb(0, 0, 0));
    surface.with_canvas(Rect::new(0, 0, 64, 64), |canvas| {
        canvas.fill_rounded_rect_corners(
            Rect::new(16, 16, 48, 48),
            [Corner::uniform(12.0); 4],
            Rgba::rgb(255, 0, 0),
        );
    });

    let image = surface.to_image();
    assert_eq!(image.pixel(32, 32), Some([255, 0, 0, 255]), "centre filled");
    assert_eq!(
        image.pixel(17, 17),
        Some([0, 0, 0, 255]),
        "the corner is clipped away"
    );
}

#[test]
fn a_rounded_clip_rounds_off_its_corners() {
    let mut surface = Surface::new(64, 64);
    surface.fill(Color::rgb(0, 0, 0));
    surface.with_canvas(Rect::new(0, 0, 64, 64), |canvas| {
        canvas.push_clip_rounded(Rect::new(8, 8, 56, 56), [Corner::uniform(16.0); 4]);
        canvas.fill_rect(Rect::new(0, 0, 64, 64), Color::rgb(0, 255, 0));
        canvas.pop_clip();
    });

    let image = surface.to_image();
    assert_eq!(image.pixel(32, 32), Some([0, 255, 0, 255]), "inside clip");
    assert_eq!(
        image.pixel(9, 9),
        Some([0, 0, 0, 255]),
        "the rounded corner stays clear"
    );
}

#[test]
fn a_dashed_line_leaves_gaps() {
    let mut surface = Surface::new(64, 64);
    surface.fill(Color::rgb(0, 0, 0));
    surface.with_canvas(Rect::new(0, 0, 64, 64), |canvas| {
        canvas.draw_line_stroked(
            Point::new(2, 32),
            Point::new(62, 32),
            Rgba::rgb(255, 255, 255),
            &Stroke::new(2.0).dash(Dash::Dashed),
        );
    });

    let image = surface.to_image();
    let lit = (2..62)
        .filter(|x| image.pixel(*x, 32).unwrap()[0] > 128)
        .count();
    let dark = (2..62)
        .filter(|x| image.pixel(*x, 32).unwrap()[0] <= 128)
        .count();
    assert!(
        lit > 0 && dark > 0,
        "a dashed line has both dashes ({lit}) and gaps ({dark})"
    );
}

#[test]
fn a_two_stop_gradient_varies_across_the_rect() {
    let mut surface = Surface::new(64, 64);
    surface.fill(Color::rgb(255, 0, 255));
    surface.with_canvas(Rect::new(0, 0, 64, 64), |canvas| {
        canvas.fill_rect_linear(
            Rect::new(0, 0, 64, 64),
            &LinearGradient::new(
                Point::new(0, 0),
                Point::new(64, 0),
                vec![
                    GradientStop::new(0.0, Rgba::rgb(0, 0, 0)),
                    GradientStop::new(1.0, Rgba::rgb(255, 255, 255)),
                ],
            ),
        );
    });

    let image = surface.to_image();
    let left = image.pixel(4, 32).unwrap()[0];
    let right = image.pixel(60, 32).unwrap()[0];
    assert!(
        left < right,
        "the gradient runs dark to light: {left} vs {right}"
    );
}

#[test]
fn a_radial_gradient_fades_from_the_centre() {
    let mut surface = Surface::new(64, 64);
    surface.fill(Color::rgb(0, 0, 0));
    surface.with_canvas(Rect::new(0, 0, 64, 64), |canvas| {
        canvas.fill_rect_radial(
            Rect::new(0, 0, 64, 64),
            &RadialGradient::new(
                Point::new(32, 32),
                24.0,
                24.0,
                vec![
                    GradientStop::new(0.0, Rgba::rgb(255, 255, 255)),
                    GradientStop::new(1.0, Rgba::rgb(0, 0, 0)),
                ],
            ),
        );
    });

    let image = surface.to_image();
    let centre = image.pixel(32, 32).unwrap()[0];
    let edge = image.pixel(9, 9).unwrap()[0];
    assert!(
        centre > edge,
        "the radial gradient is bright at the centre: {centre} vs {edge}"
    );
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
fn a_repeat_draw_reuses_the_cached_upload() {
    // A 4x4 checkerboard, so a scaled blit has real pixels to land.
    let mut pixels = Vec::new();
    for y in 0..4 {
        for x in 0..4 {
            let channel = if (x + y) % 2 == 0 { 0xF0 } else { 0x10 };
            pixels.extend_from_slice(&[channel, channel, channel, 255]);
        }
    }
    let image = Image::from_rgba(4, 4, pixels).expect("image");

    // Repaint the same surface twice: the second draw must come from the
    // cached upload and land exactly the same pixels.
    let mut surface = Surface::new(16, 16);
    let paint = |surface: &mut Surface| {
        surface.fill(Color::rgb(0, 0, 0));
        surface.with_canvas(Rect::new(0, 0, 16, 16), |canvas| {
            canvas.draw_image(&image, Rect::new(0, 0, 16, 16));
        });
        surface.to_image()
    };
    let first = paint(&mut surface);
    let second = paint(&mut surface);

    assert!(first.pixel(0, 0).is_some_and(|p| p[0] > 0xC0));
    assert_eq!(
        first.pixels, second.pixels,
        "the cached draw must land the same pixels"
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
fn a_design_length_is_converted_once_at_each_dpi() {
    let backend = OffscreenBackend::new();
    // A 10 DIP square is 10px at 100% and 20px at 200%. If the boundary scaled
    // it twice it would cover 40px at 200%, so probing 25px catches that.
    for (dpi, covered, outside) in [(96u32, 5u32, 15u32), (192u32, 15u32, 25u32)] {
        let window = backend
            .open_window_at(&PlatformSpec::new("dpi"), dpi)
            .unwrap();
        assert_eq!(backend.dpi(window), dpi, "the window renders at {dpi}");

        let node = backend
            .create(
                ParentRef::Window(window),
                &NodeSpec::new(NodeKind::Custom, backend.client_rect(window)),
            )
            .unwrap();
        backend.set_painter(
            node,
            Rc::new(move |canvas| {
                let side = Dip(10.0).to_px(canvas.dpi()).value();
                canvas.fill_rect(Rect::new(0, 0, side, side), Color::rgb(255, 0, 0));
            }),
        );

        let image = backend.render(window).expect("a rendered window");
        assert_eq!(
            image.pixel(covered, covered),
            Some([255, 0, 0, 255]),
            "a 10 DIP square covers {covered}px at {dpi} DPI"
        );
        assert_ne!(
            image.pixel(outside, outside),
            Some([255, 0, 0, 255]),
            "and not {outside}px, so the DIP was scaled exactly once"
        );
    }
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
