//! Renders the sample document headlessly, light and dark, into
//! `target/snapshots/rich-text-{light,dark}.png`. No window opens.
//!
//! ```text
//! cargo run -p xui-rich-text --example snapshot
//! ```

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::{Dip, Rect, Theme};
use xui_rich_text::RichTextEditor;
#[path = "../tests/common/mod.rs"]
mod common;

use common::sample_document;

struct Host {
    _editor: RichTextEditor<()>,
}

impl App for Host {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

const WIDTH: f32 = 640.0;
const HEIGHT: f32 = 760.0;

fn main() {
    let dir = std::path::Path::new("target/snapshots");
    std::fs::create_dir_all(dir).expect("snapshot directory");
    for (name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        let image = try_render(Snapshot::new(Dip(WIDTH), Dip(HEIGHT)).theme(theme), |ui| {
            let editor = RichTextEditor::new(ui, Rect::new(0, 0, WIDTH as i32, HEIGHT as i32))?
                .document(sample_document());
            Ok(Host { _editor: editor })
        })
        .expect("render");
        let path = dir.join(format!("rich-text-{name}.png"));
        image.save_png(&path).expect("save");
        println!("{}", path.display());
    }
}
