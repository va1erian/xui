//! Headless renders of highlighted code, asserted on pixels. The PNGs land in
//! `target/snapshots/` for a human to look at.

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::image::Image;
use xui_core::{Color, Dip, Rect, Theme};
use xui_markdown::{CodeClass, CodeColors, CodePalette, CodeSpan, Highlighter, MarkdownView};

const SOURCE: &str = "```rust\nfn main() {\n    let x = 1;\n}\n```";
const WIDTH: f32 = 400.0;
const HEIGHT: f32 = 200.0;

/// Marks every `fn` and `let` as a keyword.
struct Keywords;

impl Highlighter for Keywords {
    fn line(&self, _lang: &str, line: &str, _state: u32) -> (Vec<CodeSpan>, u32) {
        let spans = ["fn", "let"]
            .iter()
            .filter_map(|word| {
                line.find(word).map(|at| CodeSpan {
                    range: at..at + word.len(),
                    class: CodeClass::Keyword,
                })
            })
            .collect();
        (spans, 0)
    }
}

struct Host {
    _view: MarkdownView<()>,
}

impl App for Host {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

const KEYWORD: Color = Color {
    r: 255,
    g: 0,
    b: 255,
};

const OTHER: Color = Color { r: 0, g: 0, b: 0 };

fn palette() -> CodePalette {
    let colors = CodeColors {
        keyword: KEYWORD,
        function: OTHER,
        string: OTHER,
        comment: OTHER,
        number: OTHER,
        type_name: OTHER,
    };
    CodePalette {
        light: colors,
        dark: colors,
    }
}

fn render(theme: Theme, highlighted: bool, setup: impl FnOnce(&MarkdownView<()>)) -> Image {
    try_render(Snapshot::new(Dip(WIDTH), Dip(HEIGHT)).theme(theme), |ui| {
        let mut view =
            MarkdownView::new(ui, Rect::new(0, 0, WIDTH as i32, HEIGHT as i32))?.markdown(SOURCE);
        if highlighted {
            view = view.highlighter(Keywords);
        }
        setup(&view);
        Ok(Host { _view: view })
    })
    .expect("render")
}

fn count(image: &Image, test: impl Fn([u8; 3]) -> bool) -> usize {
    (0..HEIGHT as u32)
        .flat_map(|y| (0..WIDTH as u32).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let [r, g, b, _] = image.pixel(x, y).expect("inside the image");
            test([r, g, b])
        })
        .count()
}

fn is(color: Color) -> impl Fn([u8; 3]) -> bool {
    move |p| p == [color.r, color.g, color.b]
}

fn save(name: &str, image: &Image) {
    let dir = std::path::Path::new("../../target/snapshots");
    std::fs::create_dir_all(dir).expect("snapshot directory");
    image.save_png(dir.join(name)).expect("save");
}

#[test]
fn keywords_are_drawn_in_the_palette_colour() {
    for (theme, name) in [
        (Theme::light(), "markdown-highlight-light.png"),
        (Theme::dark(), "markdown-highlight-dark.png"),
    ] {
        let on = render(theme, true, |v| v.set_code_palette(Some(palette())));
        save(name, &on);
        let keyword = count(&on, is(KEYWORD));
        assert!(keyword > 20, "{keyword} keyword pixels");
        let off = render(theme, false, |v| v.set_code_palette(Some(palette())));
        assert_eq!(count(&off, is(KEYWORD)), 0, "no highlighter, no colour");
    }
}

#[test]
fn without_a_palette_code_takes_the_theme_colours() {
    for theme in [Theme::light(), Theme::dark()] {
        let themed = render(theme, true, |v| v.set_code_palette(None));
        assert_eq!(count(&themed, is(KEYWORD)), 0);
        let accent = count(&themed, is(theme.accent));
        assert!(accent > 20, "{accent} accent pixels (keywords)");
        let plain = render(theme, false, |_| {});
        assert!(count(&plain, is(theme.accent)) < accent);
        let reset = render(theme, true, |v| {
            v.set_code_palette(Some(palette()));
            v.set_code_palette(None);
        });
        assert_eq!(count(&reset, is(KEYWORD)), 0, "the palette was removed");
    }
}
