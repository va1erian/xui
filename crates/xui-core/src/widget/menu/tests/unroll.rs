//! The popups unrolling open and rolling shut.

use std::time::Duration;

use super::*;

const SETTLED: Duration = Duration::from_millis(500);

type Rt = Rc<Runtime<TestApp>>;
type Log = Rc<RefCell<Vec<u32>>>;

fn bounds(backend: &HeadlessBackend, id: WidgetId) -> Rect {
    backend.node(id).expect("node").1
}

/// Delivers one frame `by` after the last.
fn frame(backend: &HeadlessBackend, runtime: &Rt, by: Duration) {
    backend.advance(by);
    runtime.deliver(WidgetId::NONE, &Event::Frame);
}

/// The bounds the first popup of `sample_bar` has when fully open, from a run
/// with no clock, which opens it at once.
fn full_bounds() -> Rect {
    let (backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let runtime = Runtime::primary(core, TestApp(Rc::default()));
    runtime.deliver(menu.id().unwrap(), &down(5, 14));
    bounds(&backend, menu.popup_id(0).unwrap())
}

/// A bar menu on a headless window whose clock reads zero, with its first
/// popup opened and the runtime.
fn animated() -> (Rc<HeadlessBackend>, Menu<u32>, Rt, Log) {
    let (backend, core, ui) = setup();
    backend.set_now(Duration::ZERO);
    let menu = sample_bar(&ui);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));
    runtime.deliver(menu.id().unwrap(), &down(5, 14));
    (backend, menu, runtime, log)
}

#[test]
fn a_menu_opening_from_closed_unrolls_from_its_top_edge() {
    let full = full_bounds();
    let (backend, menu, runtime, _log) = animated();
    let popup = menu.popup_id(0).unwrap();

    let start = bounds(&backend, popup);
    assert!(visible(&backend, popup));
    assert_eq!(
        (start.left, start.top, start.right),
        (full.left, full.top, full.right)
    );
    assert!(start.height() < full.height(), "it starts small: {start:?}");

    frame(&backend, &runtime, Duration::from_millis(20));
    let part = bounds(&backend, popup);
    assert_eq!(part.top, full.top, "still hanging from the top edge");
    assert!(part.height() > start.height() && part.height() < full.height());

    frame(&backend, &runtime, SETTLED);
    assert_eq!(bounds(&backend, popup), full);
    let requests = backend.frame_requests();
    frame(&backend, &runtime, SETTLED);
    assert_eq!(
        backend.frame_requests(),
        requests,
        "no frame after it landed"
    );
}

#[test]
fn a_pick_emits_at_once_and_the_popup_rolls_shut_then_hides() {
    let full = full_bounds();
    let (backend, menu, runtime, log) = animated();
    let popup = menu.popup_id(0).unwrap();
    frame(&backend, &runtime, SETTLED);

    let (row, _, pad) = geometry(menu.rt.ui.dpi());
    runtime.deliver(popup, &down(5, pad + row / 2));
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(
        *log.borrow(),
        vec![2],
        "the message is not held for the animation"
    );
    assert!(!menu.is_open());
    assert!(
        visible(&backend, popup),
        "it is still on screen, rolling shut"
    );

    frame(&backend, &runtime, Duration::from_millis(20));
    let part = bounds(&backend, popup);
    assert!(visible(&backend, popup));
    assert!(part.height() < full.height(), "shrinking: {part:?}");
    backend.render(popup);
    let texts = backend
        .ops(popup)
        .iter()
        .filter(|op| matches!(op, DrawOp::Text(..)))
        .count();
    assert_eq!(texts, 2, "its rows are still painted while it shrinks");

    frame(&backend, &runtime, SETTLED);
    assert!(!visible(&backend, popup), "hidden once it has landed");
    let requests = backend.frame_requests();
    frame(&backend, &runtime, SETTLED);
    assert_eq!(backend.frame_requests(), requests);
}

#[test]
fn reopening_a_popup_that_is_rolling_shut_cancels_the_close() {
    let full = full_bounds();
    let (backend, menu, runtime, _log) = animated();
    let popup = menu.popup_id(0).unwrap();
    frame(&backend, &runtime, SETTLED);

    runtime.deliver(popup, &key(Key::ESCAPE));
    frame(&backend, &runtime, Duration::from_millis(20));
    assert!(visible(&backend, popup));

    runtime.deliver(menu.id().unwrap(), &down(5, 14));
    assert!(menu.is_open());
    frame(&backend, &runtime, SETTLED);
    assert!(visible(&backend, popup), "the close did not hide it");
    assert_eq!(bounds(&backend, popup), full);
}

#[test]
fn moving_along_the_bar_swaps_the_popup_without_a_frame() {
    let (backend, menu, runtime, _log) = animated();
    let popup = menu.popup_id(0).unwrap();
    frame(&backend, &runtime, SETTLED);
    let requests = backend.frame_requests();

    runtime.deliver(menu.id().unwrap(), &key(Key::RIGHT));
    assert!(menu.is_open() && visible(&backend, popup));
    let swapped = bounds(&backend, popup);
    assert_eq!(
        backend.frame_requests(),
        requests,
        "no animation was started"
    );
    frame(&backend, &runtime, SETTLED);
    assert_eq!(bounds(&backend, popup), swapped, "it was full size at once");
}

#[test]
fn a_submenu_unrolls_and_hides_at_once_with_its_parent() {
    let (backend, core, ui) = setup();
    backend.set_now(Duration::ZERO);
    let menu = Menu::context(&ui).build(|m| {
        m.submenu(MenuId::new(1), "&More", |s| {
            s.item(MenuId::new(2), "&Deep");
            s.item(MenuId::new(3), "D&eeper");
        });
    });
    let (outer, inner) = (menu.popup_id(0).unwrap(), menu.popup_id(1).unwrap());
    let runtime = Runtime::primary(core, TestApp(Rc::default()));
    menu.show_context(0, 0);
    frame(&backend, &runtime, SETTLED);

    runtime.deliver(outer, &key(Key::RIGHT));
    let start = bounds(&backend, inner);
    frame(&backend, &runtime, SETTLED);
    let full = bounds(&backend, inner);
    assert!(
        start.height() < full.height(),
        "{start:?} grows to {full:?}"
    );
    assert_eq!(start.top, full.top);

    menu.close();
    assert!(!visible(&backend, inner), "a nested popup hides at once");
    assert!(visible(&backend, outer), "the outermost rolls shut");
    frame(&backend, &runtime, SETTLED);
    assert!(!visible(&backend, outer));
}

#[test]
fn without_a_clock_the_popups_open_and_close_at_once() {
    let full = full_bounds();
    let (backend, core, ui) = setup();
    let menu = sample_bar(&ui);
    let popup = menu.popup_id(0).unwrap();
    let runtime = Runtime::primary(core, TestApp(Rc::default()));

    runtime.deliver(menu.id().unwrap(), &down(5, 14));
    assert_eq!(bounds(&backend, popup), full);
    assert!(visible(&backend, popup));
    runtime.deliver(popup, &key(Key::ESCAPE));
    assert!(!visible(&backend, popup));
    assert_eq!(backend.frame_requests(), 0);
}
