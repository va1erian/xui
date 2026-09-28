//! Unit tests for the offscreen backend: per-node clipping, hit-testing and
//! layout. Pointer capture has its own file, [`super::capture_tests`].

use std::cell::RefCell;
use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{
    Backend, Event, NodeKind, NodeSpec, Painter, ParentRef, PlatformSpec, WidgetId,
};
use xui_core::message::{Modifiers, MouseButton};
use xui_core::widget::{Label, ScrollView};
use xui_core::{Color, Dip, Rect, Theme};

use super::OffscreenBackend;
use crate::tests::save;

/// A sink that records every event it is handed.
pub(super) struct Recorder(pub(super) Rc<RefCell<Vec<(WidgetId, Event)>>>);

impl xui_core::router::WidgetHost for Recorder {
    fn deliver(&self, target: WidgetId, event: &Event) -> bool {
        self.0.borrow_mut().push((target, *event));
        true
    }
}

fn red_pixels(image: &crate::RgbaImage) -> usize {
    image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] > 200 && pixel[1] < 80 && pixel[2] < 80)
        .count()
}

fn blue_pixels(image: &crate::RgbaImage) -> usize {
    image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] < 80 && pixel[1] < 80 && pixel[2] > 200)
        .count()
}

/// A painter that fills the node's bounds with a solid colour.
fn fill(color: Color) -> Painter {
    Rc::new(move |canvas| {
        let bounds = canvas.bounds();
        canvas.fill_rect(bounds, color);
    })
}

#[test]
fn a_nested_child_paints_at_its_parents_offset() {
    let backend = OffscreenBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("nested").size(Dip(100.0), Dip(100.0)))
        .unwrap();
    let parent = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(20, 30, 90, 80)),
        )
        .unwrap();
    let child = backend
        .create(
            ParentRef::Widget(parent),
            &NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 10, 10)),
        )
        .unwrap();
    backend.set_painter(
        child,
        Rc::new(|canvas| {
            let bounds = canvas.bounds();
            canvas.fill_rect(bounds, Color::rgb(255, 0, 0));
        }),
    );

    let image = backend.render(window).expect("a rendered window");
    assert_eq!(
        image.pixel(25, 35),
        Some([255, 0, 0, 255]),
        "the child paints at the parent's offset"
    );
    assert_ne!(
        image.pixel(5, 5),
        Some([255, 0, 0, 255]),
        "not at its parent-relative position"
    );
}

#[test]
fn a_painter_capturing_its_own_window_does_not_recurse() {
    let backend = Rc::new(OffscreenBackend::new());
    let window = backend
        .open_window(&PlatformSpec::new("reentrant capture"))
        .unwrap();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 10, 10)),
        )
        .unwrap();
    // The painter asks the backend to capture the very window being painted; the
    // nested render must be refused, not recurse until the stack overflows.
    let nested = Rc::new(RefCell::new(None));
    let backend_for_painter = Rc::clone(&backend);
    let nested_for_painter = Rc::clone(&nested);
    backend.set_painter(
        node,
        Rc::new(move |_canvas| {
            *nested_for_painter.borrow_mut() = Some(backend_for_painter.capture(window).is_err());
        }),
    );

    let image = backend.render(window);
    assert!(image.is_some(), "the outer render completes");
    assert_eq!(
        *nested.borrow(),
        Some(true),
        "the nested capture is refused"
    );
}

#[test]
fn a_nested_child_is_hit_at_its_absolute_position() {
    let backend = OffscreenBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("nested hit"))
        .unwrap();
    let parent = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(20, 30, 60, 40)),
        )
        .unwrap();
    let child = backend
        .create(
            ParentRef::Widget(parent),
            &NodeSpec::new(NodeKind::Container, Rect::new(5, 5, 10, 10)),
        )
        .unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    backend.set_event_sink(window, Rc::new(Recorder(Rc::clone(&log))));

    let consumed = backend.inject(
        window,
        Event::MouseDown {
            x: 27,
            y: 37,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );

    assert!(consumed, "the child consumes the click");
    assert!(
        matches!(
            log.borrow().last(),
            Some((target, Event::MouseDown { x: 2, y: 2, .. })) if *target == child
        ),
        "the click targets the child at (25, 35) and is translated locally"
    );
}

