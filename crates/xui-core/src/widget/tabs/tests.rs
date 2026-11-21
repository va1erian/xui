use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::Tabs;
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Modifiers, MouseButton};
use crate::property::{Properties, Value};
use crate::widget::Label;

mod scroll;

struct TestApp(Rc<RefCell<Vec<usize>>>);

impl App for TestApp {
    type Msg = usize;
    fn update(&mut self, msg: usize, _ui: &mut Ui<usize>) {
        self.0.borrow_mut().push(msg);
    }
}

fn setup() -> (Rc<HeadlessBackend>, Rc<Core<usize>>, Ui<usize>) {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("tabs")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    (backend, core, ui)
}

fn click(x: i32) -> Event {
    Event::MouseUp {
        x,
        y: 8,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn the_selected_page_shows_and_the_others_hide() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let a = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "one").unwrap();
    let b = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "two").unwrap();
    let tabs = tabs.page("One", &[a.id()]).page("Two", &[b.id()]);

    assert!(backend.node(a.id()).unwrap().3, "the first page is shown");
    assert_eq!(
        backend.node(a.id()).unwrap().1,
        Rect::new(0, 32, 200, 120),
        "its child fills the page area below the strip"
    );
    assert!(
        !backend.node(b.id()).unwrap().3,
        "the second page is hidden"
    );
    assert_eq!(tabs.selected(), 0);
}

#[test]
fn selecting_a_page_swaps_the_visible_children() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let a = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "one").unwrap();
    let b = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "two").unwrap();
    let tabs = tabs.page("One", &[a.id()]).page("Two", &[b.id()]);
    let core = _core;

    tabs.select(1);
    assert_eq!(tabs.selected(), 1);
    assert!(!backend.node(a.id()).unwrap().3);
    assert!(backend.node(b.id()).unwrap().3);
    assert_eq!(backend.node(b.id()).unwrap().1, Rect::new(0, 32, 200, 120));
    let _ = core;
}

#[test]
fn clicking_a_tab_selects_it_and_raises_the_message() {
    let (_backend, core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let a = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "one").unwrap();
    let b = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "two").unwrap();
    let tabs = tabs
        .page("One", &[a.id()])
        .page("Two", &[b.id()])
        .on_change(Some);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    let second = tabs.shared.tabs.borrow()[1];
    runtime.deliver(tabs.shared.strip_id, &click(second.left + 2));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(tabs.selected(), 1);
    assert_eq!(*log.borrow(), vec![1]);
}

#[test]
fn a_built_tab_control_maps_a_click_through_on_change_with() {
    use crate::arrange::{Handle, LayoutExt, column, tabs};
    let (_backend, core, ui) = setup();
    let container = Handle::<Tabs<usize>>::new();
    let _mounted = ui
        .mount(
            column().child(
                tabs()
                    .page("One", column())
                    .page("Two", column())
                    .on_change_with(|index| (index > 0).then_some(index * 10))
                    .bind(&container)
                    .fill(1),
            ),
        )
        .unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    let tabs = container.get();
    let second = tabs.shared.tabs.borrow()[1];
    runtime.deliver(tabs.shared.strip_id, &click(second.left + 2));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![10]);
}

#[test]
fn hovering_a_tab_only_repaints_the_strip() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let a = Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "one").unwrap();
    let _ = a;
    let tabs = tabs.page("One", &[a.id()]).page("Two", &[]);

    let before = backend.invalidations();
    let strip = tabs.shared.strip_id;
    let runtime = Runtime::primary(_core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    runtime.deliver(
        strip,
        &Event::MouseMove {
            x: 4,
            y: 8,
            modifiers: Modifiers::NONE,
        },
    );
    assert!(backend.invalidations() > before, "hover repaints the strip");
}

#[test]
fn the_selected_property_round_trips() {
    let (_backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let tabs = tabs.page("One", &[]).page("Two", &[]);
    assert_eq!(tabs.property("selected"), Some(Value::Integer(0)));
    assert!(tabs.set_property("selected", Value::Integer(1)));
    assert_eq!(tabs.selected(), 1);
}

#[test]
fn a_tabs_own_design_mode_freezes_its_strip() {
    let (_backend, core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let tabs = tabs.page("One", &[]).page("Two", &[]);
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    let second = tabs.shared.tabs.borrow()[1];

    tabs.set_design_mode(true);
    runtime.deliver(tabs.shared.strip_id, &click(second.left + 2));
    assert_eq!(
        tabs.selected(),
        0,
        "the strip ignores clicks in design mode"
    );

    tabs.set_design_mode(false);
    runtime.deliver(tabs.shared.strip_id, &click(second.left + 2));
    assert_eq!(tabs.selected(), 1);
}

fn label(tabs: &Tabs<usize>, text: &str) -> Label<usize> {
    Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), text).unwrap()
}

