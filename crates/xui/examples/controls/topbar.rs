//! Demonstrates the portable [`TopBar`]: vector transport icons, toggles, a
//! label, an expanding seek slider and a fixed-width volume slider report
//! through one label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_topbar
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Glyph, HasText, Label, TopBar, TopBarId};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Transport(&'static str),
    Repeat(bool),
    Shuffle(bool),
    Seek(f64),
    Volume(f64),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _bar: TopBar<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Transport(name) => self.result.set_text(&format!("{name} pressed")),
            Msg::Repeat(on) => self.result.set_text(&format!("Repeat: {on}")),
            Msg::Shuffle(on) => self.result.set_text(&format!("Shuffle: {on}")),
            Msg::Seek(value) => self.result.set_text(&format!("Seek: {value:.0}")),
            Msg::Volume(value) => self.result.set_text(&format!("Volume: {value:.0}")),
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("TopBar demo").size(Dip(640.0), Dip(200.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let previous = TopBarId::new(1);
            let play = TopBarId::new(2);
            let pause = TopBarId::new(3);
            let stop = TopBarId::new(4);
            let next = TopBarId::new(5);
            let repeat = TopBarId::new(6);
            let shuffle = TopBarId::new(7);
            let seek = TopBarId::new(8);
            let volume = TopBarId::new(9);
            let bar = TopBar::new(ui, l.rect(16.0, 16.0, 624.0, 72.0))
                .unwrap()
                .icon(previous, Glyph::Previous)
                .icon(play, Glyph::Play)
                .icon(pause, Glyph::Pause)
                .icon(stop, Glyph::Stop)
                .icon(next, Glyph::Next)
                .toggle(repeat, Glyph::Repeat)
                .toggle(shuffle, Glyph::Shuffle)
                .label(TopBarId::new(10), "Seek")
                .slider(seek, 0.0, 100.0)
                .expand(seek)
                .label(TopBarId::new(11), "Vol")
                .slider(volume, 0.0, 100.0)
                .width(volume, Dip(80.0))
                .on_click(move |id| match id {
                    id if id == previous => Some(Msg::Transport("Previous")),
                    id if id == play => Some(Msg::Transport("Play")),
                    id if id == pause => Some(Msg::Transport("Pause")),
                    id if id == stop => Some(Msg::Transport("Stop")),
                    id if id == next => Some(Msg::Transport("Next")),
                    _ => None,
                })
                .on_toggle(move |id, checked| {
                    if id == repeat {
                        Some(Msg::Repeat(checked))
                    } else if id == shuffle {
                        Some(Msg::Shuffle(checked))
                    } else {
                        None
                    }
                })
                .on_change(move |id, value| {
                    if id == seek {
                        Some(Msg::Seek(value))
                    } else if id == volume {
                        Some(Msg::Volume(value))
                    } else {
                        None
                    }
                });
            let result = Label::new(ui, l.rect(16.0, 88.0, 624.0, 120.0), "Top bar ready").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo { result, _bar: bar }
        },
    )
}
