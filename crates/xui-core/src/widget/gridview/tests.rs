#![forbid(unsafe_code)]

//! Unit tests for [`GridView`](super::GridView): virtualisation, hit-testing,
//! selection, hover, wheel scrolling and column reflow.

use std::cell::RefCell;
use std::rc::Rc;

use super::model::{GridModel, Tile};
use super::{GridView, TileSize};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::{Point, Rect};
use crate::image::Image;
use crate::message::{Key, Modifiers, MouseButton};
use crate::units::Dip;

struct TestApp(Rc<RefCell<Vec<u32>>>);

impl App for TestApp {
    type Msg = u32;

    fn update(&mut self, msg: u32, _ui: &mut Ui<u32>) {
        self.0.borrow_mut().push(msg);
    }
}

fn setup() -> (Rc<HeadlessBackend>, Rc<Core<u32>>, Ui<u32>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn log() -> Rc<RefCell<Vec<u32>>> {
    Rc::new(RefCell::new(Vec::new()))
}

fn model(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("tile {index}")).collect()
}

fn tiles(width: Dip, height: Dip, gap: Dip) -> TileSize {
    TileSize::new(width, height).gap(gap)
}

fn down(x: i32, y: i32) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn key(key: Key) -> Event {
    Event::KeyDown {
        key,
        modifiers: Modifiers::NONE,
        repeat: 1,
        system: false,
    }
}

