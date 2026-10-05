//! Mounting builder trees on a headless window.

mod containers;
mod features;
mod grid;
mod views;

use std::rc::Rc;

use super::{Handle, LayoutExt, build, button, column, edit, label, list, row, spacer, status_bar};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, BackendError, Event, PlatformSpec, WindowId};
use crate::geometry::Rect;
use crate::units::Dip;
use crate::widget::{Button, Edit, Fill, ListView, Panel, StatusBar};

struct Idle;

impl App for Idle {
    type Msg = u32;

    fn update(&mut self, _msg: u32, _ui: &mut Ui<u32>) {}
}

/// A 400x300 window whose sink is the runtime, so window events reach layouts.
fn setup() -> (Rc<HeadlessBackend>, WindowId, Ui<u32>, Rc<Runtime<Idle>>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend
        .open_window(&PlatformSpec::new("arrange").size(Dip(400.0), Dip(300.0)))
        .unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    let runtime = Runtime::primary(core, Idle);
    runtime.attach(backend.as_ref());
    (backend, window, ui, runtime)
}

/// Delivers a no-op event, as the backend does for each input: the runtime
/// runs the layout pass the test's changes asked for once it is handled.
fn settle(backend: &HeadlessBackend, window: WindowId) {
    backend.inject(window, crate::backend::WidgetId::NONE, Event::Wake);
}

fn bounds<W: crate::widget::Placeable<u32>>(ui: &Ui<u32>, handle: &Handle<W>) -> Rect {
    ui.bounds(handle.get().id())
}

#[test]
fn a_column_places_its_widgets_with_padding_and_gap() {
    let (_backend, _window, ui, _runtime) = setup();
    let (name, ok) = (Handle::<Edit<u32>>::new(), Handle::<Button<u32>>::new());
    let _mounted = ui
        .mount(
            column()
                .padding(8)
                .gap(4)
                .child(edit().bind(&name))
                .child(button("OK").bind(&ok).fill(1)),
        )
        .unwrap();

    // The edit keeps its 28-dip natural height; the button takes the rest.
    assert_eq!(bounds(&ui, &name), Rect::new(8, 8, 392, 36));
    assert_eq!(bounds(&ui, &ok), Rect::new(8, 40, 392, 292));
}

#[test]
fn a_constructor_error_surfaces_from_mount() {
    let (_backend, _window, ui, _runtime) = setup();
    let failing = build(|_: &Ui<u32>| -> crate::backend::Result<Button<u32>> {
        Err(BackendError::Unsupported("button"))
    });
    let mounted = ui.mount(column().children((label("ok"), failing)));
    assert!(matches!(mounted, Err(BackendError::Unsupported("button"))));
}

#[test]
fn a_spacer_pushes_its_neighbours_apart() {
    let (_backend, _window, ui, _runtime) = setup();
    let (left, right) = (Handle::new(), Handle::new());
    let _mounted = ui
        .mount(row().children((
            button("Left").bind(&left).fixed(60),
            spacer(),
            button("Right").bind(&right).fixed(80),
        )))
        .unwrap();

    assert_eq!(bounds(&ui, &left), Rect::new(0, 0, 60, 300));
    assert_eq!(bounds(&ui, &right), Rect::new(320, 0, 400, 300));
}

#[test]
fn a_nested_layout_takes_its_natural_size_unless_it_fills() {
    let (_backend, _window, ui, _runtime) = setup();
    let (a, b, c) = (Handle::new(), Handle::new(), Handle::new());
    let _mounted = ui
        .mount(
            column().children((
                row().child(button("A").bind(&a).fill(1)),
                row()
                    .children((
                        button("B").bind(&b).fixed(100),
                        button("C").bind(&c).fill(1),
                    ))
                    .fill(1),
            )),
        )
        .unwrap();

    assert_eq!(bounds(&ui, &a), Rect::new(0, 0, 400, 28), "a natural row");
    assert_eq!(bounds(&ui, &b), Rect::new(0, 28, 100, 300));
    assert_eq!(bounds(&ui, &c), Rect::new(100, 28, 400, 300));
}

