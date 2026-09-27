//! A portable, app-shaped demo of the [`TopBar`](xui_core::widget::TopBar)
//! transport band with a search field beside it, mirroring the emusic top bar:
//! the band is one surface across its whole width, with a one-text-line-tall
//! search field inset at the right. The dark-mode input contrast (#119) can be
//! checked on the software (tiny-skia) path as well as the native one.
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

/// The demo window's design width; the band is inset from both edges.
const WINDOW_WIDTH: f32 = 900.0;
/// The band's vertical extent, in design units.
const BAND_TOP: f32 = 8.0;
const BAND_BOTTOM: f32 = 44.0;
/// The horizontal inset of the band contents from the window edges.
const BAND_INSET: f32 = 6.0;
/// The search field's size, in design units: one text line tall, so the band's
/// surface shows above and below it instead of the field filling the band.
const SEARCH_WIDTH: f32 = 200.0;
const SEARCH_HEIGHT: f32 = 22.0;

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
    /// A band-wide surface behind the transport bar and the search field, so
    /// the field's column shares the band's background colour.
    _band: TopBar<Msg>,
    /// Kept alive so the transport stays on screen; its events drive `Msg`.
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
        PlatformSpec::new("TopBar demo").size(Dip(WINDOW_WIDTH), Dip(220.0)),
        |ui| {
            ui.set_theme(Theme::dark());
            let l = Layout::new(ui.dpi());

            // The band's geometry, as in the emusic top bar: a field width and
            // inset off the right edge, and the transport bar flush to it.
            let band_left = BAND_INSET;
            let band_right = WINDOW_WIDTH - BAND_INSET;
            let search_left = band_right - SEARCH_WIDTH - BAND_INSET;
            let search_top = (BAND_TOP + BAND_BOTTOM - SEARCH_HEIGHT) / 2.0;
            let search_right = band_right - BAND_INSET;

            // The band's surface, full width. Created first so the transport
            // bar and the search field sit on top of it; the field's column and
            // the space above and below the one-line field stay the band colour
            // instead of showing the window background.
            let band = TopBar::new(ui, l.rect(band_left, BAND_TOP, band_right, BAND_BOTTOM))
                .expect("band");

            let previous = TopBarId::new(1);
            let play = TopBarId::new(2);
            let stop = TopBarId::new(3);
            let next = TopBarId::new(4);
            let repeat = TopBarId::new(5);
            let shuffle = TopBarId::new(6);
            let elapsed = TopBarId::new(7);
            let seek = TopBarId::new(8);
            let total = TopBarId::new(9);
            let volume = TopBarId::new(10);
            let search_icon = TopBarId::new(11);
            let bar = TopBar::new(ui, l.rect(band_left, BAND_TOP, search_left, BAND_BOTTOM))
                .expect("top bar")
                .icon(previous, Glyph::Previous)
                .icon(play, Glyph::Play)
                .icon(stop, Glyph::Stop)
                .icon(next, Glyph::Next)
                .toggle(repeat, Glyph::Repeat)
                .toggle(shuffle, Glyph::Shuffle)
                .label(elapsed, "0:00")
                .slider(seek, 0.0, 100.0)
                // The seek absorbs the band's leftover width, so it stretches
                // between the transport and the volume/search controls.
                .expand(seek)
                .label(total, "")
                .slider(volume, 0.0, 1.0)
                .icon(search_icon, Glyph::Search)
                .on_click(move |id| {
                    if id == previous {
                        Some(Msg::Transport("Previous"))
                    } else if id == play {
                        Some(Msg::Transport("Play"))
                    } else if id == stop {
                        Some(Msg::Transport("Stop"))
                    } else if id == next {
                        Some(Msg::Transport("Next"))
                    } else if id == search_icon {
                        Some(Msg::Transport("Search"))
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
            // sibling widget sharing the same band, flush with the bar.
            let search = Edit::new(
                ui,
                l.rect(
                    search_left,
                    search_top,
                    search_right,
                    search_top + SEARCH_HEIGHT,
                ),
                "",
            )
            .expect("search")
            .on_change(|text| Some(Msg::Search(text.to_owned())));
            search.focus();
            // The band's surface is a full-width sibling behind the transport
            // and the field; keep those two above it.
            ui.raise(bar.id());
            ui.raise(search.id());

            let status =
                Label::new(ui, l.rect(8.0, 64.0, 892.0, 92.0), "Top bar ready").expect("status");
            let theme_button = Button::new(ui, l.rect(8.0, 104.0, 148.0, 140.0), "Light")
                .expect("theme")
                .on_click(|| Some(Msg::Theme));

            autoclose(ui, || Msg::Quit);

            Demo {
                theme: Theme::dark(),
                _band: band,
                _bar: bar,
                _search: search,
                status,
                theme_button,
            }
        },
    )
}