fn labels(backend: &HeadlessBackend, id: WidgetId) -> Vec<String> {
    backend
        .ops(id)
        .into_iter()
        .filter_map(|op| match op {
            DrawOp::Text(_, text, _) => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn a_large_model_paints_only_the_visible_tiles() {
    let (backend, _core, ui) = setup();
    let grid = GridView::with_model(&ui, Rect::new(0, 0, 220, 120), model(10_000))
        .unwrap()
        .tile_size(tiles(Dip(100.0), Dip(100.0), Dip(20.0)));

    backend.render(grid.id());
    assert_eq!(labels(&backend, grid.id()), vec!["tile 0", "tile 1"]);
}

#[test]
fn the_wheel_scrolls_the_virtual_window() {
    let (backend, core, ui) = setup();
    let grid = GridView::with_model(&ui, Rect::new(0, 0, 220, 120), model(10_000))
        .unwrap()
        .tile_size(tiles(Dip(100.0), Dip(100.0), Dip(20.0)));
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(
        grid.id(),
        &Event::MouseWheel {
            delta: -1,
            horizontal: false,
            x: 5,
            y: 5,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    backend.render(grid.id());

    assert_eq!(labels(&backend, grid.id()), vec!["tile 6", "tile 7"]);
}

#[test]
fn a_click_hit_tests_selects_and_skips_gaps() {
    let (_backend, core, ui) = setup();
    let log = log();
    let grid = GridView::with_model(&ui, Rect::new(0, 0, 220, 120), model(10))
        .unwrap()
        .tile_size(tiles(Dip(100.0), Dip(100.0), Dip(20.0)))
        .on_select(|index| Some(index as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(grid.id(), &down(10, 10));
    assert_eq!(grid.selected(), Some(0));
    runtime.deliver(grid.id(), &down(130, 10));
    assert_eq!(grid.selected(), Some(1));
    runtime.deliver(grid.id(), &down(115, 10));
    assert_eq!(
        grid.selected(),
        Some(1),
        "a click in the gap selects nothing"
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![0, 1]);
}

#[test]
fn hover_tracks_the_pointer_and_clears_on_leave() {
    let (_backend, core, ui) = setup();
    let grid = GridView::with_model(&ui, Rect::new(0, 0, 220, 120), model(10))
        .unwrap()
        .tile_size(tiles(Dip(100.0), Dip(100.0), Dip(20.0)));
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(
        grid.id(),
        &Event::MouseMove {
            x: 130,
            y: 10,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(grid.state.borrow().hover, Some(1));

    runtime.deliver(grid.id(), &Event::MouseLeave);
    assert_eq!(grid.state.borrow().hover, None);
}

#[test]
fn resizing_reflows_the_columns() {
    let (backend, _core, ui) = setup();
    let grid = GridView::with_model(&ui, Rect::new(0, 0, 320, 100), model(100))
        .unwrap()
        .tile_size(tiles(Dip(100.0), Dip(100.0), Dip(10.0)));

    backend.render(grid.id());
    assert_eq!(labels(&backend, grid.id()).len(), 3);

    grid.control.set_bounds(Rect::new(0, 0, 560, 100));
    backend.render(grid.id());
    assert_eq!(labels(&backend, grid.id()).len(), 5);
}

#[test]
fn a_custom_painter_draws_every_visible_tile() {
    let (backend, _core, ui) = setup();
    let grid = GridView::with_model(&ui, Rect::new(0, 0, 220, 120), model(10))
        .unwrap()
        .tile_size(tiles(Dip(100.0), Dip(100.0), Dip(20.0)))
        .on_paint_tile(|canvas, paint| {
            canvas.fill_ellipse(
                Point::new(paint.rect.left + 2, paint.rect.top + 2),
                2.0,
                2.0,
                paint.theme.accent,
            );
        });

    backend.render(grid.id());
    let ellipses = backend
        .ops(grid.id())
        .iter()
        .filter(|op| matches!(op, DrawOp::Ellipse(..)))
        .count();
    assert_eq!(ellipses, 2);
}

#[test]
fn the_default_painter_draws_the_tiles_image_and_label() {
    struct Art {
        labels: Vec<String>,
        images: Vec<Image>,
    }
    impl GridModel for Art {
        fn len(&self) -> usize {
            self.labels.len()
        }
        fn tile(&self, index: usize) -> Option<Tile<'_>> {
            let label = self.labels.get(index)?;
            let image = self.images.get(index % self.images.len())?;
            Some(Tile::new(label).image(image))
        }
    }

    let (backend, _core, ui) = setup();
    let art = Art {
        labels: vec!["a".into(), "b".into(), "c".into()],
        images: vec![Image::from_rgba(2, 2, vec![255; 16]).unwrap()],
    };
    let grid = GridView::with_model(&ui, Rect::new(0, 0, 220, 120), art)
        .unwrap()
        .tile_size(tiles(Dip(100.0), Dip(100.0), Dip(20.0)));

    backend.render(grid.id());
    let ops = backend.ops(grid.id());
    let images = ops
        .iter()
        .filter(|op| matches!(op, DrawOp::Image(..)))
        .count();
    assert_eq!(images, 2, "one image per visible tile");
    assert_eq!(labels(&backend, grid.id()), vec!["a", "b"]);
}

#[test]
fn arrow_keys_move_the_selection() {
    let (_backend, core, ui) = setup();
    let log = log();
    let grid = GridView::with_model(&ui, Rect::new(0, 0, 220, 120), model(10))
        .unwrap()
        .tile_size(tiles(Dip(100.0), Dip(100.0), Dip(20.0)))
        .on_select(|index| Some(index as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(grid.id(), &key(Key::RIGHT));
    assert_eq!(grid.selected(), Some(1));
    runtime.deliver(grid.id(), &key(Key::DOWN));
    assert_eq!(grid.selected(), Some(3));
    runtime.deliver(grid.id(), &key(Key::UP));
    assert_eq!(grid.selected(), Some(1));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![1, 3, 1]);
}

#[test]
fn an_empty_model_paints_nothing_and_selects_nothing() {
    let (backend, _core, ui) = setup();
    let grid = GridView::new(&ui, Rect::new(0, 0, 100, 100), &[]).unwrap();

    assert!(grid.is_empty());
    assert_eq!(grid.selected(), None);
    backend.render(grid.id());
    assert!(labels(&backend, grid.id()).is_empty());
}
