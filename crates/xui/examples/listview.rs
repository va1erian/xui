//! A portable, virtual [`ListView`] over a 5000-row model: columns and a
//! header, a sort arrow, multi-select and a context hook.
//!
//! Run with:
//!
//! ```text
//! XUI_BACKEND=canvas cargo run -p xui --features canvas --example listview
//! ```
//!
//! `XUI_LIST_THEME=dark` starts on the dark palette and
//! `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself, for headless smoke runs;
//! `XUI_SNAPSHOT=<dir>` saves a light and a dark screenshot instead.

use std::rc::Rc;

use xui::prelude::*;
use xui_core::icon::{IconRef, Lucide};
use xui_core::widget::{ListModel, SortDirection};

/// One row of mock data; the numeric columns are pre-formatted so the
/// model's accessors can borrow `&str`.
struct Track {
    title: String,
    artist: String,
    album: String,
    year: String,
}

/// A virtual model: a shared row store plus the display order.
struct TrackModel {
    tracks: Rc<Vec<Track>>,
    order: Vec<usize>,
}

impl ListModel for TrackModel {
    fn rows(&self) -> usize {
        self.order.len()
    }

    fn cell(&self, row: usize, column: usize) -> Option<&str> {
        let track = self.tracks.get(*self.order.get(row)?)?;
        Some(match column {
            0 => track.title.as_str(),
            1 => track.artist.as_str(),
            2 => track.album.as_str(),
            3 => track.year.as_str(),
            _ => "",
        })
    }

    /// Flags a few tracks with a leading icon, so the demo shows row icons.
    fn icon(&self, row: usize) -> Option<IconRef> {
        let index = *self.order.get(row)?;
        match index % 11 {
            0 => Some(Lucide::CircleX.into()),
            5 => Some(Lucide::TriangleAlert.into()),
            _ => None,
        }
    }
}

#[derive(Clone)]
enum Msg {
    Select(Vec<usize>),
    Play(usize),
    Context(usize, Point),
    Sort(usize),
    Resize(usize, Dip),
}

struct Library {
    list: Handle<ListView<Msg>>,
    status: Handle<StatusBar<Msg>>,
    tracks: Rc<Vec<Track>>,
    order: Vec<usize>,
    sort: Option<(usize, bool)>,
}

impl Library {
    fn model(&self) -> TrackModel {
        TrackModel {
            tracks: Rc::clone(&self.tracks),
            order: self.order.clone(),
        }
    }

    fn sort_by(&mut self, column: usize) {
        let ascending = match self.sort {
            Some((sorted, was)) if sorted == column => !was,
            _ => true,
        };
        if let Some((sorted, _)) = self.sort
            && sorted != column
        {
            self.list.get().clear_sort_indicator(sorted);
        }
        let tracks = Rc::clone(&self.tracks);
        let key = |&row: &usize| &tracks[row];
        match column {
            1 => self.order.sort_by(|a, b| key(a).artist.cmp(&key(b).artist)),
            2 => self.order.sort_by(|a, b| key(a).album.cmp(&key(b).album)),
            3 => self.order.sort_by(|a, b| key(a).year.cmp(&key(b).year)),
            _ => self.order.sort_by(|a, b| key(a).title.cmp(&key(b).title)),
        }
        if !ascending {
            self.order.reverse();
        }
        self.sort = Some((column, ascending));
        self.list.get().set_sort_indicator(
            column,
            if ascending {
                SortDirection::Ascending
            } else {
                SortDirection::Descending
            },
        );
        self.list.get().set_model(self.model());
    }
}

impl App for Library {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        let note = match msg {
            Msg::Select(rows) => format!("{} selected", rows.len()),
            Msg::Play(row) => format!("Play row {row}"),
            Msg::Context(row, at) => format!("Context row {row} at {},{}", at.x, at.y),
            Msg::Sort(column) => {
                self.sort_by(column);
                format!("Sorted by column {column}")
            }
            Msg::Resize(column, width) => format!("Column {column} is now {}px", width.value()),
        };
        self.status.get().set_text(1, &note);
    }
}

fn tracks(count: usize) -> Vec<Track> {
    (0..count)
        .map(|index| Track {
            title: format!("Track {}", index + 1),
            artist: format!("Artist {}", index % 37),
            album: format!("Album {}", index % 13),
            year: format!("{}", 1950 + index % 75),
        })
        .collect()
}

fn main() -> Result<()> {
    xui::app("xui listview").size(720, 460).run(|ui| {
        let tracks = Rc::new(tracks(5000));
        let order: Vec<usize> = (0..tracks.len()).collect();
        let model = TrackModel {
            tracks: Rc::clone(&tracks),
            order: order.clone(),
        };
        let library = Library {
            list: Handle::new(),
            status: Handle::new(),
            tracks,
            order,
            sort: None,
        };
        ui.root(
            column().padding(12).gap(8).children((
                list()
                    .column("Title", Fill)
                    .column("Artist", 140)
                    .column("Album", 140)
                    .column_right("Year", 56)
                    .on_activate(Msg::Play)
                    .then(move |list| {
                        list.set_model(model);
                        list.multi_select(true)
                            .on_selection(|rows| Some(Msg::Select(rows.to_vec())))
                            .on_context(|row, at| Some(Msg::Context(row, at)))
                            .on_sort(|column| Some(Msg::Sort(column)))
                            .on_resize(|column, width| Some(Msg::Resize(column, width)))
                    })
                    .bind(&library.list)
                    .fill(1),
                status_bar(&["Ready", "3 selected"])
                    .bind(&library.status)
                    .height(32),
            )),
        )?;
        library.list.get().set_selection(&[1, 2, 3]);
        if std::env::var("XUI_LIST_THEME").as_deref() == Ok("dark") {
            ui.set_theme(Theme::dark());
        }
        Ok(library)
    })
}