#[test]
fn removing_the_selected_page_selects_its_successor() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let (a, b, c) = (label(&tabs, "a"), label(&tabs, "b"), label(&tabs, "c"));
    let tabs = tabs
        .page("A", &[a.id()])
        .page("B", &[b.id()])
        .page("C", &[c.id()]);
    tabs.select(1);
    assert_eq!(tabs.remove_page(1), Some(vec![b.id()]));
    assert_eq!(tabs.page_count(), 2);
    assert_eq!(tabs.selected(), 1, "page C took the removed page's place");
    assert!(backend.node(c.id()).unwrap().3);
    assert!(!backend.node(a.id()).unwrap().3);
    assert!(
        !backend.node(b.id()).unwrap().3,
        "the removed page's child is hidden"
    );

    tabs.remove_page(1);
    assert_eq!(tabs.selected(), 0, "removing the last page clamps");
    assert!(backend.node(a.id()).unwrap().3);
    assert_eq!(tabs.remove_page(1), None);
}

#[test]
fn removing_an_earlier_page_keeps_the_selection_on_its_page() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let (a, b, c) = (label(&tabs, "a"), label(&tabs, "b"), label(&tabs, "c"));
    let tabs = tabs
        .page("A", &[a.id()])
        .page("B", &[b.id()])
        .page("C", &[c.id()]);
    tabs.select(2);
    tabs.remove_page(0);
    assert_eq!(tabs.selected(), 1);
    assert!(backend.node(c.id()).unwrap().3);
}

#[test]
fn renaming_and_adding_pages_update_the_strip() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 200, 120)).unwrap();
    let tabs = tabs.page("A", &[]).page("B", &[]);
    assert!(tabs.rename_page(1, "Renamed"));
    assert!(!tabs.rename_page(9, "x"));
    assert_eq!(tabs.shared.titles.borrow()[1], "Renamed");
    let before = tabs.shared.tabs.borrow()[1];
    assert!(tabs.rename_page(1, "A much longer title"));
    assert!(tabs.shared.tabs.borrow()[1].width() > before.width());

    let extra = label(&tabs, "d");
    tabs.add_page("D", &[extra.id()]);
    assert_eq!(tabs.page_count(), 3);
    assert!(
        !backend.node(extra.id()).unwrap().3,
        "an added page starts hidden"
    );
}

#[test]
fn a_page_added_while_the_container_has_no_bounds_starts_hidden() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::default()).unwrap();
    let (a, b) = (label(&tabs, "a"), label(&tabs, "b"));
    let tabs = tabs.page("A", &[a.id()]).page("B", &[b.id()]);
    assert!(
        !backend.node(b.id()).unwrap().3,
        "the unselected page's child is hidden even without bounds"
    );
    tabs.add_page("C", &[]);
    assert_eq!(tabs.page_count(), 3);

    tabs.select(1);
    assert!(backend.node(b.id()).unwrap().3, "selecting shows the page");
    assert!(!backend.node(a.id()).unwrap().3);
    assert!(tabs.remove_page(0).is_some());
    assert!(
        backend.node(b.id()).unwrap().3,
        "the successor of a removed page is shown without bounds"
    );
}

/// The accent underline in the last render of the strip.
fn underline(backend: &HeadlessBackend, tabs: &Tabs<usize>) -> Rect {
    use crate::backend::headless::DrawOp;
    let accent = tabs.scoped.theme().accent;
    backend.render(tabs.shared.strip_id);
    backend
        .ops(tabs.shared.strip_id)
        .iter()
        .find_map(|op| match op {
            DrawOp::Fill(rect, color) if *color == accent => Some(*rect),
            _ => None,
        })
        .expect("an underline was painted")
}

fn frame_after(runtime: &Rc<Runtime<TestApp>>, backend: &HeadlessBackend, millis: u64) {
    backend.advance(Duration::from_millis(millis));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
}

