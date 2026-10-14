//! What the offscreen backend promises about frames and damage, through its
//! public API: an idle frame does nothing, a damaged one repaints only what
//! changed (and the whole window when it must), a painter that re-enters the
//! backend is safe, and one that draws further out than the overdraw margin
//! can say so.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use xui_canvas::OffscreenBackend;
use xui_core::backend::{
    Backend, NodeKind, NodeSpec, Painter, ParentRef, PlatformSpec, WidgetId, WindowId,
};
use xui_core::geometry::{Point, Rect};
use xui_core::{Color, Dip, Theme};

const W: i32 = 400;
const H: i32 = 300;

fn window(backend: &OffscreenBackend) -> WindowId {
    backend
        .open_window(&PlatformSpec::new("damage").size(Dip(W as f32), Dip(H as f32)))
        .unwrap()
}

fn node(backend: &OffscreenBackend, window: WindowId, rect: Rect) -> WidgetId {
    backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, rect),
        )
        .unwrap()
}

fn fill(color: Color, runs: &Rc<Cell<u32>>) -> Painter {
    let runs = Rc::clone(runs);
    Rc::new(move |canvas| {
        runs.set(runs.get() + 1);
        let bounds = canvas.bounds();
        canvas.fill_rect(bounds, color);
    })
}

/// The frame's damage after the previous call, copying nothing else.
fn damage(backend: &OffscreenBackend, window: WindowId) -> Vec<Rect> {
    backend
        .present(window, |frame| frame.damage.to_vec())
        .expect("a frame")
}

#[test]
fn the_first_frame_paints_the_whole_window_and_an_idle_one_nothing() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let runs = Rc::new(Cell::new(0));
    let id = node(&backend, window, Rect::new(10, 10, 60, 40));
    backend.set_painter(id, fill(Color::rgb(200, 0, 0), &runs));

    assert_eq!(damage(&backend, window), [Rect::new(0, 0, W, H)]);
    assert_eq!(runs.get(), 1);

    let first = backend
        .present(window, |frame| frame.pixels.as_ptr() as usize)
        .unwrap();
    for _ in 0..20 {
        let (rects, at) = backend
            .present(window, |frame| {
                (frame.damage.len(), frame.pixels.as_ptr() as usize)
            })
            .unwrap();
        assert_eq!(rects, 0, "nothing changed");
        assert_eq!(at, first, "the same persistent surface, never a copy");
    }
    assert_eq!(runs.get(), 1, "an idle frame runs no painter");
}

#[test]
fn render_still_returns_an_independent_copy_of_the_frame() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let runs = Rc::new(Cell::new(0));
    let id = node(&backend, window, Rect::new(10, 10, 60, 40));
    backend.set_painter(id, fill(Color::rgb(200, 0, 0), &runs));

    let before = backend.render(window).unwrap();
    backend.set_visible(id, false);
    let after = backend.render(window).unwrap();
    assert_eq!(before.pixel(20, 20), Some([200, 0, 0, 255]));
    assert_ne!(after.pixel(20, 20), Some([200, 0, 0, 255]));
    assert_eq!(
        before.pixel(20, 20),
        Some([200, 0, 0, 255]),
        "an earlier image does not follow the surface"
    );
}

#[test]
fn a_change_repaints_only_what_it_touched() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let (left_runs, right_runs) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let left = node(&backend, window, Rect::new(10, 10, 60, 40));
    let right = node(&backend, window, Rect::new(300, 200, 360, 260));
    backend.set_painter(left, fill(Color::rgb(200, 0, 0), &left_runs));
    backend.set_painter(right, fill(Color::rgb(0, 0, 200), &right_runs));
    backend.render(window).unwrap();

    backend.apply_moves(window, &[(left, Rect::new(12, 10, 62, 40))]);
    let rects = damage(&backend, window);
    assert_eq!(rects.len(), 1, "a nudge is one rectangle: {rects:?}");
    assert!(rects[0].width() < 100 && rects[0].height() < 80);
    assert_eq!(left_runs.get(), 2, "the moved node repainted");
    assert_eq!(right_runs.get(), 1, "its distant neighbour did not");
}