#[test]
fn a_list_and_a_status_bar_fill_a_column_and_follow_a_resize() {
    let (backend, window, ui, _runtime) = setup();
    let (table, status): (Handle<ListView<u32>>, Handle<StatusBar<u32>>) =
        (Handle::new(), Handle::new());
    let _mounted = ui
        .mount(column().padding(8).gap(4).children((
            list().column("Name", Fill).bind(&table).fill(1),
            status_bar(&["Ready", "0 targets"]).bind(&status).fixed(24),
        )))
        .unwrap();

    assert_eq!(bounds(&ui, &table), Rect::new(8, 8, 392, 264));
    assert_eq!(bounds(&ui, &status), Rect::new(8, 268, 392, 292));

    backend.resize_window(window, 500, 400);
    assert_eq!(bounds(&ui, &table), Rect::new(8, 8, 492, 364));
    assert_eq!(bounds(&ui, &status), Rect::new(8, 368, 492, 392));
}

#[test]
fn hiding_a_widget_reflows_the_layout_once_the_event_is_handled() {
    let (backend, window, ui, _runtime) = setup();
    let (top, rest) = (Handle::<Edit<u32>>::new(), Handle::new());
    let _mounted = ui
        .mount(column().children((edit().bind(&top), button("rest").bind(&rest).fill(1))))
        .unwrap();
    assert_eq!(bounds(&ui, &rest).top, 28);

    ui.set_visible(top.get().id(), false);
    assert_eq!(bounds(&ui, &rest).top, 28, "not before the event is done");
    settle(&backend, window);
    assert_eq!(
        bounds(&ui, &rest),
        Rect::new(0, 0, 400, 300),
        "a hidden widget takes no space"
    );

    ui.set_visible(top.get().id(), true);
    settle(&backend, window);
    assert_eq!(bounds(&ui, &rest).top, 28);
}

#[test]
fn a_window_resize_and_a_dpi_change_reflow_the_layout() {
    let (backend, window, ui, _runtime) = setup();
    let fixed = Handle::new();
    let _mounted = ui
        .mount(row().children((button("fixed").bind(&fixed).fixed(30), spacer())))
        .unwrap();
    assert_eq!(bounds(&ui, &fixed), Rect::new(0, 0, 30, 300));

    backend.resize_window(window, 500, 200);
    assert_eq!(bounds(&ui, &fixed).bottom, 200);
    backend.set_window_dpi(window, 192);
    assert_eq!(bounds(&ui, &fixed).width(), 60);
}

#[test]
fn dropping_the_mounted_layout_destroys_its_widgets_and_stops_the_reflow() {
    let (backend, window, ui, _runtime) = setup();
    let owned = Handle::<Button<u32>>::new();
    let mounted = ui
        .mount(column().child(button("owned").bind(&owned)))
        .unwrap();
    let id = owned.get().id();
    drop(owned);
    assert!(backend.has_node(id));

    let moves = backend.move_calls();
    drop(mounted);
    assert!(!backend.has_node(id), "the layout owned the widget");
    backend.resize_window(window, 500, 200);
    assert_eq!(backend.move_calls(), moves, "no reflow after the drop");
}

#[test]
fn a_root_layout_lives_as_long_as_the_window() {
    let (backend, window, ui, runtime) = setup();
    let kept = Handle::<Button<u32>>::new();
    ui.root(column().child(button("kept").bind(&kept))).unwrap();
    let id = kept.get().id();
    drop(kept);
    backend.resize_window(window, 500, 200);
    assert_eq!(ui.bounds(id).right, 500, "still placed with no handle left");

    drop(runtime);
    assert!(!backend.has_node(id), "released with the window's runtime");
}

#[test]
fn mount_in_creates_the_widgets_inside_a_container_and_follows_its_resize() {
    let (backend, window, ui, _runtime) = setup();
    let panel = Panel::new(&ui, Rect::new(10, 10, 210, 110)).unwrap();
    let inner = Handle::new();
    let _mounted = ui
        .mount_in(
            panel.id(),
            column()
                .padding(5)
                .child(button("inner").bind(&inner).fill(1)),
        )
        .unwrap();
    assert_eq!(
        bounds(&ui, &inner),
        Rect::new(5, 5, 195, 95),
        "placed in the panel's own coordinates"
    );
    assert_eq!(backend.parent_of(inner.get().id()), Some(panel.id()));

    panel.set_bounds(Rect::new(10, 10, 310, 210));
    backend.inject(
        window,
        panel.id(),
        Event::Resize {
            width: 300,
            height: 200,
        },
    );
    assert_eq!(bounds(&ui, &inner), Rect::new(5, 5, 295, 195));
}
