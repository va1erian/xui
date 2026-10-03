//! Headless renders of the sample document, asserted on pixels (no golden
//! files). The PNGs land in `target/snapshots/` for a human to look at.

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::image::Image;
use xui_core::{Dip, Rect, Theme};
use xui_rich_text::layout::sample::sample_document;
use xui_rich_text::{DocPos, RichTextEditor};

struct Host {
    _editor: RichTextEditor<()>,
}

impl App for Host {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

const WIDTH: f32 = 640.0;
const HEIGHT: f32 = 760.0;

fn render(theme: Theme, dpi: u32, setup: impl FnOnce(&RichTextEditor<()>)) -> Image {
    render_sized(theme, dpi, HEIGHT, setup)
}

fn render_sized(
    theme: Theme,
    dpi: u32,
    height: f32,
    setup: impl FnOnce(&RichTextEditor<()>),
) -> Image {
    let scale = dpi as f32 / 96.0;
    try_render(
        Snapshot::new(Dip(WIDTH), Dip(height)).theme(theme).dpi(dpi),
        |ui| {
            let editor = RichTextEditor::new(
                ui,
                Rect::new(0, 0, (WIDTH * scale) as i32, (height * scale) as i32),
            )?
            .document(sample_document());
            setup(&editor);
            Ok(Host { _editor: editor })
        },
    )
    .expect("render")
}

fn save(name: &str, image: &Image) {
    let dir = std::path::Path::new("../../target/snapshots");
    std::fs::create_dir_all(dir).expect("snapshot directory");
    image.save_png(dir.join(name)).expect("save");
}

/// How many pixels in `(x0..x1, y0..y1)` satisfy `test`.
fn count(image: &Image, xs: (u32, u32), ys: (u32, u32), test: impl Fn([u8; 4]) -> bool) -> usize {
    (ys.0..ys.1)
        .flat_map(|y| (xs.0..xs.1).map(move |x| (x, y)))
        .filter(|&(x, y)| image.pixel(x, y).is_some_and(&test))
        .count()
}

fn blueish([r, _, b, _]: [u8; 4]) -> bool {
    b > 140 && r < 110
}

fn orangeish([r, g, b, _]: [u8; 4]) -> bool {
    r > 190 && (110..200).contains(&g) && b < 100
}

#[test]
fn the_sample_renders_in_light_and_dark() {
    let light = render(Theme::light(), 96, |_| {});
    let dark = render(Theme::dark(), 96, |_| {});
    save("rich-text-light.png", &light);
    save("rich-text-dark.png", &dark);
    assert_eq!(light.size(), (640, 760));

    let (light_bg, dark_bg) = (
        light.pixel(300, 740).unwrap(),
        dark.pixel(300, 740).unwrap(),
    );
    assert_ne!(light_bg, dark_bg, "the page follows the theme");
    assert!(light_bg[0] > 200 && dark_bg[0] < 80);

    // Text is dark on light and light on dark.
    let ink = |image: &Image, bg: [u8; 4]| {
        count(image, (8, 600), (10, 40), |p| p[0].abs_diff(bg[0]) > 120)
    };
    assert!(
        ink(&light, light_bg) > 200,
        "the heading painted in the light theme"
    );
    assert!(ink(&dark, dark_bg) > 200, "and in the dark theme");
}

#[test]
fn floats_paint_where_the_text_leaves_room() {
    let image = render(Theme::light(), 96, |_| {});
    // The blue picture is 120 x 90 at the left margin, just below the heading.
    assert!(
        count(&image, (8, 128), (48, 140), blueish) > 3_000,
        "left float"
    );
    // The orange one is 100 x 130 against the right edge of the text area.
    assert!(
        count(&image, (508, 616), (170, 310), orangeish) > 5_000,
        "right float"
    );
    // No text in the column the left float holds: only the picture and the
    // background are there, so no dark text pixels sit beside it at the left
    // margin below the picture.
    let right_of_float = count(&image, (136, 400), (52, 100), |p| p[0] < 90 && p[2] < 90);
    assert!(
        right_of_float > 100,
        "text flows to the right of the left float"
    );
    let under_float = count(&image, (8, 128), (176, 200), |p| p[0] < 90 && p[2] < 90);
    assert!(
        under_float > 50,
        "text returns to the left margin below the float"
    );
}

#[test]
fn a_selection_paints_behind_the_text() {
    let plain = render(Theme::light(), 96, |_| {});
    let selected = render(Theme::light(), 96, |editor| {
        editor.set_selection(Some((DocPos::new(3, 0), DocPos::new(3, 60))));
    });
    save("rich-text-selection.png", &selected);
    let theme = Theme::light().selection;
    let is_selection = |p: [u8; 4]| p[..3] == [theme.r, theme.g, theme.b];
    assert_eq!(count(&plain, (0, 640), (0, 760), is_selection), 0);
    assert!(count(&selected, (0, 640), (0, 760), is_selection) > 500);
}

#[test]
fn scrolling_moves_the_content() {
    // A short window, so the document overflows and can scroll.
    let top = render_sized(Theme::light(), 96, 300.0, |_| {});
    let scrolled = render_sized(Theme::light(), 96, 300.0, |editor| editor.set_scroll(60.0));
    assert_ne!(top.pixels(), scrolled.pixels());
    // Every row moved up by the scroll offset.
    for row in [10, 40, 120] {
        for x in 0..620 {
            assert_eq!(
                scrolled.pixel(x, row),
                top.pixel(x, row + 60),
                "({x}, {row})"
            );
        }
    }
    // The scrollbar thumb shows because the content overflows.
    let track = top.pixel(634, 2).unwrap();
    assert!(
        count(&top, (628, 640), (0, 300), |p| p != track) > 100,
        "a thumb"
    );
}

#[test]
fn a_high_dpi_render_is_larger_and_still_paints_text() {
    let image = render(Theme::dark(), 192, |_| {});
    assert_eq!(image.size(), (1280, 1520));
    save("rich-text-dark-192dpi.png", &image);
    let bg = image.pixel(600, 1500).unwrap();
    assert!(count(&image, (16, 1200), (20, 80), |p| p[0].abs_diff(bg[0]) > 120) > 800);
}

#[test]
fn clicking_the_scrollbar_track_pages_the_document() {
    use xui_canvas::snapshot::render_with;

    let at_top = render_sized(Theme::light(), 96, 300.0, |_| {});
    let paged = render_with(
        Snapshot::new(Dip(WIDTH), Dip(300.0)).theme(Theme::light()),
        |ui| {
            let editor = RichTextEditor::new(ui, Rect::new(0, 0, WIDTH as i32, 300))?
                .document(sample_document());
            Ok(Host { _editor: editor })
        },
        |stage| {
            // The first paint lays the document out; the second sees the bar.
            stage.hover(10, 10);
            stage.click(634, 290);
        },
    )
    .expect("render");
    assert_ne!(
        at_top.pixels(),
        paged.pixels(),
        "the click scrolled the page"
    );
    save("rich-text-paged.png", &paged);
}
