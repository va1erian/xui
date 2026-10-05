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
//! makes it quit itself, for headless smoke runs; `XUI_SNAPSHOT=<dir>` saves a
//! light and a dark screenshot instead.

use xui::prelude::*;
use xui_core::Image;
use xui_core::backend::{Canvas, TextStyle};
use xui_core::widget::{GridModel, Tile, TilePaint, TileSize};

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

#[derive(Clone)]
enum Msg {
    Select(usize),
    Activate(usize),
}

#[derive(Default)]
struct AppState {
    grid: Handle<GridView<Msg>>,
    status: Handle<StatusBar<Msg>>,
}

impl App for AppState {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        let status = self.status.get();
        match msg {
            Msg::Select(index) => {
                let total = self.grid.get().len();
                status.set_text(0, &format!("{total} albums"));
                status.set_text(1, &format!("Album {}", index + 1));
            }
            Msg::Activate(index) => status.set_text(1, &format!("Play {index}")),
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

/// Draws a tile: its art above a one-line caption.
fn paint_tile(canvas: &mut dyn Canvas, paint: &TilePaint<'_>) {
    let dpi = paint.dpi;
    let inner = Dip(6.0).to_px(dpi).value();
    let caption = Dip(20.0).to_px(dpi).value();
    let (art, text) = paint.rect.shrink(inner).split_bottom(caption);
    if let Some(image) = paint.tile.image {
        canvas.draw_image(image, art);
    }
    let style = TextStyle::new(paint.theme.text, Dip(11.0)).middle();
    canvas.draw_text(paint.tile.label, text, &style);
}

fn albums(count: usize) -> Vec<Album> {
    (0..count)
        .map(|index| Album {
            caption: format!("Album {} · Artist {}", index + 1, index % 37),
        })
        .collect()
}

fn main() -> Result<()> {
    xui::app("xui gridview").size(760, 520).run(|ui| {
        let library = Library {
            albums: albums(10_000),
            art: art(12),
        };
        let app = AppState::default();
        ui.root(
            column().padding(12).gap(8).children((
                grid_view_with(library)
                    .on_select(Msg::Select)
                    .on_activate(Msg::Activate)
                    .then(|grid| {
                        grid.tile_size(TileSize::new(Dip(148.0), Dip(168.0)).gap(Dip(12.0)))
                            .on_paint_tile(paint_tile)
                    })
                    .bind(&app.grid)
                    .fill(1),
                status_bar(&["Ready", "Album 1"])
                    .bind(&app.status)
                    .height(32),
            )),
        )?;
        if std::env::var("XUI_LIST_THEME").as_deref() == Ok("dark") {
            ui.set_theme(Theme::dark());
            // Re-composite now, so a backend that only stores the theme
            // presents the dark frame instead of the first light one.
            ui.invalidate(app.grid.get().id());
        }
        Ok(app)
    })
}