#[test]
fn a_child_under_a_hidden_ancestor_is_not_hit() {
    let backend = OffscreenBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("hidden hit"))
        .unwrap();
    let parent = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(20, 30, 60, 40)),
        )
        .unwrap();
    let _child = backend
        .create(
            ParentRef::Widget(parent),
            &NodeSpec::new(NodeKind::Container, Rect::new(5, 5, 10, 10)),
        )
        .unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    backend.set_event_sink(window, Rc::new(Recorder(Rc::clone(&log))));
    backend.set_visible(parent, false);

    let consumed = backend.inject(
        window,
        Event::MouseDown {
            x: 27,
            y: 37,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );

    assert!(!consumed, "no node is hit");
    assert!(
        log.borrow().is_empty(),
        "the hidden subtree receives nothing"
    );
}

#[test]
fn a_clip_hides_an_overflowing_childs_painting() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("clip")).unwrap();
    let view = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(0, 0, 100, 80)),
        )
        .unwrap();
    let child = backend
        .create(
            ParentRef::Widget(view),
            &NodeSpec::new(NodeKind::Container, Rect::new(0, 50, 100, 60)),
        )
        .unwrap();
    backend.set_painter(
        child,
        Rc::new(|canvas| {
            let bounds = canvas.bounds();
            canvas.fill_rect(bounds, Color::rgb(255, 0, 0));
        }),
    );
    // The view clips its descendants to a 10px band at the top; the child
    // starts below it, so nothing paints.
    backend.set_clip(view, Some(Rect::new(0, 0, 100, 10)));

    let image = backend.render(window).expect("a rendered window");
    assert_eq!(red_pixels(&image), 0, "the child is fully clipped away");
}

#[test]
fn a_clip_keeps_the_visible_part_of_a_child() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("clip")).unwrap();
    let view = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(0, 0, 100, 80)),
        )
        .unwrap();
    let child = backend
        .create(
            ParentRef::Widget(view),
            &NodeSpec::new(NodeKind::Container, Rect::new(0, 5, 100, 60)),
        )
        .unwrap();
    backend.set_painter(
        child,
        Rc::new(|canvas| {
            let bounds = canvas.bounds();
            canvas.fill_rect(bounds, Color::rgb(255, 0, 0));
        }),
    );
    backend.set_clip(view, Some(Rect::new(0, 0, 100, 10)));

    let image = backend.render(window).expect("a rendered window");
    let red = red_pixels(&image);
    assert!(red > 0, "the visible band is painted");
    assert_eq!(
        red,
        100 * 5,
        "only the 5px inside the clip is painted, not the 55 below"
    );
}

#[test]
fn a_hidden_panel_hides_its_painted_child() {
    let backend = OffscreenBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("visibility"))
        .unwrap();
    let panel = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(0, 0, 100, 80)),
        )
        .unwrap();
    let child = backend
        .create(
            ParentRef::Widget(panel),
            &NodeSpec::new(NodeKind::Container, Rect::new(10, 10, 30, 30)),
        )
        .unwrap();
    backend.set_painter(child, fill(Color::rgb(255, 0, 0)));
    // Hiding the panel must skip its whole subtree.
    backend.set_visible(panel, false);

    let image = backend.render(window).expect("a rendered window");
    assert_eq!(
        red_pixels(&image),
        0,
        "the child of a hidden panel is not painted"
    );
}

#[test]
fn a_visible_panel_with_a_hidden_child_paints_only_the_panel() {
    let backend = OffscreenBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("visibility"))
        .unwrap();
    let panel = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(0, 0, 100, 80)),
        )
        .unwrap();
    let child = backend
        .create(
            ParentRef::Widget(panel),
            &NodeSpec::new(NodeKind::Container, Rect::new(10, 10, 30, 30)),
        )
        .unwrap();
    backend.set_painter(panel, fill(Color::rgb(255, 0, 0)));
    backend.set_painter(child, fill(Color::rgb(0, 0, 255)));
    backend.set_visible(child, false);

    let image = backend.render(window).expect("a rendered window");
    assert_eq!(red_pixels(&image), 100 * 80, "the panel itself is painted");
    assert_eq!(blue_pixels(&image), 0, "its hidden child is skipped");
}