/// Tabs "A", "B", "C" in a running app, with a clock.
fn animated() -> (Rc<HeadlessBackend>, Rc<Runtime<TestApp>>, Tabs<usize>) {
    let (backend, core, ui) = setup();
    backend.set_now(Duration::ZERO);
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 300, 120)).unwrap();
    let (a, b, c) = (label(&tabs, "a"), label(&tabs, "b"), label(&tabs, "c"));
    let tabs = tabs
        .page("A", &[a.id()])
        .page("B", &[b.id()])
        .page("C", &[c.id()])
        .on_change(Some);
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    (backend, runtime, tabs)
}

#[test]
fn a_click_slides_the_underline_to_the_new_tab() {
    let (backend, runtime, tabs) = animated();
    let (first, second) = (tabs.shared.tabs.borrow()[0], tabs.shared.tabs.borrow()[1]);
    let bottom = underline(&backend, &tabs).bottom;
    assert_eq!(underline(&backend, &tabs).left, first.left);

    runtime.deliver(tabs.shared.strip_id, &click(second.left + 2));
    assert_eq!(tabs.selected(), 1, "the selection changes at once");
    let start = underline(&backend, &tabs);
    assert_eq!((start.left, start.right), (first.left, first.right));

    frame_after(&runtime, &backend, 90);
    let mid = underline(&backend, &tabs);
    assert!(mid.left > first.left && mid.left < second.left, "{mid:?}");
    assert!(
        mid.right > first.right && mid.right < second.right,
        "{mid:?}"
    );
    assert_eq!(mid.bottom, bottom);

    frame_after(&runtime, &backend, 200);
    let end = underline(&backend, &tabs);
    assert_eq!(
        (end.left, end.right),
        (second.left, second.right),
        "the last frame is the picture without animation"
    );
    assert_eq!(backend.frame_requests(), 2, "no frame is asked for after");
}

#[test]
fn selecting_during_a_flight_starts_from_where_the_underline_is() {
    let (backend, runtime, tabs) = animated();
    tabs.select(2);
    frame_after(&runtime, &backend, 90);
    let before = underline(&backend, &tabs);

    tabs.select(0);
    assert_eq!(underline(&backend, &tabs), before, "no jump");
    frame_after(&runtime, &backend, 400);
    let first = tabs.shared.tabs.borrow()[0];
    let end = underline(&backend, &tabs);
    assert_eq!((end.left, end.right), (first.left, first.right));
}

#[test]
fn without_a_clock_the_underline_is_in_place_at_once() {
    let (backend, _core, ui) = setup();
    let tabs = Tabs::new(&ui, Rect::new(0, 0, 300, 120)).unwrap();
    let tabs = tabs.page("A", &[]).page("B", &[]);
    tabs.select(1);
    let second = tabs.shared.tabs.borrow()[1];
    let line = underline(&backend, &tabs);
    assert_eq!((line.left, line.right), (second.left, second.right));
    assert_eq!(backend.frame_requests(), 0);
}

#[test]
fn removing_or_renaming_a_page_mid_flight_ends_the_flight_in_place() {
    let (backend, runtime, tabs) = animated();
    tabs.select(2);
    frame_after(&runtime, &backend, 50);
    tabs.remove_page(2);
    assert_eq!(tabs.selected(), 1);
    let second = tabs.shared.tabs.borrow()[1];
    let line = underline(&backend, &tabs);
    assert_eq!((line.left, line.right), (second.left, second.right));

    tabs.select(0);
    frame_after(&runtime, &backend, 50);
    assert!(tabs.rename_page(0, "A much longer title"));
    let first = tabs.shared.tabs.borrow()[0];
    let line = underline(&backend, &tabs);
    assert_eq!((line.left, line.right), (first.left, first.right));
    frame_after(&runtime, &backend, 400);
    assert_eq!(underline(&backend, &tabs), line);
}

#[test]
fn a_layout_pass_after_a_selection_does_not_cut_the_travel_short() {
    let (backend, runtime, tabs) = animated();
    let (first, second) = (tabs.shared.tabs.borrow()[0], tabs.shared.tabs.borrow()[1]);
    tabs.select(1);
    // What a mounted layout does once the event is handled.
    tabs.relayout();
    frame_after(&runtime, &backend, 90);
    tabs.relayout();
    let mid = underline(&backend, &tabs);
    assert!(mid.left > first.left && mid.left < second.left, "{mid:?}");
}