#[test]
fn invalidate_rect_is_honoured_and_a_full_invalidate_repaints_everything() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let runs = Rc::new(Cell::new(0));
    let id = node(&backend, window, Rect::new(100, 100, 300, 250));
    backend.set_painter(id, fill(Color::rgb(0, 150, 0), &runs));
    backend.render(window).unwrap();

    // A rectangle in the node's own coordinates: (10, 10)..(30, 20) is
    // (110, 110)..(130, 120) in the window.
    backend.invalidate_rect(id, Rect::new(10, 10, 30, 20));
    let rects = damage(&backend, window);
    assert_eq!(rects.len(), 1);
    let rect = rects[0];
    assert!(
        rect.left <= 110 && rect.top <= 110 && rect.right >= 130 && rect.bottom >= 120,
        "it covers the rectangle: {rect:?}"
    );
    assert!(
        rect.width() < 60 && rect.height() < 40,
        "and not the node's whole area: {rect:?}"
    );

    backend.invalidate_window(window);
    assert_eq!(damage(&backend, window), [Rect::new(0, 0, W, H)]);

    backend.set_theme(window, &Theme::dark());
    assert_eq!(
        damage(&backend, window),
        [Rect::new(0, 0, W, H)],
        "a theme change is a full repaint"
    );
}

#[test]
fn many_tiny_damages_merge_and_the_list_stays_short() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let runs = Rc::new(Cell::new(0));
    let id = node(&backend, window, Rect::new(0, 0, W, H));
    backend.set_painter(id, fill(Color::rgb(10, 10, 10), &runs));
    backend.render(window).unwrap();

    // 2000 one-pixel damages scattered over the window.
    for step in 0..2000 {
        let x = (step * 37) % (W - 4);
        let y = (step * 53) % (H - 4);
        backend.invalidate_rect(id, Rect::new(x, y, x + 1, y + 1));
    }
    let rects = damage(&backend, window);
    assert!(
        !rects.is_empty() && rects.len() <= 8,
        "bounded by the merge rule: {} rectangles",
        rects.len()
    );
}

/// Two backends given the same operations: `a` repaints from damage only,
/// `b` repaints the whole window before each frame.
struct Pair {
    a: OffscreenBackend,
    b: OffscreenBackend,
    wa: WindowId,
    wb: WindowId,
}

impl Pair {
    fn new(dpi: u32) -> Pair {
        let spec = PlatformSpec::new("pair").size(Dip(W as f32), Dip(H as f32));
        let a = OffscreenBackend::with_dpi(dpi);
        let b = OffscreenBackend::with_dpi(dpi);
        let (wa, wb) = (a.open_window(&spec).unwrap(), b.open_window(&spec).unwrap());
        Pair { a, b, wa, wb }
    }

    /// A node on the window (or in `parent`) with `painter`, in both.
    fn node(&self, parent: Option<WidgetId>, rect: Rect, painter: &Painter) -> WidgetId {
        let mut made = None;
        for (backend, window) in [(&self.a, self.wa), (&self.b, self.wb)] {
            let parent = parent.map_or(ParentRef::Window(window), ParentRef::Widget);
            let id = backend
                .create(parent, &NodeSpec::new(NodeKind::Custom, rect))
                .unwrap();
            backend.set_painter(id, Rc::clone(painter));
            made = Some(id);
        }
        made.unwrap()
    }

    fn each(&self, mut f: impl FnMut(&OffscreenBackend, WindowId)) {
        f(&self.a, self.wa);
        f(&self.b, self.wb);
    }

    /// Whether the damage frame equals the whole-window frame now.
    fn same(&self) -> bool {
        let got = self.a.render(self.wa).unwrap();
        self.b.invalidate_window(self.wb);
        got.pixels == self.b.render(self.wb).unwrap().pixels
    }
}

/// A painter that draws a ring `reach` pixels outside its node.
fn ring(reach: i32) -> Painter {
    Rc::new(move |canvas| {
        let bounds = canvas.bounds();
        canvas.fill_rect(bounds, Color::rgb(30, 30, 30));
        canvas.stroke_rect(
            Rect::new(
                bounds.left - reach,
                bounds.top - reach,
                bounds.right + reach,
                bounds.bottom + reach,
            ),
            Color::rgb(0, 100, 255),
            2.0,
        );
    })
}

