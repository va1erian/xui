//! Demonstrates the portable [`IconView`] over a small model: three buttons
//! switch the icon size and a label reports the last click, double click or
//! right click.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_iconview
//! ```

use xui::icon::{IconRef, Lucide};
use xui::prelude::*;
use xui::widget::{IconModel, IconSize};

const NAMES: [&str; 6] = [
    "Reports",
    "Pictures",
    "Music",
    "Videos",
    "Documents",
    "Archive",
];
const KINDS: [&str; 6] = [
    "Text Document",
    "Image",
    "Audio",
    "Video",
    "Folder",
    "Compressed",
];
const SIZES: [&str; 6] = ["12 KB", "2.4 MB", "1 KB", "48 MB", "", "6.1 MB"];

struct Model;

impl IconModel for Model {
    fn items(&self) -> usize {
        NAMES.len()
    }

    fn icon(&self, item: usize) -> Option<IconRef> {
        Some(
            match item % 6 {
                0 => Lucide::File,
                1 => Lucide::Image,
                2 => Lucide::Play,
                3 => Lucide::Download,
                4 => Lucide::Folder,
                _ => Lucide::Package,
            }
            .into(),
        )
    }

    fn line(&self, item: usize, line: usize) -> Option<&str> {
        match line {
            0 => Some(NAMES[item % NAMES.len()]),
            1 => Some(KINDS[item % KINDS.len()]),
            2 => Some(SIZES[item % SIZES.len()]),
            _ => None,
        }
    }
}

#[derive(Clone)]
enum Msg {
    Select(usize),
    Activate(usize),
    Context(Option<usize>, Point),
    Size(IconSize),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
    view: Handle<IconView<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        let result = self.result.get();
        match msg {
            Msg::Select(index) => result.set_text(&format!("Selected tile {index}")),
            Msg::Activate(index) => result.set_text(&format!("Activated tile {index}")),
            Msg::Context(item, at) => result.set_text(&format!(
                "Context on tile {} at {}, {}",
                item.map_or(-1, |item| item as i32),
                at.x,
                at.y
            )),
            Msg::Size(size) => {
                self.view.get().set_icon_size(size);
                result.set_text(&format!("Icon size: {size:?}"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("IconView demo").size(640, 440).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(8).children((
                label("No click yet").bind(&demo.result),
                icon_view_with(Model)
                    .on_select(Msg::Select)
                    .on_activate(Msg::Activate)
                    .then(|view| view.on_context(|item, at| Some(Msg::Context(item, at))))
                    .bind(&demo.view)
                    .fill(1),
                row().gap(8).children((
                    button("Small").on_click(Msg::Size(IconSize::Small)).fill(1),
                    button("Medium")
                        .on_click(Msg::Size(IconSize::Medium))
                        .fill(1),
                    button("Large").on_click(Msg::Size(IconSize::Large)).fill(1),
                )),
            )),
        )?;
        Ok(demo)
    })
}
