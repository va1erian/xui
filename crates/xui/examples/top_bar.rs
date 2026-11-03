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
//! Win32 backend is used where it exists. `XUI_SNAPSHOT=<dir>` saves a light
//! and a dark screenshot instead of opening a window.

use xui::prelude::*;
use xui_core::Insets;
use xui_core::widget::Glyph;

/// The band's height, in design units.
const BAND_HEIGHT: f32 = 36.0;
/// The inset of the band contents from the window edges and of the search
/// field from the band's right end.
const BAND_INSET: f32 = 6.0;
/// The search field's size, in design units: one text line tall, so the band's
/// surface shows above and below it instead of the field filling the band.
const SEARCH_WIDTH: f32 = 200.0;
const SEARCH_HEIGHT: f32 = 22.0;

const PREVIOUS: TopBarId = TopBarId::new(1);
const PLAY: TopBarId = TopBarId::new(2);
const STOP: TopBarId = TopBarId::new(3);
const NEXT: TopBarId = TopBarId::new(4);
const REPEAT: TopBarId = TopBarId::new(5);
const SHUFFLE: TopBarId = TopBarId::new(6);
const ELAPSED: TopBarId = TopBarId::new(7);
const SEEK: TopBarId = TopBarId::new(8);
const TOTAL: TopBarId = TopBarId::new(9);
const VOLUME: TopBarId = TopBarId::new(10);
const SEARCH_ICON: TopBarId = TopBarId::new(11);

#[derive(Clone)]
enum Msg {
    Transport(&'static str),
    Repeat(bool),
    Shuffle(bool),
    Seek(f64),
    Volume(f64),
    Search(String),
    Theme,
}

struct Demo {
    theme: Theme,
    status: Handle<Label<Msg>>,
    theme_button: Handle<Button<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        let status = self.status.get();
        match msg {
            Msg::Transport(name) => status.set_text(&format!("{name} pressed")),
            Msg::Repeat(on) => status.set_text(&format!("Repeat: {on}")),
            Msg::Shuffle(on) => status.set_text(&format!("Shuffle: {on}")),
            Msg::Seek(value) => status.set_text(&format!("Seek: {value:.0}")),
            Msg::Volume(value) => status.set_text(&format!("Volume: {value:.2}")),
            Msg::Search(text) => status.set_text(&format!("Search: {text}")),
            Msg::Theme => {
                self.theme = if self.theme.is_dark {
                    Theme::light()
                } else {
                    Theme::dark()
                };
                ui.set_theme(self.theme);
                let next = if self.theme.is_dark { "Light" } else { "Dark" };
                self.theme_button.get().set_text(next);
                status.set_text(if self.theme.is_dark {
                    "Theme: dark"
                } else {
                    "Theme: light"
                });
            }
        }
    }
}

/// The transport: buttons, toggles, a seek slider that absorbs the band's
/// leftover width, the volume and a search icon.
fn transport() -> Build<TopBar<Msg>, Msg> {
    top_bar()
        .then(|bar| {
            bar.icon(PREVIOUS, Glyph::Previous)
                .icon(PLAY, Glyph::Play)
                .icon(STOP, Glyph::Stop)
                .icon(NEXT, Glyph::Next)
                .toggle(REPEAT, Glyph::Repeat)
                .toggle(SHUFFLE, Glyph::Shuffle)
                .label(ELAPSED, "0:00")
                .slider(SEEK, 0.0, 100.0)
                .expand(SEEK)
                .label(TOTAL, "")
                .slider(VOLUME, 0.0, 1.0)
                .icon(SEARCH_ICON, Glyph::Search)
        })
        .then(|bar| {
            bar.on_click(|id| {
                let name = match id {
                    PREVIOUS => "Previous",
                    PLAY => "Play",
                    STOP => "Stop",
                    NEXT => "Next",
                    SEARCH_ICON => "Search",
                    _ => return None,
                };
                Some(Msg::Transport(name))
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
        })
}

fn main() -> Result<()> {
    xui::app("TopBar demo").size(900, 220).run(|ui| {
        ui.set_theme(Theme::dark());
        let demo = Demo {
            theme: Theme::dark(),
            status: Handle::new(),
            theme_button: Handle::new(),
        };
        let search: Handle<Edit<Msg>> = Handle::new();
        // The band's surface is a full-width top bar of its own, stacked
        // under the transport and the search field, so the field's column and
        // the space above and below the one-line field show the band colour
        // instead of the window background. The portable TopBar has no native
        // slot, so the field is a sibling sharing the band.
        let inset = Dip(BAND_INSET);
        let band = stack()
            .children((
                top_bar(),
                row()
                    .padding(Insets::new(Dip(0.0), Dip(0.0), inset, Dip(0.0)))
                    .children((
                        transport().fill(1),
                        edit()
                            .on_change(Msg::Search)
                            .bind(&search)
                            .size(SEARCH_WIDTH, SEARCH_HEIGHT)
                            .align_y(Align::Center),
                    )),
            ))
            .height(BAND_HEIGHT);
        ui.root(
            column()
                .padding(Insets::symmetric(inset, Dip(8.0)))
                .gap(20)
                .children((
                    band,
                    label("Top bar ready").bind(&demo.status),
                    button("Light")
                        .on_click(Msg::Theme)
                        .bind(&demo.theme_button)
                        .size(140, 36),
                )),
        )?;
        search.get().focus();
        Ok(demo)
    })
}