#[test]
fn an_unhidden_child_under_a_visible_parent_still_paints() {
    let backend = OffscreenBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("visibility"))
        .unwrap();
    let panel = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(0, 0, 100, 80)),
        )
        .unwrap();
    let child = backend
        .create(
            ParentRef::Widget(panel),
            &NodeSpec::new(NodeKind::Container, Rect::new(10, 10, 30, 30)),
        )
        .unwrap();
    backend.set_painter(child, fill(Color::rgb(255, 0, 0)));

    let image = backend.render(window).expect("a rendered window");
    assert_eq!(
        red_pixels(&image),
        20 * 20,
        "the child of a visible panel is painted, not over-culled"
    );
}

#[test]
fn a_scroll_view_clips_overflowing_content_light_and_dark() {
    struct Scroll {
        _view: ScrollView<u32>,
        _labels: Vec<Label<u32>>,
    }

    impl App for Scroll {
        type Msg = u32;
        fn update(&mut self, _msg: u32, _ui: &mut Ui<u32>) {}
    }

    for (dark, name) in [
        (false, "scrollview-light.png"),
        (true, "scrollview-dark.png"),
    ] {
        let backend = Rc::new(OffscreenBackend::new());
        let backend_for_run: Rc<dyn Backend> = backend.clone();
        let mut rendered = None;

        let result = run_app(
            backend_for_run,
            PlatformSpec::new("scroll view").size(Dip(180.0), Dip(120.0)),
            |ui| {
                ui.set_theme(if dark { Theme::dark() } else { Theme::light() });
                let view = ScrollView::new(ui, Rect::new(12, 12, 128, 84)).unwrap();
                let mut labels = Vec::new();
                for text in ["alpha", "beta", "gamma", "delta", "epsilon"] {
                    let label = Label::new(view.ui(), Rect::new(0, 0, 10, 10), text).unwrap();
                    view.add(label.id(), Dip(28.0));
                    labels.push(label);
                }
                let image = backend.render(ui.window()).expect("a rendered window");
                save(name, &image);
                rendered = Some(image);
                Scroll {
                    _view: view,
                    _labels: labels,
                }
            },
        );

        assert!(result.is_ok(), "the app ran");
        assert!(rendered.is_some(), "the view rendered");
    }
}

#[test]
fn capture_returns_the_rendered_surface() {
    let backend = OffscreenBackend::new();
    let window = backend
        .open_window(&PlatformSpec::new("shot").size(Dip(60.0), Dip(40.0)))
        .unwrap();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 60, 40)),
        )
        .unwrap();
    backend.set_painter(
        node,
        Rc::new(|canvas| {
            let bounds = canvas.bounds();
            canvas.fill_rect(bounds, Color::rgb(255, 0, 0));
        }),
    );

    let image = backend.capture(window).expect("the window was captured");
    assert_eq!(image.size(), (60, 40));
    assert_eq!(image.pixel(30, 20), Some([255, 0, 0, 255]));
}

#[test]
fn capturing_a_closed_window_is_an_error() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("gone")).unwrap();
    backend.close_window(window);
    assert!(backend.capture(window).is_err());
}

#[test]
fn a_resized_move_notifies_the_node_but_a_reposition_does_not() {
    // A node has no HWND of its own to fire a native size message on, so a
    // parent that resizes it through `apply_moves` (as `Control::set_bounds`
    // does) must be told directly: otherwise a widget with layout cached from
    // its last `Event::Resize` (a scrollbar's track, say) never sees the new
    // size.
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("resize")).unwrap();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(0, 0, 40, 20)),
        )
        .unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    backend.set_event_sink(window, Rc::new(Recorder(Rc::clone(&log))));

    backend.apply_moves(window, &[(node, Rect::new(10, 10, 50, 30))]);
    assert!(
        log.borrow().is_empty(),
        "a same-size move is just a reposition, not a resize"
    );

    backend.apply_moves(window, &[(node, Rect::new(10, 10, 90, 30))]);
    assert!(
        matches!(
            log.borrow().last(),
            Some((target, Event::Resize { width: 80, height: 20 })) if *target == node
        ),
        "a size change delivers Event::Resize to the moved node itself"
    );
}
