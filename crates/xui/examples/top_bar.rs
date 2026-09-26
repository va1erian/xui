//! A portable, app-shaped demo of the [`TopBar`](xui_core::widget::TopBar)
//! band with a search field beside it, so the dark-mode input contrast (#119)
//! can be checked on the software (tiny-skia) path as well as the native one.
//!
//! Run with:
//!
//! ```text
//! XUI_BACKEND=canvas cargo run -p xui --features canvas --example top_bar
//! ```
//!
//! `XUI_BACKEND=canvas` selects the software backend; without it the native
//! Win32 backend is used where it exists.

use xui_core::Dip;
use xui_core::Theme;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Button, Edit, Glyph, HasText, Label, TopBar, TopBarId};

#[path = "controls/support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Transport(&'static str),
    Repeat(bool),
    Shuffle(bool),
    Seek(f64),
    Volume(f64),
    Search(String),
    Theme,
    Quit,
}

struct Demo {
    theme: Theme,
    /// Kept alive so the band stays on screen; its events drive `Msg`.
    _bar: TopBar<Msg>,
    /// Kept alive so the search field stays on screen.
    _search: Edit<Msg>,
    status: Label<Msg>,
    theme_button: Button<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Transport(name) => self.status.set_text(&format!("{name} pressed")),
            Msg::Repeat(on) => self.status.set_text(&format!("Repeat: {on}")),
            Msg::Shuffle(on) => self.status.set_text(&format!("Shuffle: {on}")),
            Msg::Seek(value) => self.status.set_text(&format!("Seek: {value:.0}")),
            Msg::Volume(value) => self.status.set_text(&format!("Volume: {value:.2}")),
            Msg::Search(text) => self.status.set_text(&format!("Search: {text}")),
            Msg::Theme => {
                self.theme = if self.theme.is_dark {
                    Theme::light()
                } else {
                    Theme::dark()
                };
                ui.set_theme(self.theme);
                let next = if self.theme.is_dark { "Light" } else { "Dark" };
                self.theme_button.set_text(next);
                self.status.set_text(if self.theme.is_dark {
                    "Theme: dark"
                } else {
                    "Theme: light"
                });
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("TopBar demo").size(Dip(900.0), Dip(220.0)),
        |ui| {
            ui.set_theme(Theme::dark());
            let l = Layout::new(ui.dpi());

            let previous = TopBarId::new(1);
            let play = TopBarId::new(2);
            let pause = TopBarId::new(3);
            let stop = TopBarId::new(4);
            let next = TopBarId::new(5);
            let repeat = TopBarId::new(6);
            let shuffle = TopBarId::new(7);
            let seek = TopBarId::new(8);
            let elapsed = TopBarId::new(9);
            let volume = TopBarId::new(10);
            let bar = TopBar::new(ui, l.rect(8.0, 8.0, 656.0, 44.0))
                .expect("top bar")
                .icon(previous, Glyph::Previous)
                .icon(play, Glyph::Play)
                .icon(pause, Glyph::Pause)
                .icon(stop, Glyph::Stop)
                .icon(next, Glyph::Next)
                .toggle(repeat, Glyph::Repeat)
                .toggle(shuffle, Glyph::Shuffle)
                .slider(seek, 0.0, 100.0)
                .expand(seek)
                .label(elapsed, "0:00")
                .slider(volume, 0.0, 1.0)
                .width(volume, Dip(80.0))
                .on_click(move |id| {
                    if id == previous {
                        Some(Msg::Transport("Previous"))
                    } else if id == play {
                        Some(Msg::Transport("Play"))
                    } else if id == pause {
                        Some(Msg::Transport("Pause"))
                    } else if id == stop {
                        Some(Msg::Transport("Stop"))
                    } else if id == next {
                        Some(Msg::Transport("Next"))
                    } else {
                        None
                    }
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

            // The portable TopBar has no native slot, so the search field is a
            // sibling widget sharing the same band, mirroring the win32 demo.
            let search = Edit::new(ui, l.rect(676.0, 8.0, 892.0, 44.0), "")
                .expect("search")
                .on_change(|text| Some(Msg::Search(text.to_owned())));
            search.focus();

            let status =
                Label::new(ui, l.rect(8.0, 64.0, 892.0, 92.0), "Top bar ready").expect("status");
            let theme_button = Button::new(ui, l.rect(8.0, 104.0, 148.0, 140.0), "Light")
                .expect("theme")
                .on_click(|| Some(Msg::Theme));

            autoclose(ui, || Msg::Quit);

            Demo {
                theme: Theme::dark(),
                _bar: bar,
                _search: search,
                status,
                theme_button,
            }
        },
    )
}
