use crate::Color;
use crate::image::Image;
use crate::{Cap, Corner, Dash, GradientStop, LinearGradient, RadialGradient, Rgba, Stroke};

use super::*;
use crate::geometry::Point;
use crate::message::MouseButton;
use crate::router::Router;
use crate::units::dip;

fn node(kind: NodeKind) -> NodeSpec {
    NodeSpec::new(kind, Rect::new(0, 0, 100, 30))
}

#[test]
fn a_painted_node_records_its_drawing() {
    let backend = HeadlessBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("test").size(dip(400.0), dip(300.0)))
        .unwrap();
    let label = backend
        .create(ParentRef::Window(window), &node(NodeKind::Label).text("hi"))
        .unwrap();

    backend.set_painter(
        label,
        Rc::new(|canvas| {
            canvas.clear(Color::rgb(255, 255, 255));
            canvas.fill_rect(Rect::new(4, 4, 40, 24), Color::rgb(0, 0, 0));
        }),
    );
    backend.render(label);

    let ops = backend.ops(label);
    assert_eq!(ops.len(), 2, "clear + fill recorded: {ops:?}");
    assert!(matches!(ops[0], DrawOp::Clear(_)));
    assert!(matches!(ops[1], DrawOp::Fill(_, _)));
}

#[test]
fn a_transform_moves_the_recorded_geometry() {
    let backend = HeadlessBackend::new();
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let node_id = backend
        .create(ParentRef::Window(window), &node(NodeKind::Custom))
        .unwrap();

    backend.set_painter(
        node_id,
        Rc::new(|canvas| {
            canvas.set_translation(0.0, -10.0);
            canvas.fill_rect(Rect::new(0, 10, 20, 20), Color::rgb(1, 2, 3));
        }),
    );
    backend.render(node_id);

    match backend.ops(node_id).as_slice() {
        [DrawOp::Fill(rect, _)] => assert_eq!(*rect, Rect::new(0, 0, 20, 10)),
        other => panic!("unexpected ops: {other:?}"),
    }
}

#[test]
fn a_drawn_image_is_recorded() {
    let backend = HeadlessBackend::new();
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let node_id = backend
        .create(ParentRef::Window(window), &node(NodeKind::Custom))
        .unwrap();

    let image = Image::from_rgba(1, 1, vec![9, 8, 7, 255]).unwrap();
    backend.set_painter(
        node_id,
        Rc::new(move |canvas| {
            canvas.draw_image(&image, Rect::new(0, 5, 10, 15));
        }),
    );
    backend.render(node_id);

    match backend.ops(node_id).as_slice() {
        [DrawOp::Image(rect, recorded)] => {
            assert_eq!(*rect, Rect::new(0, 5, 10, 15));
            assert_eq!(recorded.pixel(0, 0), Some([9, 8, 7, 255]));
        }
        other => panic!("unexpected ops: {other:?}"),
    }
}

#[test]
fn the_rich_shape_api_is_recorded() {
    let backend = HeadlessBackend::new();
    let window = backend.open_window(&PlatformSpec::new("shapes")).unwrap();
    let id = backend
        .create(ParentRef::Window(window), &node(NodeKind::Custom))
        .unwrap();

    backend.set_painter(
        id,
        Rc::new(|canvas| {
            let rect = Rect::new(0, 0, 40, 40);
            let corners = [Corner::uniform(6.0); 4];
            canvas.fill_rect_rgba(rect, Rgba::with_alpha(255, 0, 0, 128));
            canvas.fill_rounded_rect_corners(rect, corners, Rgba::rgb(0, 0, 255));
            canvas.stroke_rounded_rect_corners(
                rect,
                corners,
                Rgba::rgb(0, 0, 0),
                &Stroke::new(2.0).dash(Dash::Dashed),
            );
            canvas.draw_line_stroked(
                Point::new(0, 0),
                Point::new(40, 40),
                Rgba::rgb(0, 255, 0),
                &Stroke::new(1.0),
            );
            canvas.stroke_ellipse_stroked(
                Point::new(20, 20),
                10.0,
                10.0,
                Rgba::rgb(1, 2, 3),
                &Stroke::new(1.0).cap(Cap::Round),
            );
            canvas.fill_rect_linear(
                rect,
                &LinearGradient::new(
                    Point::new(0, 0),
                    Point::new(40, 0),
                    vec![
                        GradientStop::new(0.0, Rgba::rgb(0, 0, 0)),
                        GradientStop::new(1.0, Rgba::rgb(255, 255, 255)),
                    ],
                ),
            );
            canvas.fill_rect_radial(
                rect,
                &RadialGradient::new(
                    Point::new(20, 20),
                    10.0,
                    10.0,
                    vec![
                        GradientStop::new(0.0, Rgba::TRANSPARENT),
                        GradientStop::new(1.0, Rgba::rgb(0, 0, 0)),
                    ],
                ),
            );
            canvas.push_clip_rounded(rect, corners);
            canvas.pop_clip();
        }),
    );
    backend.render(id);

    let ops = backend.ops(id);
    assert_eq!(ops.len(), 9, "every op recorded once: {ops:?}");
    assert!(matches!(ops[0], DrawOp::FillRgba(..)));
    assert!(matches!(ops[1], DrawOp::RoundedCorners(..)));
    assert!(matches!(ops[2], DrawOp::StrokeRoundedCorners(..)));
    assert!(matches!(ops[3], DrawOp::LineStroked(..)));
    assert!(matches!(ops[4], DrawOp::StrokeEllipseStroked(..)));
    assert!(matches!(ops[5], DrawOp::FillLinear(..)));
    assert!(matches!(ops[6], DrawOp::FillRadial(..)));
    assert!(matches!(ops[7], DrawOp::ClipRounded(..)));
    assert!(matches!(ops[8], DrawOp::Unclip));
}

