//! The layout containers beyond rows, columns and grids: a `wrap()` of
//! chips that breaks into lines as the window narrows, a `scroll()` of
//! settings, an `overlay()` badge over a card, and a `group()` hidden as one
//! unit. Labels change length as you click, and the layout follows on its own.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example containers
//! ```
//!
//! `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself; `XUI_SNAPSHOT=<dir>` saves a
//! light and a dark screenshot instead of opening a window.

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Pick(&'static str),
    Advanced(bool),
}

#[derive(Default)]
struct Demo {
    picked: Handle<Label<Msg>>,
    advanced: Handle<GroupBox<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Pick(tag) => self.picked.get().set_text(&format!("Picked: {tag}")),
            Msg::Advanced(on) => ui.set_visible(self.advanced.get().id(), on),
        }
    }
}

const TAGS: [&str; 9] = [
    "Rust",
    "Layout",
    "Wrap",
    "Overlay",
    "Scroll",
    "Grid",
    "Themes",
    "Dark mode",
    "Builders",
];

fn main() -> Result<()> {
    xui::app("Containers").size(560, 420).run(|ui| {
        let demo = Demo::default();
        let chips: Vec<Entry<Msg>> = TAGS
            .iter()
            .map(|tag| {
                toggle_button(*tag)
                    .on_toggle(move |_| Msg::Pick(tag))
                    .into_entry()
            })
            .collect();
        let settings = (1..=12)
            .map(|n| checkbox(format!("Setting number {n}")).into_entry())
            .collect::<Vec<_>>();
        ui.root(
            column().padding(16).gap(12).children((
                label("Tags").caption(),
                wrap().gap(6).children(chips),
                label("Nothing picked yet").bind(&demo.picked),
                row()
                    .gap(12)
                    .children((
                        scroll(column().padding(8).gap(4).children(settings)).fill(1),
                        column()
                            .gap(8)
                            .children((
                                overlay()
                                    .children((
                                        group("Card", column().padding(8).child(label("Content")))
                                            .align(Align::Stretch),
                                        button("New").primary().align(Align::End),
                                    ))
                                    .fill(1),
                                checkbox("Advanced").checked(true).on_toggle(Msg::Advanced),
                                group("Advanced", column().padding(8).child(slider(0.0, 10.0)))
                                    .bind(&demo.advanced),
                            ))
                            .fill(1),
                    ))
                    .fill(1),
            )),
        )?;
        Ok(demo)
    })
}
