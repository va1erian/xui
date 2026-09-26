//! A portable, virtual [`GridView`] over a 10 000-album model: a custom tile
//! painter draws album art and a caption, and clicking a tile selects it.
//!
//! Run with:
//!
//! ```text
//! XUI_BACKEND=canvas cargo run -p xui --features canvas --example gridview
//! ```
//!
//! `XUI_LIST_THEME=dark` starts on the dark palette and `XUI_DEMO_AUTOCLOSE_MS`
//! makes it quit itself, for headless smoke runs.

use std::cell::Cell;
use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec, TextStyle};
use xui_core::widget::{GridModel, GridView, StatusBar, Tile, TileSize};
use xui_core::{Dip, Image, Rect, Theme};

/// One album; the caption is pre-formatted so the model can borrow it.
struct Album {
    caption: String,
}

/// A virtual grid model: a shared album store plus reusable art.
struct Library {
    albums: Vec<Album>,
    art: Vec<Image>,
}

impl GridModel for Library {
    fn len(&self) -> usize {
        self.albums.len()
    }

    fn tile(&self, index: usize) -> Option<Tile<'_>> {
        let album = self.albums.get(index)?;
        let image = self.art.get(index % self.art.len())?;
        Some(Tile::new(&album.caption).image(image))
    }
}

enum Msg {
    Select(usize),
    Activate(usize),
    Autoclose,
}

struct AppState {
    grid: GridView<Msg>,
    status: StatusBar<Msg>,
}

impl App for AppState {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(index) => {
                let total = self.grid.len();
                self.status.set_text(0, &format!("{total} albums"));
                self.status.set_text(1, &format!("Album {}", index + 1));
            }
            Msg::Activate(index) => self.status.set_text(1, &format!("Play {index}")),
            Msg::Autoclose => ui.quit(),
        }
    }
}

/// `count` small gradient tiles with a cycling palette.
fn art(count: usize) -> Vec<Image> {
    (0..count)
        .map(|index| {
            let base = [
                (index * 37 % 256) as u8,
                (index * 91 % 256) as u8,
                (index * 53 % 256) as u8,
            ];
            let mut pixels = Vec::with_capacity(16 * 16 * 4);
            for y in 0..16 {
                for x in 0..16 {
                    let shade = ((x + y) * 6) as u8;
                    pixels.extend_from_slice(&[
                        base[0].saturating_add(shade),
                        base[1].saturating_add(shade),
                        base[2].saturating_add(shade),
                        255,
                    ]);
                }
            }
            Image::from_rgba(16, 16, pixels).unwrap()
        })
        .collect()
}

fn albums(count: usize) -> Vec<Album> {
    (0..count)
        .map(|index| Album {
            caption: format!("Album {} · Artist {}", index + 1, index % 37),
        })
        .collect()
}

fn backend() -> Rc<dyn Backend> {
    #[cfg(all(feature = "win32", windows))]
    {
        if std::env::var("XUI_BACKEND").as_deref() == Ok("canvas") {
            Rc::new(xui_canvas::WinitBackend::new())
        } else {
            Rc::new(xui_win32::Win32Backend::new())
        }
    }
    #[cfg(not(all(feature = "win32", windows)))]
    {
        Rc::new(xui_canvas::WinitBackend::new())
    }
}

fn main() {
    let _ = run_app(
        backend(),
        PlatformSpec::new("xui gridview").size(Dip(760.0), Dip(520.0)),
        |ui| {
            let dpi = ui.dpi();
            let p = move |value: f32| Dip(value).to_px(dpi).value();
            let grid_rect = Rect::new(p(12.0), p(12.0), p(748.0), p(452.0));

            let library = Library {
                albums: albums(10_000),
                art: art(12),
            };
            let grid = GridView::with_model(ui, grid_rect, library)
                .unwrap()
                .tile_size(TileSize::new(Dip(148.0), Dip(168.0)).gap(Dip(12.0)))
                .on_select(|index| Some(Msg::Select(index)))
                .on_activate(|index| Some(Msg::Activate(index)))
                .on_paint_tile(|canvas, paint| {
                    let dpi = paint.dpi;
                    let inner = Dip(6.0).to_px(dpi).value();
                    let caption = Dip(20.0).to_px(dpi).value();
                    let rect = paint.rect;
                    let art = Rect::new(
                        rect.left + inner,
                        rect.top + inner,
                        rect.right - inner,
                        rect.bottom - inner - caption,
                    );
                    if let Some(image) = paint.tile.image {
                        canvas.draw_image(image, art);
                    }
                    let text = Rect::new(
                        rect.left + inner,
                        art.bottom,
                        rect.right - inner,
                        rect.bottom - inner,
                    );
                    let style = TextStyle::new(paint.theme.text, Dip(11.0)).middle();
                    canvas.draw_text(paint.tile.label, text, &style);
                });

            let status = StatusBar::new(
                ui,
                Rect::new(p(12.0), p(460.0), p(748.0), p(492.0)),
                &["Ready", "Album 1"],
            )
            .unwrap();

            let autoclose = Rc::new(Cell::new(None));
            if let Ok(millis) = std::env::var("XUI_DEMO_AUTOCLOSE_MS") {
                let _ = millis
                    .parse::<u32>()
                    .map(|ms| autoclose.set(Some(ui.set_timer(ms))));
            }
            let autoclose_for_timer = Rc::clone(&autoclose);
            ui.on_timer(move |fired| {
                (autoclose_for_timer.get() == Some(fired)).then_some(Msg::Autoclose)
            });

            if std::env::var("XUI_LIST_THEME").as_deref() == Ok("dark") {
                ui.set_theme(Theme::dark());
                // Re-composite now, so a backend that only stores the theme
                // presents the dark frame instead of the first light one.
                ui.invalidate(grid.id());
            }

            AppState { grid, status }
        },
    );
}