#[test]
fn a_container_owns_its_children_and_destroy_cascades() {
    let backend = HeadlessBackend::new();
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let panel = backend
        .create(ParentRef::Window(window), &node(NodeKind::Panel))
        .unwrap();
    let child = backend
        .create(ParentRef::Widget(panel), &node(NodeKind::Label))
        .unwrap();

    backend.destroy(panel);
    assert!(!backend.has_node(panel));
    assert!(!backend.has_node(child), "the child cascaded");
}

#[test]
fn closing_a_window_drops_its_whole_node_tree() {
    let backend = HeadlessBackend::new();
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let panel = backend
        .create(ParentRef::Window(window), &node(NodeKind::Panel))
        .unwrap();
    let child = backend
        .create(ParentRef::Widget(panel), &node(NodeKind::Label))
        .unwrap();

    backend.close_window(window);
    assert!(!backend.has_node(panel));
    assert!(!backend.has_node(child), "the grandchild went too");
}

#[test]
fn creating_under_a_dead_parent_fails() {
    let backend = HeadlessBackend::new();
    let ghost = WidgetId::from_raw(999);
    assert!(
        backend
            .create(ParentRef::Widget(ghost), &node(NodeKind::Label))
            .is_err()
    );
}

#[test]
fn events_reach_the_installed_sink() {
    use std::cell::Cell;

    let backend = HeadlessBackend::new();
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let label = backend
        .create(ParentRef::Window(window), &node(NodeKind::Label))
        .unwrap();

    let router = Rc::new(Router::new());
    let hits = Rc::new(Cell::new(0));
    let counter = Rc::clone(&hits);
    router.register(label, move |event| {
        if matches!(event, Event::MouseDown { .. }) {
            counter.set(counter.get() + 1);
        }
        true
    });
    backend.set_event_sink(window, router);

    assert!(backend.inject(
        window,
        label,
        Event::MouseDown {
            x: 1,
            y: 2,
            button: MouseButton::Left,
            modifiers: Default::default(),
        }
    ));
    assert_eq!(hits.get(), 1);
}

#[test]
fn moves_and_state_updates_are_recorded() {
    let backend = HeadlessBackend::new();
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let label = backend
        .create(ParentRef::Window(window), &node(NodeKind::Label))
        .unwrap();

    backend.apply_moves(window, &[(label, Rect::new(5, 5, 105, 35))]);
    backend.set_text(label, "changed");
    backend.set_enabled(label, false);
    backend.focus(label);
    backend.invalidate(label);

    let (kind, bounds, text, _, enabled) = backend.node(label).unwrap();
    assert_eq!(kind, NodeKind::Label);
    assert_eq!(bounds, Rect::new(5, 5, 105, 35));
    assert_eq!(text, "changed");
    assert!(!enabled);
    assert_eq!(backend.focused(), Some(label));
    assert_eq!(backend.move_calls(), 1);
    assert_eq!(backend.invalidations(), 1);
}

#[test]
fn window_state_is_recorded() {
    let backend = HeadlessBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("mail").size(dip(320.0), dip(200.0)))
        .unwrap();

    assert_eq!(backend.window_title(window).as_deref(), Some("mail"));
    assert_eq!(backend.client_rect(window), Rect::new(0, 0, 320, 200));
    assert_eq!(backend.dpi(window), 96);

    backend.wake(window);
    backend.wake(window);
    assert_eq!(backend.wakes(window), 2);

    backend.set_theme(window, &Theme::dark());
    assert_eq!(backend.window_theme(window), Some(Theme::dark()));
}

#[test]
fn quitting_is_recorded() {
    let backend = HeadlessBackend::new();
    assert!(!backend.quit_requested());
    backend.quit(0);
    assert!(backend.quit_requested());
}
