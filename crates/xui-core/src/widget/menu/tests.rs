//! Unit tests for [`Menu`](super::Menu).

use std::cell::RefCell;
use std::rc::Rc;

use super::{Menu, MenuId, layout};
use crate::app::{App, Core, Runtime, Ui};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, Event, PlatformSpec, WidgetId};
use crate::geometry::Rect;
use crate::message::{Key, Modifiers, MouseButton};

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

fn visible(backend: &HeadlessBackend, id: WidgetId) -> bool {
    backend.node(id).expect("node").3
}

/// The row geometry a popup lays out its entries with.
fn geometry(dpi: u32) -> (i32, i32, i32) {
    (
        layout::ROW.to_px(dpi).value(),
        layout::SEPARATOR_ROW.to_px(dpi).value(),
        layout::PAD.to_px(dpi).value(),
    )
}

fn sample_bar(ui: &Ui<u32>) -> Menu<u32> {
    Menu::bar(ui, Rect::new(0, 0, 260, 28))
        .unwrap()
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.submenu(MenuId::new(1), "&File", |f| {
                f.item(MenuId::new(2), "&New");
                f.item(MenuId::new(3), "&Open");
            });
            m.submenu(MenuId::new(4), "&Edit", |e| {
                e.item(MenuId::new(5), "Cu&t");
            });
        })
}

#[test]
fn a_bar_click_opens_its_menu_and_escape_closes_it() {
    let (backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    assert!(!visible(&backend, popup), "the popup starts hidden");
    runtime.deliver(bar, &down(5, 14));
    assert!(menu.is_open(), "clicking the title opens its menu");
    assert!(visible(&backend, popup));

    runtime.deliver(popup, &key(Key::ESCAPE));
    assert!(!menu.is_open(), "Escape closes it");
    assert!(!visible(&backend, popup));
}

#[test]
fn a_context_menu_opens_at_a_point_and_maps_its_selection() {
    let (backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.item(MenuId::new(7), "&Copy");
            m.item(MenuId::new(8), "&Paste");
        });
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    menu.show_context(10, 20);
    assert!(menu.is_open());
    assert!(visible(&backend, popup));

    let (row, _, pad) = geometry(ui.dpi());
    runtime.deliver(popup, &down(5, pad + row + row / 2));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![8], "the second row raised its id");
    assert!(!menu.is_open(), "choosing a command closes the menu");
}

#[test]
fn keyboard_navigation_moves_and_activates() {
    let (_backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.item(MenuId::new(7), "One");
            m.item(MenuId::new(8), "Two");
            m.item(MenuId::new(9), "Three");
        });
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    menu.show_context(0, 0);
    runtime.deliver(popup, &key(Key::DOWN));
    assert_eq!(
        menu.rt.view.borrow().levels[0].hover,
        Some(1),
        "the arrow moves off the initially highlighted row"
    );
    runtime.deliver(popup, &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![8]);
    assert!(!menu.is_open());
}

#[test]
fn a_submenu_opens_sideways_and_selects_a_nested_item() {
    let (backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.submenu(MenuId::new(1), "&More", |s| {
                s.item(MenuId::new(2), "&Deep");
            });
        });
    let outer = menu.popup_id(0).unwrap();
    let inner = menu.popup_id(1).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    menu.show_context(0, 0);
    assert!(!visible(&backend, inner), "the submenu starts hidden");
    runtime.deliver(outer, &key(Key::RIGHT));
    assert!(visible(&backend, inner), "Right opens the submenu");

    runtime.deliver(inner, &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![2]);
    assert!(!menu.is_open());
}

