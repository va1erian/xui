//! Demonstrates the portable [`TreeView`] over a tiny in-memory tree: selecting
//! a node reports its id to a label, expanding a branch reports the toggle.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_treeview
//! ```

use xui::image::Image;
use xui::prelude::*;
use xui::widget::Glyph;

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

#[derive(Clone)]
enum Msg {
    Select(NodeId),
    Toggle(NodeId, bool),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        let text = match msg {
            Msg::Select(id) => format!("Selected node {id}"),
            Msg::Toggle(id, expanded) => format!(
                "Node {id} {}",
                if expanded { "expanded" } else { "collapsed" }
            ),
        };
        self.result.get().set_text(&text);
    }
}

fn main() -> Result<()> {
    xui::app("TreeView demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Nothing selected").bind(&demo.result),
                tree_view()
                    .rows([
                        TreeRow::new("Inbox", 0)
                            .expandable(true)
                            .expanded(true)
                            .icon(Glyph::Folder),
                        TreeRow::new("Work", 1).icon(Glyph::Tag),
                        TreeRow::new("Art", 1).icon(art_image()),
                        TreeRow::new("Archive", 0)
                            .expandable(true)
                            .icon(Glyph::History),
                    ])
                    .on_select(Msg::Select)
                    .then(|tree| tree.on_toggle(|id, expanded| Some(Msg::Toggle(id, expanded))))
                    .height(176),
            )),
        )?;
        Ok(demo)
    })
}
