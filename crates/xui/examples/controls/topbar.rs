//! Demonstrates the portable [`TopBar`]: vector transport icons, toggles, a
//! label, an expanding seek slider and a fixed-width volume slider report
//! through one label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_topbar
//! ```

use xui::prelude::*;
use xui::widget::Glyph;

const PREVIOUS: TopBarId = TopBarId::new(1);
const PLAY: TopBarId = TopBarId::new(2);
const PAUSE: TopBarId = TopBarId::new(3);
const STOP: TopBarId = TopBarId::new(4);
const NEXT: TopBarId = TopBarId::new(5);
const REPEAT: TopBarId = TopBarId::new(6);
const SHUFFLE: TopBarId = TopBarId::new(7);
const SEEK: TopBarId = TopBarId::new(8);
const VOLUME: TopBarId = TopBarId::new(9);

#[derive(Clone)]
enum Msg {
    Transport(&'static str),
    Repeat(bool),
    Shuffle(bool),
    Seek(f64),
    Volume(f64),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        let text = match msg {
            Msg::Transport(name) => format!("{name} pressed"),
            Msg::Repeat(on) => format!("Repeat: {on}"),
            Msg::Shuffle(on) => format!("Shuffle: {on}"),
            Msg::Seek(value) => format!("Seek: {value:.0}"),
            Msg::Volume(value) => format!("Volume: {value:.0}"),
        };
        self.result.get().set_text(&text);
    }
}

/// The bar's items and the mapping of each event to a message (ids with no
/// message, such as the labels, raise nothing).
fn transport(bar: TopBar<Msg>) -> TopBar<Msg> {
    bar.icon(PREVIOUS, Glyph::Previous)
        .icon(PLAY, Glyph::Play)
        .icon(PAUSE, Glyph::Pause)
        .icon(STOP, Glyph::Stop)
        .icon(NEXT, Glyph::Next)
        .toggle(REPEAT, Glyph::Repeat)
        .toggle(SHUFFLE, Glyph::Shuffle)
        .label(TopBarId::new(10), "Seek")
        .slider(SEEK, 0.0, 100.0)
        .expand(SEEK)
        .label(TopBarId::new(11), "Vol")
        .slider(VOLUME, 0.0, 100.0)
        .width(VOLUME, Dip(80.0))
        .on_click(|id| match id {
            PREVIOUS => Some(Msg::Transport("Previous")),
            PLAY => Some(Msg::Transport("Play")),
            PAUSE => Some(Msg::Transport("Pause")),
            STOP => Some(Msg::Transport("Stop")),
            NEXT => Some(Msg::Transport("Next")),
            _ => None,
        })
        .on_toggle(|id, checked| match id {
            REPEAT => Some(Msg::Repeat(checked)),
            SHUFFLE => Some(Msg::Shuffle(checked)),
            _ => None,
        })
        .on_change(|id, value| match id {
            SEEK => Some(Msg::Seek(value)),
            VOLUME => Some(Msg::Volume(value)),
            _ => None,
        })
}

fn main() -> Result<()> {
    xui::app("TopBar demo").size(640, 200).run(|ui| {
        let demo = Demo::default();
        ui.root(column().padding(16).gap(16).children((
            top_bar().then(transport),
            label("Top bar ready").bind(&demo.result),
        )))?;
        Ok(demo)
    })
}