/// A node moved away must not leave its overdraw behind.
fn moving_leaves_nothing_behind(dpi: u32, reach: i32) -> bool {
    let pair = Pair::new(dpi);
    let id = pair.node(None, Rect::new(100, 100, 140, 130), &ring(reach));
    assert!(pair.same());
    for to in [Rect::new(250, 150, 290, 180), Rect::new(-10, -10, 30, 20)] {
        pair.each(|backend, window| backend.apply_moves(window, &[(id, to)]));
        if !pair.same() {
            return false;
        }
    }
    true
}

#[test]
fn overdraw_within_the_margin_leaves_no_stale_pixels() {
    for reach in 0..=6 {
        assert!(
            moving_leaves_nothing_behind(96, reach),
            "a ring {reach} px outside the node is cleaned up"
        );
    }
}

/// A ring of 6 design units plus its stroke, centred on the bounds edge, as
/// the colour panel handles draw: whole at every scale, and cleaned up.
#[test]
fn overdraw_in_design_units_is_inside_the_margin_at_every_dpi() {
    for dpi in [96u32, 120, 144, 168, 192, 288] {
        let reach = (6 * dpi).div_ceil(96) as i32;
        assert!(
            moving_leaves_nothing_behind(dpi, reach),
            "{dpi} DPI: a ring {reach} px out is cleaned up"
        );
        // And it is drawn whole: its outer edge is inside the reach.
        let backend = OffscreenBackend::with_dpi(dpi);
        let window = window(&backend);
        let id = node(&backend, window, Rect::new(100, 100, 140, 130));
        backend.set_painter(id, ring(reach));
        let image = backend.render(window).unwrap();
        let at = image.pixel((100 - reach) as u32, 115).unwrap();
        assert_eq!(at, [0, 100, 255, 255], "{dpi} DPI: the ring left edge");
    }
}

/// A painter cannot draw outside its reach, in a frame of damage or of
/// everything: a ring 20 px out is cut to the bounds and the margin.
#[test]
fn a_painter_is_clipped_to_its_reach() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let id = node(&backend, window, Rect::new(100, 100, 140, 130));
    backend.set_painter(id, ring(20));
    let image = backend.render(window).unwrap();
    let background = image.pixel(5, 5).unwrap();
    for x in [79, 85, 91] {
        // 100 - 20 is the ring; 100 - 8 is the edge of the reach.
        assert_eq!(image.pixel(x, 115), Some(background), "x = {x}");
    }
    let pair = Pair::new(96);
    let id = pair.node(None, Rect::new(100, 100, 140, 130), &ring(20));
    pair.each(|backend, window| {
        backend.apply_moves(window, &[(id, Rect::new(200, 100, 240, 130))])
    });
    assert!(pair.same(), "a moved ring leaves nothing, in either path");
}

/// Outlines, an ellipse and a line crossing the clip of their container are
/// cut by it; nothing is drawn above, below or beside the viewport.
#[test]
fn strokes_and_lines_are_cut_by_the_clip_of_their_container() {
    let strokes: Painter = Rc::new(|canvas| {
        let bounds = canvas.bounds();
        canvas.stroke_rect(bounds, Color::rgb(200, 0, 0), 3.0);
        canvas.stroke_ellipse(
            Point::new(
                (bounds.left + bounds.right) / 2,
                (bounds.top + bounds.bottom) / 2,
            ),
            (bounds.width() / 2) as f32,
            (bounds.height() / 2) as f32,
            Color::rgb(0, 0, 0),
            3.0,
        );
        canvas.draw_line(
            Point::new(bounds.left, bounds.bottom),
            Point::new(bounds.right, bounds.top),
            Color::rgb(0, 160, 0),
            4.0,
        );
    });
    let nothing: Painter = Rc::new(|_| {});
    // The container is at (60, 60); its clip is (40, 40)..(140, 100) in its
    // own coordinates, so the viewport is this in the window.
    let viewport = Rect::new(100, 100, 200, 160);

    let pair = Pair::new(96);
    let container = pair.node(None, Rect::new(60, 60, 260, 220), &nothing);
    pair.each(|backend, _| backend.set_clip(container, Some(Rect::new(40, 40, 140, 100))));
    let child = pair.node(Some(container), Rect::new(10, 10, 190, 150), &strokes);
    assert!(pair.same());

    // Nothing outside the viewport: the frame equals one without the child.
    let with = pair.a.render(pair.wa).unwrap();
    pair.each(|backend, _| backend.set_visible(child, false));
    let without = pair.a.render(pair.wa).unwrap();
    for y in 0..H as u32 {
        for x in 0..W as u32 {
            let inside = (viewport.left..viewport.right).contains(&(x as i32))
                && (viewport.top..viewport.bottom).contains(&(y as i32));
            if !inside {
                assert_eq!(with.pixel(x, y), without.pixel(x, y), "({x}, {y})");
            }
        }
    }
    pair.each(|backend, _| backend.set_visible(child, true));
    // Scrolling the child: every frame equals a whole repaint.
    for dy in [-30, -90, -140, 5] {
        pair.each(|backend, window| {
            backend.apply_moves(window, &[(child, Rect::new(10, 10 + dy, 190, 150 + dy))])
        });
        assert!(pair.same(), "scrolled by {dy}");
    }
}