#[test]
fn check_and_radio_items_toggle_and_clear_siblings() {
    let (_backend, core, ui) = setup();
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&toggles);
    let menu = Menu::context(&ui)
        .on_toggle(move |id, checked| {
            seen.borrow_mut().push((id.0, checked));
            None
        })
        .build(|m| {
            m.check(MenuId::new(1), "Auto", false);
            m.separator();
            m.radio(MenuId::new(2), "Left", true);
            m.radio(MenuId::new(3), "Right", false);
        });
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));
    let (row, separator, pad) = geometry(ui.dpi());

    menu.show_context(0, 0);
    runtime.deliver(popup, &down(5, pad + row / 2));
    assert!(menu.is_checked(MenuId::new(1)), "the check toggled on");

    menu.show_context(0, 0);
    let right_top = pad + row * 2 + separator;
    runtime.deliver(popup, &down(5, right_top + row / 2));
    assert!(menu.is_checked(MenuId::new(3)));
    assert!(
        !menu.is_checked(MenuId::new(2)),
        "the sibling radio cleared"
    );
    assert_eq!(*toggles.borrow(), vec![(1, true), (3, true)]);
}

#[test]
fn a_disabled_item_is_skipped_and_ignores_clicks() {
    let (_backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.item(MenuId::new(1), "One");
            m.item(MenuId::new(2), "Two");
        });
    menu.set_enabled(MenuId::new(1), false);
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    menu.show_context(0, 0);
    assert_eq!(
        menu.rt.view.borrow().levels[0].hover,
        Some(1),
        "the first selectable row skips the disabled one"
    );
    let (row, _, pad) = geometry(ui.dpi());
    runtime.deliver(popup, &down(5, pad + row / 2));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert!(log.borrow().is_empty(), "a disabled item raises nothing");
    assert!(menu.is_open(), "and does not close the menu");
}

#[test]
fn a_mnemonic_letter_activates_its_item() {
    let (_backend, core, ui) = setup();
    let log = Rc::new(RefCell::new(Vec::new()));
    let menu = Menu::context(&ui)
        .on_select(|id| Some(id.0 as u32))
        .build(|m| {
            m.item(MenuId::new(7), "&Copy");
            m.item(MenuId::new(8), "&Paste");
        });
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    menu.show_context(0, 0);
    runtime.deliver(popup, &Event::Char('p'));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![8]);
}

#[test]
fn a_popup_paints_its_labels() {
    let (backend, _core, ui) = setup();
    let menu = Menu::context(&ui).build(|m| {
        m.item(MenuId::new(1), "&Copy");
        m.check(MenuId::new(2), "Wrap", true);
    });
    menu.show_context(0, 0);
    let popup = menu.popup_id(0).unwrap();

    backend.render(popup);
    let ops = backend.ops(popup);
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Copy")),
        "the popup painted its label: {ops:?}"
    );
}

#[test]
fn opening_the_bar_menu_paints_its_entries() {
    let (backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(bar, &down(5, 14));
    assert!(visible(&backend, popup), "the bar menu opened");

    backend.render(popup);
    let ops = backend.ops(popup);
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "New")),
        "the bar menu painted its entries: {ops:?}"
    );
}

#[test]
fn the_first_frame_of_an_opened_menu_already_shows_its_entries() {
    let (backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::new(RefCell::new(Vec::new()))));

    runtime.deliver(bar, &down(5, 14));

    // A native show paints synchronously, which the headless backend models,
    // so the recorded ops must already hold the entries with no explicit
    // render: the level is recorded before the popup is shown (#130).
    let ops = backend.ops(popup);
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "New")),
        "the show-time frame already paints the entries: {ops:?}"
    );
}

#[test]
fn popups_are_created_as_transient_popup_surfaces() {
    let (backend, _core, ui) = setup();
    let menu = sample_bar(&ui);
    let bar = menu.id().unwrap();
    let popup = menu.popup_id(0).unwrap();

    assert!(
        backend.is_popup(popup),
        "a dropdown is a transient surface the backend can float above the window"
    );
    assert!(!backend.is_popup(bar), "the bar itself is an ordinary node");
}

#[test]
fn dropping_a_menu_unregisters_its_mappers() {
    let (_backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    assert!(!core.router().is_empty(), "the mappers are registered");

    drop(menu);
    assert!(
        core.router().is_empty(),
        "the bar and popup mappers were unregistered"
    );
}
