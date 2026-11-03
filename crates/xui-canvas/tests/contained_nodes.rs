//! Contained nodes (`NodeSpec::contained`) on the offscreen backend: a change to
//! one repaints its bounds and nothing around them, its painter is cut at its
//! bounds, and shapes that miss a repainted rectangle are skipped without
//! changing a pixel of it.

use std::cell::Cell;
use std::rc::Rc;

use xui_canvas::OffscreenBackend;
use xui_core::backend::{
    Backend, NodeKind, NodeSpec, Painter, ParentRef, PlatformSpec, WidgetId, WindowId,
};
use xui_core::geometry::{Point, Rect};
use xui_core::{Color, Dip};

const W: i32 = 200;
const H: i32 = 120;

fn window(backend: &OffscreenBackend) -> WindowId {
    backend
        .open_window(&PlatformSpec::new("contained").size(Dip(W as f32), Dip(H as f32)))
        .unwrap()
}

fn counting(color: Color, runs: &Rc<Cell<u32>>) -> Painter {
    let runs = Rc::clone(runs);
    Rc::new(move |canvas| {
        runs.set(runs.get() + 1);
        let bounds = canvas.bounds();
        canvas.fill_rect(bounds, color);
    })
}

/// Two nodes side by side, 4 px apart: closer than the overdraw margin.
fn pair(contained: bool) -> (OffscreenBackend, WindowId, WidgetId, [Rc<Cell<u32>>; 2]) {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let runs = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    let places = [Rect::new(10, 10, 90, 110), Rect::new(94, 10, 190, 110)];
    let mut first = None;
    for (place, runs) in places.into_iter().zip(&runs) {
        let mut spec = NodeSpec::new(NodeKind::Custom, place);
        if contained {
            spec = spec.contained();
        }
        let id = backend.create(ParentRef::Window(window), &spec).unwrap();
        backend.set_painter(id, counting(Color::rgb(200, 0, 0), runs));
        first.get_or_insert(id);
    }
    backend.render(window).unwrap();
    (backend, window, first.expect("two nodes"), runs)
}

#[test]
fn a_contained_node_repaints_its_bounds_and_not_its_neighbour() {
    let (backend, window, left, runs) = pair(true);
    backend.invalidate(left);
    let damage = backend
        .present(window, |frame| frame.damage.to_vec())
        .unwrap();
    assert_eq!(damage, [Rect::new(10, 10, 90, 110)], "no margin around it");
    assert_eq!(runs[0].get(), 2);
    assert_eq!(runs[1].get(), 1, "the neighbour 4 px away did not repaint");
}

#[test]
fn an_ordinary_node_repaints_its_neighbour_within_the_margin() {
    let (backend, window, left, runs) = pair(false);
    backend.invalidate(left);
    backend.render(window).unwrap();
    assert_eq!(runs[1].get(), 2, "the margin reaches the neighbour");
}

#[test]
fn a_contained_painter_is_cut_at_its_bounds() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let spec = NodeSpec::new(NodeKind::Custom, Rect::new(50, 50, 100, 100)).contained();
    let id = backend.create(ParentRef::Window(window), &spec).unwrap();
    backend.set_painter(
        id,
        Rc::new(|canvas| {
            let b = canvas.bounds();
            // An outline on the edge reaches half its width outside.
            canvas.stroke_rect(b, Color::rgb(0, 0, 0), 3.0);
        }),
    );
    let image = backend.render(window).unwrap();
    let background = image.pixel(5, 5);
    assert_eq!(
        image.pixel(48, 75),
        background,
        "nothing left of the bounds"
    );
    assert_ne!(image.pixel(50, 75), background, "the outline inside them");
}

/// A thick line just outside a repainted rectangle still reaches into it, so
/// it must be drawn there exactly as a whole-window paint draws it.
#[test]
fn a_shape_beside_a_repainted_rectangle_draws_its_edge_inside_it() {
    let paint = |canvas: &mut dyn xui_core::backend::Canvas| {
        canvas.draw_line(
            Point::new(20, 57),
            Point::new(180, 57),
            Color::rgb(0, 0, 0),
            9.0,
        );
        canvas.fill_ellipse(Point::new(100, 75), 12.0, 4.5, Color::rgb(0, 90, 0));
    };
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let runs = Rc::new(Cell::new(0));
    let id = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, W, H)),
        )
        .unwrap();
    let seen = Rc::clone(&runs);
    backend.set_painter(
        id,
        Rc::new(move |canvas| {
            seen.set(seen.get() + 1);
            if seen.get() > 1 {
                paint(canvas);
            }
        }),
    );
    backend.render(window).unwrap();
    // The line's path is 3 px above the rectangle, its stroke 4.5 px either side;
    // the ellipse ends 0.5 px inside it.
    let area = Rect::new(60, 60, 140, 70);
    backend.invalidate_rect(id, area);
    let repainted = backend.render(window).unwrap();

    let whole = OffscreenBackend::new();
    let other = self::window(&whole);
    let id = whole
        .create(
            ParentRef::Window(other),
            &NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, W, H)),
        )
        .unwrap();
    whole.set_painter(id, Rc::new(move |canvas| paint(canvas)));
    let reference = whole.render(other).unwrap();
    let mut touched = 0;
    for y in area.top as u32..area.bottom as u32 {
        for x in area.left as u32..area.right as u32 {
            let (got, want) = (repainted.pixel(x, y), reference.pixel(x, y));
            assert_eq!(got, want, "({x}, {y})");
            touched += usize::from(want != reference.pixel(5, 100));
        }
    }
    assert!(touched > 0, "the shapes do reach into the rectangle");
}

#[test]
fn an_ordinary_neighbour_still_repaints_a_contained_node_it_may_draw_on() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let (runs, other_runs) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let spec = NodeSpec::new(NodeKind::Custom, Rect::new(10, 10, 90, 110)).contained();
    let contained = backend.create(ParentRef::Window(window), &spec).unwrap();
    backend.set_painter(contained, counting(Color::rgb(200, 0, 0), &runs));
    let spec = NodeSpec::new(NodeKind::Custom, Rect::new(94, 10, 190, 110));
    let ordinary = backend.create(ParentRef::Window(window), &spec).unwrap();
    backend.set_painter(ordinary, counting(Color::rgb(0, 0, 200), &other_runs));
    backend.render(window).unwrap();
    backend.invalidate(ordinary);
    backend.render(window).unwrap();
    assert_eq!(runs.get(), 2, "the neighbour's margin reaches it");
}

#[test]
fn a_contained_container_keeps_the_margin_for_its_children() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let spec = NodeSpec::new(NodeKind::Custom, Rect::new(40, 20, 120, 100)).contained();
    let container = backend.create(ParentRef::Window(window), &spec).unwrap();
    backend.set_painter(
        container,
        Rc::new(|canvas| canvas.clear(Color::rgb(9, 9, 9))),
    );
    let spec = NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 20, 20));
    let child = backend.create(ParentRef::Widget(container), &spec).unwrap();
    backend.set_painter(child, Rc::new(|canvas| canvas.clear(Color::rgb(90, 9, 9))));
    backend.render(window).unwrap();
    backend.invalidate(container);
    let damage = backend
        .present(window, |frame| frame.damage.to_vec())
        .unwrap();
    assert_eq!(damage.len(), 1);
    assert!(
        damage[0].left < 40 && damage[0].top < 20,
        "its child may draw past it: {damage:?}"
    );
}