/// Nodes whose size becomes nothing along one axis or both still repaint
/// like a whole frame with painters that draw a line, a ring, a polygon and
/// a caret, which is what is left of a collapsed widget.
#[test]
fn a_node_resized_to_nothing_repaints_like_a_whole_frame() {
    let line: Painter = Rc::new(|canvas| {
        let b = canvas.bounds();
        canvas.draw_line(
            Point::new(b.left, b.top),
            Point::new(b.right, b.bottom),
            Color::rgb(200, 0, 0),
            3.0,
        );
    });
    let polygon: Painter = Rc::new(|canvas| {
        let b = canvas.bounds();
        canvas.fill_polygon(
            &[
                Point::new(b.left, b.bottom),
                Point::new(b.right, b.top + 4),
                Point::new(b.left + b.width() / 3, b.top),
            ],
            Color::rgb(0, 0, 200),
        );
    });
    let caret: Painter = Rc::new(|canvas| {
        let b = canvas.bounds();
        canvas.fill_rect(
            Rect::new(b.left + 2, b.top - 4, b.left + 4, b.bottom + 4),
            Color::rgb(10, 10, 10),
        );
    });
    for painter in [line, ring(5), polygon, caret] {
        let pair = Pair::new(96);
        let id = pair.node(None, Rect::new(100, 100, 150, 130), &painter);
        assert!(pair.same());
        for (width, height) in [(0, 30), (50, 0), (0, 0), (50, 30), (0, 30), (7, 0)] {
            let to = Rect::new(100, 100, 100 + width, 100 + height);
            pair.each(|backend, window| backend.apply_moves(window, &[(id, to)]));
            assert!(pair.same(), "{width}x{height}");
        }
    }
}

#[test]
fn destroying_and_creating_again_leaves_no_stale_pixels() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let runs = Rc::new(Cell::new(0));
    let first = node(&backend, window, Rect::new(50, 50, 150, 120));
    backend.set_painter(first, fill(Color::rgb(220, 0, 0), &runs));
    assert_eq!(
        backend.render(window).unwrap().pixel(60, 60),
        Some([220, 0, 0, 255])
    );

    backend.destroy(first);
    let background = backend.render(window).unwrap().pixel(60, 60).unwrap();
    assert_ne!(background, [220, 0, 0, 255], "the destroyed node is gone");

    let second = node(&backend, window, Rect::new(100, 80, 200, 150));
    backend.set_painter(second, fill(Color::rgb(0, 0, 220), &runs));
    let image = backend.render(window).unwrap();
    assert_eq!(image.pixel(60, 60), Some(background), "no remnant");
    assert_eq!(image.pixel(110, 90), Some([0, 0, 220, 255]));
}

#[test]
fn a_node_partly_outside_the_window_repaints_like_a_whole_frame() {
    let backend = OffscreenBackend::new();
    let reference = OffscreenBackend::new();
    let (w, wr) = (window(&backend), window(&reference));
    let runs = Rc::new(Cell::new(0));
    let id = node(&backend, w, Rect::new(-30, -20, 40, 30));
    let id_r = node(&reference, wr, Rect::new(-30, -20, 40, 30));
    backend.set_painter(id, fill(Color::rgb(0, 160, 0), &runs));
    reference.set_painter(id_r, fill(Color::rgb(0, 160, 0), &runs));
    backend.render(w).unwrap();

    for rect in [
        Rect::new(W - 20, H - 10, W + 50, H + 40),
        Rect::new(-100, -100, -40, -40),
        Rect::new(10, H - 30, 70, H + 5),
    ] {
        backend.apply_moves(w, &[(id, rect)]);
        reference.apply_moves(wr, &[(id_r, rect)]);
        assert_eq!(
            backend.render(w).unwrap().pixels,
            reference.render(wr).unwrap().pixels,
            "{rect:?}"
        );
    }
}

