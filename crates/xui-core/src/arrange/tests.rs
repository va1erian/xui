use std::rc::Rc;

use super::{LayoutExt, column, row, spacer};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, BackendError, Event, PlatformSpec, WindowId};
use crate::geometry::Rect;
use crate::layout::Insets;
use crate::units::{Dip, dip};
use crate::widget::{Button, Edit, Label, Panel};

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

fn edit(ui: &Ui<u32>) -> Rc<Edit<u32>> {
    Rc::new(Edit::auto(ui, "").unwrap())
}

fn button(ui: &Ui<u32>, text: &str) -> Rc<Button<u32>> {
    Rc::new(Button::auto(ui, text).unwrap())
}

#[test]
fn a_column_places_its_widgets_with_margins_and_spacing() {
    let (_backend, _window, ui, _runtime) = setup();
    let (edit, ok) = (edit(&ui), button(&ui, "OK"));
    let _mounted = ui
        .mount(
            column()
                .margins(Insets::all(dip(8.0)))
                .spacing(dip(4.0))
                .child(&edit)
                .child(ok.fill(1)),
        )
        .unwrap();

    // The edit keeps its 28-dip natural height; the button takes the rest.
    assert_eq!(ui.bounds(edit.id()), Rect::new(8, 8, 392, 36));
    assert_eq!(ui.bounds(ok.id()), Rect::new(8, 40, 392, 292));
}

#[test]
fn a_constructor_result_goes_straight_in_and_its_error_surfaces_from_mount() {
    let (_backend, _window, ui, _runtime) = setup();
    let failing: Result<Button<u32>, BackendError> = Err(BackendError::Unsupported("button"));
    let mounted = ui.mount(column().child(Label::auto(&ui, "ok")).child(failing));
    assert!(matches!(mounted, Err(BackendError::Unsupported("button"))));
}

#[test]
fn a_spacer_pushes_its_neighbours_apart() {
    let (_backend, _window, ui, _runtime) = setup();
    let (left, right) = (button(&ui, "Left"), button(&ui, "Right"));
    let _mounted = ui
        .mount(
            row()
                .child(left.fixed(dip(60.0)))
                .child(spacer())
                .child(right.fixed(dip(80.0))),
        )
        .unwrap();

    assert_eq!(ui.bounds(left.id()), Rect::new(0, 0, 60, 300));
    assert_eq!(ui.bounds(right.id()), Rect::new(320, 0, 400, 300));
}

#[test]
fn nested_layouts_share_the_leftover_space() {
    let (_backend, _window, ui, _runtime) = setup();
    let (a, b, c) = (button(&ui, "A"), button(&ui, "B"), button(&ui, "C"));
    let _mounted = ui
        .mount(
            column()
                .child(a.fixed(dip(40.0)))
                .child(row().child(b.fixed(dip(100.0))).child(c.fill(1))),
        )
        .unwrap();

    assert_eq!(ui.bounds(a.id()), Rect::new(0, 0, 400, 40));
    assert_eq!(ui.bounds(b.id()), Rect::new(0, 40, 100, 300));
    assert_eq!(ui.bounds(c.id()), Rect::new(100, 40, 400, 300));
}

#[test]
fn hiding_a_widget_reflows_the_layout() {
    let (_backend, _window, ui, _runtime) = setup();
    let (top, rest) = (edit(&ui), button(&ui, "rest"));
    let _mounted = ui.mount(column().child(&top).child(rest.fill(1))).unwrap();
    assert_eq!(ui.bounds(rest.id()).top, 28);

    ui.set_visible(top.id(), false);
    assert_eq!(
        ui.bounds(rest.id()),
        Rect::new(0, 0, 400, 300),
        "a hidden widget takes no space"
    );

    ui.set_visible(top.id(), true);
    assert_eq!(ui.bounds(rest.id()).top, 28);
}

#[test]
fn a_window_resize_reflows_the_layout() {
    let (backend, window, ui, _runtime) = setup();
    let fill = button(&ui, "natural");
    let _mounted = ui.mount(column().child(&fill)).unwrap();
    assert_eq!(ui.bounds(fill.id()), Rect::new(0, 0, 400, 28));
    backend.resize_window(window, 500, 200);
    assert_eq!(ui.bounds(fill.id()).right, 500);
}

#[test]
fn a_dpi_change_rescales_design_sizes() {
    let (backend, window, ui, _runtime) = setup();
    let fixed = button(&ui, "fixed");
    let _mounted = ui
        .mount(row().child(fixed.fixed(dip(30.0))).child(spacer()))
        .unwrap();
    assert_eq!(ui.bounds(fixed.id()).width(), 30);

    backend.set_window_dpi(window, 192);
    assert_eq!(ui.bounds(fixed.id()).width(), 60);
}

#[test]
fn dropping_the_mounted_layout_destroys_its_widgets_and_stops_the_reflow() {
    let (backend, window, ui, _runtime) = setup();
    let owned = Button::auto(&ui, "owned").unwrap();
    let id = owned.id();
    let mounted = ui.mount(column().child(owned)).unwrap();
    assert!(backend.has_node(id));

    let moves = backend.move_calls();
    drop(mounted);
    assert!(!backend.has_node(id), "the layout owned the widget");
    backend.resize_window(window, 500, 200);
    assert_eq!(backend.move_calls(), moves, "no reflow after the drop");
}

#[test]
fn mount_in_lays_out_inside_a_container_and_follows_its_resize() {
    let (backend, window, ui, _runtime) = setup();
    let panel = Panel::new(&ui, Rect::new(10, 10, 210, 110)).unwrap();
    let inner = Rc::new(Button::auto(panel.ui(), "inner").unwrap());
    let _mounted = ui
        .mount_in(
            panel.id(),
            column().margins(Insets::all(dip(5.0))).child(inner.fill(1)),
        )
        .unwrap();
    assert_eq!(
        ui.bounds(inner.id()),
        Rect::new(5, 5, 195, 95),
        "placed in the panel's own coordinates"
    );

    panel.set_bounds(Rect::new(10, 10, 310, 210));
    backend.inject(
        window,
        panel.id(),
        Event::Resize {
            width: 300,
            height: 200,
        },
    );
    assert_eq!(ui.bounds(inner.id()), Rect::new(5, 5, 295, 195));
}
