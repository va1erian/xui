//! Demonstrates the portable [`TreeView`] over a tiny in-memory tree: selecting
//! a node reports its id to a label, expanding a branch reports the toggle.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_treeview
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::image::Image;
use xui_core::widget::{Glyph, HasText, Label, TreeRow, TreeView};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

/// A 16x16 two-tone artwork icon built in memory, so the demo exercises an
/// `Image` row icon (and the backend's decoded-image cache) beside the glyphs.
fn art_image() -> Image {
    let mut pixels = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            let (r, g, b) = if (x / 4 + y / 4) % 2 == 0 {
                (0x7A, 0xC8, 0xE8)
            } else {
                (0x2A, 0x62, 0xA8)
            };
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Image::from_rgba(16, 16, pixels).expect("16x16 RGBA")
}

enum Msg {
    Select(usize),
    Toggle(usize, bool),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _tree: TreeView<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(id) => self.result.set_text(&format!("Selected node {id}")),
            Msg::Toggle(id, expanded) => {
                self.result.set_text(&format!(
                    "Node {id} {}",
                    if expanded { "expanded" } else { "collapsed" }
                ));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("TreeView demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result =
                Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Nothing selected").unwrap();
            let tree = TreeView::new(
                ui,
                l.rect(16.0, 64.0, 504.0, 240.0),
                &[
                    TreeRow::new("Inbox", 0)
                        .expandable(true)
                        .expanded(true)
                        .icon(Glyph::Folder),
                    TreeRow::new("Work", 1).icon(Glyph::Tag),
                    TreeRow::new("Art", 1).icon(art_image()),
                    TreeRow::new("Archive", 0)
                        .expandable(true)
                        .icon(Glyph::History),
                ],
            )
            .unwrap()
            .on_select(|id| Some(Msg::Select(id)))
            .on_toggle(|id, expanded| Some(Msg::Toggle(id, expanded)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _tree: tree,
            }
        },
    )
}
