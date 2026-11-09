//! Renders the sample document headlessly, light and dark, into
//! `target/snapshots/markdown-{light,dark}.png`. No window opens. The
//! sample's `pic.png` is a gradient made here, as an app would hand over a
//! decoded file.
//!
//! ```text
//! cargo run -p xui-markdown --example snapshot
//! ```

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::image::Image;
use xui_core::{Dip, Rect, Theme};
use xui_markdown::MarkdownView;

const SAMPLE: &str = include_str!("../tests/sample.md");
const WIDTH: f32 = 640.0;
const HEIGHT: f32 = 760.0;

struct Host {
    _view: MarkdownView<()>,
}

impl App for Host {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// A 160 x 48 gradient, blue to orange.
fn picture() -> Image {
    let (w, h) = (160u32, 48u32);
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = x as f32 / (w - 1) as f32;
            let shade = 1.0 - 0.3 * y as f32 / (h - 1) as f32;
            let r = (40.0 + 200.0 * t) * shade;
            let g = (110.0 + 40.0 * t) * shade;
            let b = (220.0 - 180.0 * t) * shade;
            pixels.extend_from_slice(&[r as u8, g as u8, b as u8, 255]);
        }
    }
    Image::from_rgba(w, h, pixels).expect("a 160 x 48 picture")
}

fn main() {
    let dir = std::path::Path::new("target/snapshots");
    std::fs::create_dir_all(dir).expect("snapshot directory");
    for (name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        let image = try_render(Snapshot::new(Dip(WIDTH), Dip(HEIGHT)).theme(theme), |ui| {
            let view = MarkdownView::new(ui, Rect::new(0, 0, WIDTH as i32, HEIGHT as i32))?
                .markdown(SAMPLE);
            view.set_image("pic.png", picture());
            Ok(Host { _view: view })
        })
        .expect("render");
        let path = dir.join(format!("markdown-{name}.png"));
        image.save_png(&path).expect("save");
        println!("{}", path.display());
    }
}
