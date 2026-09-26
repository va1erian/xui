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
//! `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself, for headless smoke runs.

use std::cell::Cell;
use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::widget::{Fill, ListModel, ListView, SortDirection, StatusBar};
use xui_core::{Dip, Rect, Theme};

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
}

enum Msg {
    Select(Vec<usize>),
    Play(usize),
    Context(usize),
    Sort(usize),
    Autoclose,
}

struct Library {
    list: ListView<Msg>,
    status: StatusBar<Msg>,
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
            self.list.clear_sort_indicator(sorted);
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
        self.list.set_sort_indicator(
            column,
            if ascending {
                SortDirection::Ascending
            } else {
                SortDirection::Descending
            },
        );
        self.list.set_model(self.model());
    }
}

impl App for Library {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(rows) => self.status.set_text(1, &format!("{} selected", rows.len())),
            Msg::Play(row) => self.status.set_text(1, &format!("Play row {row}")),
            Msg::Context(row) => self.status.set_text(1, &format!("Context row {row}")),
            Msg::Sort(column) => {
                self.sort_by(column);
                self.status
                    .set_text(1, &format!("Sorted by column {column}"));
            }
            Msg::Autoclose => ui.quit(),
        }
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
        PlatformSpec::new("xui listview").size(Dip(720.0), Dip(460.0)),
        |ui| {
            let dpi = ui.dpi();
            let p = move |value: f32| Dip(value).to_px(dpi).value();
            let list_rect = Rect::new(p(12.0), p(12.0), p(708.0), p(400.0));

            let tracks = Rc::new(tracks(5000));
            let order: Vec<usize> = (0..tracks.len()).collect();
            let list = ListView::with_model(
                ui,
                list_rect,
                TrackModel {
                    tracks: Rc::clone(&tracks),
                    order: order.clone(),
                },
            )
            .unwrap()
            .column("Title", Fill)
            .column("Artist", Dip(140.0))
            .column("Album", Dip(140.0))
            .column_right("Year", Dip(56.0))
            .multi_select(true)
            .on_selection(|rows| Some(Msg::Select(rows.to_vec())))
            .on_activate(|row| Some(Msg::Play(row)))
            .on_context(|row| Some(Msg::Context(row)))
            .on_sort(|column| Some(Msg::Sort(column)));
            list.set_selection(&[1, 2, 3]);

            let status = StatusBar::new(
                ui,
                Rect::new(p(12.0), p(408.0), p(708.0), p(440.0)),
                &["Ready", "3 selected"],
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
            }

            Library {
                list,
                status,
                tracks,
                order,
                sort: None,
            }
        },
    );
}
