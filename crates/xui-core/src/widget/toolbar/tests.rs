#![forbid(unsafe_code)]

//! [`Toolbar`](super::Toolbar) behaviour tests.

use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::app::{App, Core, Runtime};
use crate::backend::headless::HeadlessBackend;
use crate::backend::{Backend, PlatformSpec};
use crate::message::Modifiers;

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

fn down(x: i32) -> Event {
    Event::MouseDown {
        x,
        y: 5,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn up(x: i32) -> Event {
    Event::MouseUp {
        x,
        y: 5,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn clicking_an_item_maps_to_the_apps_message() {
    let (_backend, core, ui) = setup();
    let toolbar = Toolbar::new(&ui, Rect::new(0, 0, 90, 28), &["one", "two", "three"])
        .unwrap()
        .fill()
        .on_click(|index| Some(index as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(toolbar.id(), &down(45));
    runtime.deliver(toolbar.id(), &up(45));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![1]);
    assert_eq!(toolbar.property("selected"), Some(Value::Integer(1)));
}

#[test]
fn a_click_past_the_last_item_raises_nothing() {
    let (_backend, core, ui) = setup();
    let toolbar = Toolbar::new(&ui, Rect::new(0, 0, 90, 28), &["one", "two", "three"])
        .unwrap()
        .fill()
        .on_click(|index| Some(index as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(toolbar.id(), &down(95));
    runtime.deliver(toolbar.id(), &up(95));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert!(log.borrow().is_empty(), "no item is under x=95");
}

#[test]
fn an_icon_toolbar_builds_lays_out_and_reports_its_items() {
    use crate::icon::Lucide;

    let (backend, _core, ui) = setup();
    let toolbar = Toolbar::empty(&ui, Rect::new(0, 0, 120, 28))
        .unwrap()
        .item(Lucide::Save, "Save")
        .item_with_text(Lucide::Play, "Run", "Run");

    assert_eq!(toolbar.len(), 2);
    assert!(!toolbar.is_empty());
    assert_eq!(toolbar.icon(0), Some(IconRef::Lucide(Lucide::Save)));
    assert_eq!(toolbar.label(0), None);
    assert_eq!(toolbar.tooltip(0).as_deref(), Some("Save"));
    assert_eq!(toolbar.icon(1), Some(IconRef::Lucide(Lucide::Play)));
    assert_eq!(toolbar.label(1).as_deref(), Some("Run"));
    assert_eq!(
        toolbar.tooltip(3),
        None,
        "an index past the end has no item"
    );

    backend.render(toolbar.id());
    assert!(
        !backend.ops(toolbar.id()).is_empty(),
        "the icon toolbar paints its items"
    );
}

#[test]
fn cells_stay_inside_the_strip_and_every_pixel_hits_its_cell() {
    // Wider than the item count with a remainder, and narrower than it.
    for (width, count) in [(10, 3), (3, 5), (100, 7), (1, 1)] {
        let entries = items(count);
        let strip = layout::compute(&entries, Mode::Fill, (width, 24), 96, &mut |_| 0);
        let mut covered = 0;
        for index in 0..count {
            let (start, end) = layout::cell_span(width, count, index);
            assert_eq!(strip.items[index], (start, end));
            assert!(
                0 <= start && start <= end && end <= width,
                "{width}/{count}: {index}"
            );
            covered += end - start;
            for x in start..end {
                assert_eq!(strip.item_at(x), Some(index), "{width}/{count} at {x}");
            }
        }
        assert_eq!(covered, width, "the cells tile the strip exactly");
        assert_eq!(strip.item_at(width), None, "past the right edge");
        assert_eq!(strip.item_at(-1), None, "before the left edge");
    }
}

#[test]
fn an_empty_or_zero_width_strip_has_no_cells() {
    assert_eq!(layout::cell_span(0, 3, 1), (0, 0));
    assert_eq!(layout::cell_span(10, 0, 0), (0, 0));
    let three = items(3);
    let none = layout::compute(&three, Mode::Fill, (0, 24), 96, &mut |_| 0);
    assert_eq!(none.item_at(0), None);
    let empty = layout::compute(&items(0), Mode::Fill, (10, 24), 96, &mut |_| 0);
    assert_eq!(empty.item_at(5), None);
}

/// `count` icon-only items.
fn items(count: usize) -> strip::Entries {
    let mut entries = strip::Entries::default();
    for _ in 0..count {
        entries.push(Entry::Item(Item {
            icon: Some(IconRef::Lucide(crate::icon::Lucide::Save)),
            label: None,
            tooltip: None,
        }));
    }
    entries
}

#[test]
fn a_separator_never_shifts_the_index_of_a_click() {
    use crate::icon::Lucide;

    let (_backend, core, ui) = setup();
    let toolbar = Toolbar::empty(&ui, Rect::new(0, 0, 400, 40))
        .unwrap()
        .separator()
        .item(Lucide::Save, "a")
        .item(Lucide::Copy, "b")
        .separator()
        .separator()
        .item_with_text(Lucide::Play, "c", "Run")
        .separator()
        .on_click(|index| Some(index as u32));
    assert_eq!(toolbar.len(), 3, "separators are not items");
    assert_eq!(toolbar.tooltip(2).as_deref(), Some("c"));
    assert_eq!(toolbar.tooltip(3), None);
    assert_eq!(toolbar.label(2).as_deref(), Some("Run"));

    let strip = layout_of(&ui, toolbar.id(), &toolbar.state);
    assert_eq!(strip.separators.len(), 4);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));
    let click = |x: i32| {
        runtime.deliver(toolbar.id(), &down(x));
        runtime.deliver(toolbar.id(), &up(x));
        runtime.deliver(WidgetId::NONE, &Event::Wake);
    };
    for (index, &(start, end)) in strip.items.iter().enumerate() {
        click(start);
        click(end - 1);
        assert_eq!(log.borrow().last(), Some(&(index as u32)));
    }
    let clicks = log.borrow().len();
    for &x in &strip.separators {
        click(x);
    }
    click(399);
    click(-1);
    assert_eq!(
        log.borrow().len(),
        clicks,
        "separators and the tail are dead"
    );
}

#[test]
fn a_clipped_trailing_item_cannot_be_clicked() {
    use crate::icon::Lucide;

    let (_backend, core, ui) = setup();
    // Two 40px squares fit in 100px; the third would end at 120.
    let toolbar = Toolbar::empty(&ui, Rect::new(0, 0, 100, 40))
        .unwrap()
        .item(Lucide::Save, "a")
        .item(Lucide::Copy, "b")
        .item(Lucide::Play, "c")
        .on_click(|index| Some(index as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));
    for x in [85, 99] {
        runtime.deliver(toolbar.id(), &down(x));
        runtime.deliver(toolbar.id(), &up(x));
    }
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(log.borrow().is_empty());
}

/// A key press.
fn key(key: crate::Key) -> Event {
    Event::KeyDown {
        key,
        modifiers: crate::Modifiers::NONE,
        repeat: 1,
        system: false,
    }
}

#[test]
fn the_keyboard_cannot_reach_or_activate_a_clipped_item() {
    use crate::Key;
    use crate::icon::Lucide;

    let (_backend, core, ui) = setup();
    // Two 40px squares fit in 100px; the third is clipped.
    let toolbar = Toolbar::empty(&ui, Rect::new(0, 0, 100, 40))
        .unwrap()
        .item(Lucide::Save, "a")
        .item(Lucide::Copy, "b")
        .item(Lucide::Play, "c")
        .on_click(|index| Some(index as u32));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));
    for _ in 0..5 {
        runtime.deliver(toolbar.id(), &key(Key::RIGHT));
    }
    runtime.deliver(toolbar.id(), &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(
        *log.borrow(),
        vec![1],
        "Right stops at the last visible item"
    );
}
