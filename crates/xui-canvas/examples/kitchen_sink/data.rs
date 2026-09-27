//! Mock data for the portable kitchen-sink demo: a track library for the
//! virtual `ListView`, album art for the `GridView`, and a folder list.

use std::rc::Rc;

use xui_core::image::Image;
use xui_core::widget::{GridModel, ListModel, Tile};

/// One library row; numeric/short columns are pre-formatted so the model can
/// borrow `&str`.
pub(crate) struct Track {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) genre: String,
    pub(crate) year: String,
    pub(crate) time: String,
    pub(crate) format: String,
    pub(crate) plays: String,
}

/// A virtual list model over a shared track store plus the display order, so a
/// sort is an `order` permutation and does not clone rows.
pub(crate) struct TrackModel {
    tracks: Rc<Vec<Track>>,
    order: Vec<usize>,
}

impl TrackModel {
    pub(crate) fn new(tracks: Rc<Vec<Track>>, order: Vec<usize>) -> TrackModel {
        TrackModel { tracks, order }
    }
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
            3 => track.genre.as_str(),
            4 => track.year.as_str(),
            5 => track.time.as_str(),
            6 => track.format.as_str(),
            7 => track.plays.as_str(),
            _ => "",
        })
    }
}

/// An album tile: its caption and a raster cover.
pub(crate) struct Album {
    pub(crate) caption: String,
    pub(crate) art: Image,
}

/// A virtual grid model over a shared album store.
pub(crate) struct AlbumModel {
    albums: Rc<Vec<Album>>,
}

impl AlbumModel {
    pub(crate) fn new(albums: Rc<Vec<Album>>) -> AlbumModel {
        AlbumModel { albums }
    }
}

impl GridModel for AlbumModel {
    fn len(&self) -> usize {
        self.albums.len()
    }

    fn tile(&self, index: usize) -> Option<Tile<'_>> {
        let album = self.albums.get(index)?;
        Some(Tile::new(&album.caption).image(&album.art))
    }
}

pub(crate) fn tracks(count: usize) -> Vec<Track> {
    const GENRES: [&str; 8] = [
        "Ambient",
        "Blues",
        "Chiptune",
        "Classical",
        "Jazz",
        "Rock",
        "Pop",
        "Electronic",
    ];
    const FORMATS: [&str; 4] = ["mp3", "flac", "ogg", "wav"];
    (0..count)
        .map(|index| Track {
            title: format!("Track {}", index + 1),
            artist: format!("Artist {}", index % 37),
            album: format!("Album {}", index % 13),
            genre: GENRES[index % GENRES.len()].to_string(),
            year: format!("{}", 1950 + index % 75),
            time: format!("{}:{:02}", 1 + index % 9, index % 60),
            format: FORMATS[index % FORMATS.len()].to_string(),
            plays: format!("{}", (index * 7) % 400),
        })
        .collect()
}

pub(crate) fn albums(count: usize) -> Vec<Album> {
    (0..count)
        .map(|index| Album {
            caption: format!("Album {}\nArtist {}", index + 1, index % 37),
            art: cover(index),
        })
        .collect()
}

pub(crate) fn folders() -> [&'static str; 5] {
    [
        r"D:\Music\Tracker",
        r"D:\Music\Library",
        r"D:\Music\Imports",
        r"D:\Music\Archive",
        r"D:\Music\Samples",
    ]
}

/// A `size` x `size` cover: a two-tone diagonal gradient tinted per album, so a
/// tile is visually distinct and the image path is exercised.
fn cover(index: usize) -> Image {
    const SIZE: u32 = 64;
    let hue = [
        (index * 37 % 256) as u8,
        (index * 91 % 256) as u8,
        (index * 53 % 256) as u8,
    ];
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let t = ((x + y) as f32 / (2 * SIZE) as f32 * 255.0) as u8;
            pixels.extend_from_slice(&[
                hue[0].saturating_add(t / 2),
                hue[1].saturating_add(t / 2),
                hue[2].saturating_add(t / 2),
                255,
            ]);
        }
    }
    Image::from_rgba(SIZE, SIZE, pixels).expect("64x64 RGBA cover")
}
