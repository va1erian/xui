use std::time::Duration;

use super::*;

fn bar(ui: &Ui<u32>) -> ProgressBar<u32> {
    ProgressBar::new(ui, Rect::new(0, 0, 100, 12), 100).unwrap()
}

fn ops(backend: &HeadlessBackend, bar: &ProgressBar<u32>) -> Vec<DrawOp> {
    backend.render(bar.id());
    backend.ops(bar.id())
}

/// What a bar at `value` paints with no animation at all.
fn settled(value: i32) -> Vec<DrawOp> {
    let (backend, _core, ui) = setup();
    let bar = bar(&ui);
    bar.set_value(value);
    assert_eq!(backend.frame_requests(), 0);
    ops(&backend, &bar)
}

#[test]
fn the_fill_eases_to_a_new_value_while_the_value_is_reported_at_once() {
    let (backend, core, ui) = setup();
    backend.set_now(Duration::ZERO);
    let bar = bar(&ui);
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );
    let empty = ops(&backend, &bar);
    let full = settled(80);

    bar.set_value(80);
    assert_eq!(bar.value(), 80, "the value is the new one immediately");
    backend.advance(Duration::from_millis(40));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
    let midway = ops(&backend, &bar);
    assert_ne!(midway, empty);
    assert_ne!(midway, full);

    backend.advance(Duration::from_millis(300));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
    assert_eq!(ops(&backend, &bar), full);
    let requests = backend.frame_requests();
    backend.advance(Duration::from_millis(300));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
    assert_eq!(backend.frame_requests(), requests, "no frame once landed");

    // Going back eases down the same way.
    bar.set_value(20);
    backend.advance(Duration::from_millis(40));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
    let falling = ops(&backend, &bar);
    assert_ne!(falling, full);
    assert_ne!(falling, settled(20));
}

#[test]
fn a_value_that_does_not_move_the_fill_asks_for_no_frame() {
    let (backend, _core, ui) = setup();
    backend.set_now(Duration::ZERO);
    let bar = bar(&ui);
    bar.set_value(0);
    bar.set_max(50);
    assert_eq!(backend.frame_requests(), 0);
}

#[test]
fn without_a_clock_the_fill_jumps() {
    let (backend, _core, ui) = setup();
    let bar = bar(&ui);
    bar.set_value(80);
    assert_eq!(ops(&backend, &bar), settled(80));
    assert_eq!(backend.frame_requests(), 0);
}