/// A painter that asks for another node to be repainted while it paints.
#[test]
fn a_painter_that_invalidates_during_paint_is_repainted_next_frame() {
    let backend = Rc::new(OffscreenBackend::new());
    let window = window(&backend);
    let other_runs = Rc::new(Cell::new(0));
    let a = node(&backend, window, Rect::new(10, 10, 50, 50));
    let b = node(&backend, window, Rect::new(200, 100, 260, 160));
    backend.set_painter(b, fill(Color::rgb(0, 0, 200), &other_runs));

    let weak: Weak<OffscreenBackend> = Rc::downgrade(&backend);
    let fired = Rc::new(Cell::new(false));
    let fired_in = Rc::clone(&fired);
    backend.set_painter(
        a,
        Rc::new(move |canvas| {
            let bounds = canvas.bounds();
            canvas.fill_rect(bounds, Color::rgb(200, 0, 0));
            if !fired_in.replace(true)
                && let Some(backend) = weak.upgrade()
            {
                // Re-enters the backend: reads, and asks for another repaint.
                let _ = backend.bounds(b);
                backend.invalidate(b);
            }
        }),
    );

    backend.render(window).unwrap();
    assert_eq!(other_runs.get(), 1);
    let rects = damage(&backend, window);
    assert_eq!(
        rects.len(),
        1,
        "the invalidation made during the paint was kept: {rects:?}"
    );
    assert_eq!(other_runs.get(), 2, "and painted on the next frame");
    assert!(damage(&backend, window).is_empty(), "then nothing is owed");
}

/// A painter that creates and paints a node during a frame.
#[test]
fn a_node_created_during_a_frame_shows_in_the_next_one() {
    let backend = Rc::new(OffscreenBackend::new());
    let window = window(&backend);
    let created: Rc<RefCell<Option<WidgetId>>> = Rc::new(RefCell::new(None));
    let host = node(&backend, window, Rect::new(10, 10, 50, 50));
    let weak = Rc::downgrade(&backend);
    let created_in = Rc::clone(&created);
    backend.set_painter(
        host,
        Rc::new(move |canvas| {
            let bounds = canvas.bounds();
            canvas.fill_rect(bounds, Color::rgb(90, 90, 90));
            if created_in.borrow().is_none()
                && let Some(backend) = weak.upgrade()
            {
                let id = backend
                    .create(
                        ParentRef::Window(window),
                        &NodeSpec::new(NodeKind::Custom, Rect::new(100, 100, 160, 140)),
                    )
                    .unwrap();
                backend.set_painter(
                    id,
                    Rc::new(|canvas| {
                        let bounds = canvas.bounds();
                        canvas.fill_rect(bounds, Color::rgb(0, 200, 0));
                    }),
                );
                *created_in.borrow_mut() = Some(id);
            }
        }),
    );

    let first = backend.render(window).unwrap();
    assert_ne!(
        first.pixel(110, 110),
        Some([0, 200, 0, 255]),
        "created too late for this frame"
    );
    let second = backend.render(window).unwrap();
    assert_eq!(second.pixel(110, 110), Some([0, 200, 0, 255]));
}

#[test]
fn a_zero_sized_and_an_off_window_node_cost_nothing_and_break_nothing() {
    let backend = OffscreenBackend::new();
    let window = window(&backend);
    let runs = Rc::new(Cell::new(0));
    let empty = node(&backend, window, Rect::new(30, 30, 30, 30));
    let outside = node(&backend, window, Rect::new(W + 10, H + 10, W + 60, H + 60));
    backend.set_painter(empty, fill(Color::rgb(1, 2, 3), &runs));
    backend.set_painter(outside, fill(Color::rgb(1, 2, 3), &runs));
    backend.render(window).unwrap();
    backend.apply_moves(window, &[(empty, Rect::new(40, 40, 40, 40))]);
    backend.invalidate(outside);
    assert!(backend.render(window).is_some());
}
