//! Unit tests for the offscreen backend: per-node clipping and pointer capture.

use std::cell::RefCell;
use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, Event, NodeKind, NodeSpec, ParentRef, PlatformSpec, WidgetId};
use xui_core::message::{Modifiers, MouseButton};
use xui_core::widget::{Label, ScrollView};
use xui_core::{Color, Dip, Rect, Theme};

use super::OffscreenBackend;
use crate::tests::save;

/// A sink that records every event it is handed.
struct Recorder(Rc<RefCell<Vec<(WidgetId, Event)>>>);

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
fn a_captured_node_receives_an_out_of_bounds_move() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("capture")).unwrap();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(10, 10, 60, 50)),
        )
        .unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    backend.set_event_sink(window, Rc::new(Recorder(Rc::clone(&log))));

    backend.set_capture(node);
    let consumed = backend.inject(
        window,
        Event::MouseMove {
            x: 500,
            y: 500,
            modifiers: Modifiers::NONE,
        },
    );

    assert!(consumed, "the captured node consumes the move");
    let events = log.borrow();
    let (target, event) = events.last().expect("an event was delivered");
    assert_eq!(*target, node, "the captured node is the target");
    // Far outside the node: the local point stays negative.
    assert!(matches!(
        event,
        Event::MouseMove { x, y, .. } if *x == 490 && *y == 490
    ));
}

#[test]
fn releasing_capture_tells_the_captured_node() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("capture")).unwrap();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(10, 10, 60, 50)),
        )
        .unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    backend.set_event_sink(window, Rc::new(Recorder(Rc::clone(&log))));

    backend.set_capture(node);
    backend.release_capture();

    let events = log.borrow();
    assert!(matches!(
        events.as_slice(),
        [(target, Event::CaptureChanged)] if *target == node
    ));
    // A move after release no longer reaches the node.
    let before = events.len();
    backend.inject(
        window,
        Event::MouseMove {
            x: 500,
            y: 500,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(log.borrow().len(), before, "capture is gone");
}

#[test]
fn a_mouse_up_reaches_the_captured_node_outside_it() {
    let backend = OffscreenBackend::new();
    let window = backend.open_window(&PlatformSpec::new("capture")).unwrap();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Container, Rect::new(0, 0, 40, 40)),
        )
        .unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    backend.set_event_sink(window, Rc::new(Recorder(Rc::clone(&log))));

    backend.set_capture(node);
    backend.inject(
        window,
        Event::MouseUp {
            x: 200,
            y: 200,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );

    assert!(matches!(
        log.borrow().last(),
        Some((target, Event::MouseUp { .. })) if *target == node
    ));
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
