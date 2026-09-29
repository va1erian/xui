//! Demonstrates the portable [`IconView`] over a small model: three buttons
//! switch the icon size and a label reports the last click, double click or
//! right click.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_iconview
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::geometry::Point;
use xui_core::icon::{IconRef, Lucide};
use xui_core::widget::{Button, HasText, IconModel, IconSize, IconView, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

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

enum Msg {
    Select(usize),
    Activate(usize),
    Context(Option<usize>, Point),
    Size(IconSize),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    view: IconView<Msg>,
    _buttons: [Button<Msg>; 3],
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(index) => self.result.set_text(&format!("Selected tile {index}")),
            Msg::Activate(index) => self.result.set_text(&format!("Activated tile {index}")),
            Msg::Context(item, at) => self.result.set_text(&format!(
                "Context on tile {} at {}, {}",
                item.map_or(-1, |item| item as i32),
                at.x,
                at.y
            )),
            Msg::Size(size) => {
                self.view.set_icon_size(size);
                self.result.set_text(&format!("Icon size: {size:?}"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("IconView demo").size(Dip(640.0), Dip(440.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 624.0, 44.0), "No click yet").unwrap();
            let view = IconView::with_model(ui, l.rect(16.0, 52.0, 624.0, 360.0), Model)
                .unwrap()
                .on_select(|index| Some(Msg::Select(index)))
                .on_activate(|index| Some(Msg::Activate(index)))
                .on_context(|item, at| Some(Msg::Context(item, at)));
            let small = Button::new(ui, l.rect(16.0, 372.0, 204.0, 404.0), "Small")
                .unwrap()
                .on_click(|| Some(Msg::Size(IconSize::Small)));
            let medium = Button::new(ui, l.rect(212.0, 372.0, 400.0, 404.0), "Medium")
                .unwrap()
                .on_click(|| Some(Msg::Size(IconSize::Medium)));
            let large = Button::new(ui, l.rect(408.0, 372.0, 596.0, 404.0), "Large")
                .unwrap()
                .on_click(|| Some(Msg::Size(IconSize::Large)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                view,
                _buttons: [small, medium, large],
            }
        },
    )
}
