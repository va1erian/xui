//! Headless renders of the sample document, asserted on pixels. The PNGs land
//! in `target/snapshots/` for a human to look at.

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::image::Image;
use xui_core::{Color, Dip, Rect, Theme};
use xui_markdown::MarkdownView;

const SAMPLE: &str = include_str!("sample.md");
const WIDTH: f32 = 640.0;
const HEIGHT: f32 = 900.0;

struct Host {
    _view: MarkdownView<()>,
}

impl App for Host {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

fn render(theme: Theme, setup: impl FnOnce(&MarkdownView<()>)) -> Image {
    render_md(theme, SAMPLE, setup)
}

fn render_md(theme: Theme, markdown: &str, setup: impl FnOnce(&MarkdownView<()>)) -> Image {
    try_render(Snapshot::new(Dip(WIDTH), Dip(HEIGHT)).theme(theme), |ui| {
        let view =
            MarkdownView::new(ui, Rect::new(0, 0, WIDTH as i32, HEIGHT as i32))?.markdown(markdown);
        setup(&view);
        Ok(Host { _view: view })
    })
    .expect("render")
}

fn save(name: &str, image: &Image) {
    let dir = std::path::Path::new("../../target/snapshots");
    std::fs::create_dir_all(dir).expect("snapshot directory");
    image.save_png(dir.join(name)).expect("save");
}

#[test]
fn the_sample_renders_in_light_and_dark() {
    let light = render(Theme::light(), |_| {});
    let dark = render(Theme::dark(), |_| {});
    save("markdown-light.png", &light);
    save("markdown-dark.png", &dark);
}

fn rgb(color: Color) -> [u8; 3] {
    [color.r, color.g, color.b]
}

fn at(image: &Image, x: u32, y: u32) -> [u8; 3] {
    let [r, g, b, _] = image.pixel(x, y).expect("inside the image");
    [r, g, b]
}

/// How many pixels in `(x0..x1, y0..y1)` satisfy `test`.
fn count(image: &Image, xs: (u32, u32), ys: (u32, u32), test: impl Fn([u8; 3]) -> bool) -> usize {
    (ys.0..ys.1)
        .flat_map(|y| (xs.0..xs.1).map(move |x| (x, y)))
        .filter(|&(x, y)| test(at(image, x, y)))
        .count()
}

fn distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.iter().zip(b).map(|(a, b)| u32::from(a.abs_diff(b))).sum()
}

#[test]
fn the_page_background_follows_the_theme() {
    let (light, dark) = (Theme::light(), Theme::dark());
    let (l, d) = (render(light, |_| {}), render(dark, |_| {}));
    // The right margin, below the heading, holds nothing but the page.
    assert_eq!(at(&l, WIDTH as u32 - 40, 4), rgb(light.input_background));
    assert_eq!(at(&d, WIDTH as u32 - 40, 4), rgb(dark.input_background));
    assert_ne!(at(&l, 600, 4), at(&d, 600, 4));
}

#[test]
fn the_heading_is_inked_in_the_text_colour() {
    for theme in [Theme::light(), Theme::dark()] {
        let image = render(theme, |_| {});
        let ink = count(&image, (16, 320), (0, 60), |p| {
            distance(p, rgb(theme.input_background)) > 200
        });
        assert!(ink > 100, "{ink} inked pixels in the heading");
        let text = count(&image, (16, 320), (0, 60), |p| {
            distance(p, rgb(theme.text)) < 30
        });
        assert!(text > 50, "{text} pixels in the text colour");
    }
}

#[test]
fn links_are_drawn_in_the_accent_colour() {
    for theme in [Theme::light(), Theme::dark()] {
        let image = render(theme, |_| {});
        let accent = count(&image, (0, WIDTH as u32 - 20), (0, 200), |p| {
            p == rgb(theme.accent)
        });
        assert!(accent > 40, "{accent} accent pixels (the underlines)");
        let plain = render_md(theme, "no links, just **bold** words", |_| {});
        let none = count(&plain, (0, WIDTH as u32 - 20), (0, 200), |p| {
            p == rgb(theme.accent)
        });
        assert_eq!(none, 0);
    }
}

#[test]
fn a_code_block_is_filled_with_the_surface_colour() {
    for theme in [Theme::light(), Theme::dark()] {
        assert_ne!(theme.surface, theme.input_background, "the fill must show");
        let image = render_md(
            theme,
            "```
fn main() {}
```",
            |_| {},
        );
        // Well right of the code, inside the box and clear of its edges.
        assert_eq!(at(&image, 500, 20), rgb(theme.surface));
        assert_eq!(at(&image, 500, 200), rgb(theme.input_background));
        let sample = render(theme, |_| {});
        let filled = count(&sample, (0, WIDTH as u32 - 20), (0, HEIGHT as u32), |p| {
            p == rgb(theme.surface)
        });
        assert!(filled > 3000, "{filled} surface pixels in the sample");
    }
}

#[test]
fn rules_and_table_grids_are_drawn_in_the_border_colour() {
    for theme in [Theme::light(), Theme::dark()] {
        let image = render(theme, |_| {});
        let border = count(&image, (0, WIDTH as u32 - 20), (0, HEIGHT as u32), |p| {
            p == rgb(theme.border)
        });
        assert!(border > 300, "{border} border pixels");
    }
}

#[test]
fn light_and_dark_differ_throughout() {
    let l = render(Theme::light(), |_| {});
    let d = render(Theme::dark(), |_| {});
    let changed = (0..HEIGHT as u32)
        .step_by(7)
        .flat_map(|y| (0..WIDTH as u32).step_by(7).map(move |x| (x, y)))
        .filter(|&(x, y)| at(&l, x, y) != at(&d, x, y))
        .count();
    assert!(changed > 1000, "{changed} sampled pixels differ");
}

#[test]
fn a_view_built_at_no_size_draws_once_it_is_given_one() {
    // A hidden view (zero bounds) has nothing to lay out for; once sized it
    // must lay its document out and paint it like any other.
    let image = try_render(
        Snapshot::new(Dip(WIDTH), Dip(HEIGHT)).theme(Theme::light()),
        |ui| {
            let view = MarkdownView::new(ui, Rect::default())?.markdown(SAMPLE);
            view.set_bounds(Rect::new(0, 0, WIDTH as i32, HEIGHT as i32));
            Ok(Host { _view: view })
        },
    )
    .expect("render");
    let sized = render(Theme::light(), |_| {});
    assert_eq!(image.pixel(40, 30), sized.pixel(40, 30));
    let ink = (0..WIDTH as u32)
        .flat_map(|x| (0..60).map(move |y| (x, y)))
        .filter(|&(x, y)| image.pixel(x, y) != sized.pixel(WIDTH as u32 - 1, HEIGHT as u32 - 1))
        .count();
    assert!(ink > 200, "the heading painted once sized ({ink} pixels)");
}
