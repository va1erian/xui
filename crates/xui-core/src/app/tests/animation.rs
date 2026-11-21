use std::time::Duration;

use super::*;
use crate::motion::{Animated, Easing, Motion};

const fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

/// 100 ms at constant speed, so a test reads the value off the clock.
const LINEAR: Motion = Motion::new(ms(100), Easing::Linear);

/// A window with a clock at zero, a node to animate and a runtime to deliver
/// frames to.
struct Rig {
    backend: Rc<HeadlessBackend>,
    ui: Ui<u32>,
    runtime: Rc<Runtime<TestApp>>,
    node: WidgetId,
    log: Rc<RefCell<Vec<u32>>>,
}

impl Rig {
    fn new() -> Rig {
        let (backend, window, core, ui) = setup();
        backend.set_now(Duration::ZERO);
        let node = backend
            .create(
                ParentRef::Window(window),
                &NodeSpec::new(NodeKind::Custom, Rect::default()),
            )
            .unwrap();
        let log = Rc::new(RefCell::new(Vec::new()));
        let runtime = Runtime::primary(core, test_app(&log));
        Rig {
            backend,
            ui,
            runtime,
            node,
            log,
        }
    }

    /// Moves the clock by `by` and delivers the frame the backend owes.
    fn frame(&self, by: Duration) {
        self.backend.advance(by);
        assert!(self.runtime.deliver(WidgetId::NONE, &Event::Frame));
    }

    fn read(&self, value: &Animated) -> f32 {
        value.value(self.ui.frame_time())
    }
}

#[test]
fn a_value_moves_with_the_clock_and_lands_exactly() {
    let rig = Rig::new();
    let value = Rc::new(Animated::new(0.0));
    rig.ui.animate(rig.node, &value, 10.0, LINEAR);
    assert_eq!(
        rig.read(&value),
        0.0,
        "nothing moved before the first frame"
    );

    rig.frame(ms(30));
    assert_eq!(rig.read(&value), 3.0);
    rig.frame(ms(40));
    assert_eq!(rig.read(&value), 7.0);
    rig.frame(ms(500));
    assert_eq!(rig.read(&value), 10.0);
}

#[test]
fn the_value_at_a_given_time_does_not_depend_on_the_frame_rate() {
    let at_90ms = |step: u64| {
        let rig = Rig::new();
        let value = Rc::new(Animated::new(0.0));
        rig.ui.animate(rig.node, &value, 1.0, Motion::TRAVEL);
        for _ in 0..(90 / step) {
            rig.frame(ms(step));
        }
        rig.read(&value)
    };
    // 30 ms, 15 ms and 5 ms frames: about 33, 66 and 200 fps.
    assert_eq!(at_90ms(30), at_90ms(15));
    assert_eq!(at_90ms(15), at_90ms(5));
}

#[test]
fn frames_are_requested_one_at_a_time_and_stop_when_nothing_moves() {
    let rig = Rig::new();
    let value = Rc::new(Animated::new(0.0));
    assert_eq!(
        rig.backend.frame_requests(),
        0,
        "an idle window asks for none"
    );

    rig.ui.animate(rig.node, &value, 1.0, LINEAR);
    assert_eq!(rig.backend.frame_requests(), 1);
    rig.frame(ms(50));
    assert_eq!(rig.backend.frame_requests(), 2, "still moving: one more");
    rig.frame(ms(50));
    assert_eq!(rig.backend.frame_requests(), 2, "landed: no more");

    // A stray frame with nothing moving changes nothing.
    let painted = rig.backend.invalidations();
    rig.frame(ms(16));
    assert_eq!(rig.backend.frame_requests(), 2);
    assert_eq!(rig.backend.invalidations(), painted);
}

#[test]
fn two_values_share_one_frame() {
    let rig = Rig::new();
    let (a, b) = (Rc::new(Animated::new(0.0)), Rc::new(Animated::new(0.0)));
    rig.ui.animate(rig.node, &a, 1.0, LINEAR);
    rig.ui
        .animate(rig.node, &b, 1.0, Motion::new(ms(200), Easing::Linear));
    assert_eq!(rig.backend.frame_requests(), 1);

    let painted = rig.backend.invalidations();
    rig.frame(ms(100));
    assert_eq!(rig.backend.invalidations(), painted + 2, "both repainted");
    assert_eq!(rig.read(&a), 1.0);
    assert_eq!(rig.read(&b), 0.5);

    let painted = rig.backend.invalidations();
    rig.frame(ms(100));
    assert_eq!(
        rig.backend.invalidations(),
        painted + 1,
        "only the one left"
    );
}

#[test]
fn every_frame_repaints_the_node_including_the_last() {
    let rig = Rig::new();
    let value = Rc::new(Animated::new(0.0));
    let start = rig.backend.invalidations();
    rig.ui.animate(rig.node, &value, 1.0, LINEAR);
    assert_eq!(rig.backend.invalidations(), start + 1);
    rig.frame(ms(60));
    rig.frame(ms(60));
    assert_eq!(rig.backend.invalidations(), start + 3);
}

#[test]
fn reversing_mid_flight_turns_round_where_it_is() {
    let rig = Rig::new();
    let value = Rc::new(Animated::new(0.0));
    rig.ui.animate(rig.node, &value, 10.0, LINEAR);
    rig.frame(ms(40));
    rig.ui.animate(rig.node, &value, 0.0, LINEAR);
    assert_eq!(rig.read(&value), 4.0, "no jump on the reversal");
    rig.frame(ms(50));
    assert_eq!(rig.read(&value), 2.0);
    rig.frame(ms(50));
    assert_eq!(rig.read(&value), 0.0);
    rig.frame(ms(16));
    assert_eq!(rig.backend.frame_requests(), 3, "one entry, not two");
}

#[test]
fn a_step_sees_every_frame_and_the_exact_end() {
    let rig = Rig::new();
    let value = Rc::new(Animated::new(0.0));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&seen);
    rig.ui
        .animate_with(rig.node, &value, 10.0, LINEAR, move |v| {
            sink.borrow_mut().push(v);
        });
    rig.frame(ms(50));
    rig.frame(ms(80));
    assert_eq!(*seen.borrow(), vec![5.0, 10.0]);
}

#[test]
fn a_step_may_start_another_animation() {
    let rig = Rig::new();
    let (first, second) = (Rc::new(Animated::new(0.0)), Rc::new(Animated::new(0.0)));
    let (ui, node, chained) = (rig.ui.clone(), rig.node, Rc::clone(&second));
    rig.ui
        .animate_with(rig.node, &first, 1.0, LINEAR, move |v| {
            if v == 1.0 {
                ui.animate(node, &chained, 8.0, LINEAR);
            }
        });
    rig.frame(ms(100));
    assert_eq!(second.target(), 8.0);
    rig.frame(ms(50));
    assert_eq!(rig.read(&second), 4.0, "the chained value is driven too");
}

fn assert_instant(rig: &Rig) {
    let value = Rc::new(Animated::new(0.0));
    let stepped = Rc::new(Cell::new(f32::NAN));
    let sink = Rc::clone(&stepped);
    let painted = rig.backend.invalidations();
    rig.ui
        .animate_with(rig.node, &value, 10.0, LINEAR, move |v| sink.set(v));
    assert!(!rig.ui.animations());
    assert_eq!(rig.read(&value), 10.0, "landed at once");
    assert_eq!(stepped.get(), 10.0, "the step ran once, with the target");
    assert_eq!(rig.backend.invalidations(), painted + 1, "one repaint");
    assert_eq!(rig.backend.frame_requests(), 0, "no frame asked for");
    assert!(rig.backend.live_timers().is_empty(), "no timer started");
}

#[test]
fn reduced_motion_makes_the_change_instant() {
    let rig = Rig::new();
    assert!(rig.ui.animations());
    rig.backend.set_prefers_reduced_motion(true);
    assert_instant(&rig);
}

#[test]
fn the_application_switch_makes_the_change_instant() {
    let rig = Rig::new();
    rig.ui.set_animations(false);
    assert_instant(&rig);
    rig.ui.set_animations(true);
    assert!(rig.ui.animations());
}

#[test]
fn design_mode_makes_the_change_instant() {
    let rig = Rig::new();
    rig.ui.set_design_mode(true);
    assert_instant(&rig);
}

#[test]
fn a_backend_with_no_clock_makes_the_change_instant() {
    // No `set_now`: the headless backend has no clock by default.
    let (backend, window, core, ui) = setup();
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::default()),
        )
        .unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let rig = Rig {
        runtime: Runtime::primary(core, test_app(&log)),
        backend,
        ui,
        node,
        log,
    };
    assert_instant(&rig);
}

#[test]
fn destroying_the_node_stops_its_animation() {
    let rig = Rig::new();
    let value = Rc::new(Animated::new(0.0));
    rig.ui.animate(rig.node, &value, 1.0, LINEAR);
    rig.ui.destroy(rig.node);

    let painted = rig.backend.invalidations();
    rig.frame(ms(30));
    assert_eq!(
        rig.backend.invalidations(),
        painted,
        "nothing left to paint"
    );
    assert_eq!(rig.backend.frame_requests(), 1, "and no further frame");
}

#[test]
fn without_frame_pacing_a_timer_ticks_and_is_killed_at_the_end() {
    let rig = Rig::new();
    rig.backend.set_frame_pacing(false);
    rig.ui.on_timer(|_| Some(7));
    let value = Rc::new(Animated::new(0.0));
    rig.ui.animate(rig.node, &value, 10.0, LINEAR);
    let timers = rig.backend.live_timers();
    assert_eq!(timers.len(), 1, "the fallback tick");

    let tick = |by| {
        rig.backend.advance(by);
        assert!(
            rig.runtime
                .deliver(WidgetId::NONE, &Event::Timer { id: timers[0] })
        );
    };
    tick(ms(50));
    assert_eq!(rig.read(&value), 5.0);
    assert_eq!(
        rig.backend.live_timers().len(),
        1,
        "one timer, not one per tick"
    );
    tick(ms(50));
    assert_eq!(rig.read(&value), 10.0);
    assert!(
        rig.backend.live_timers().is_empty(),
        "killed when it landed"
    );

    rig.runtime.deliver(WidgetId::NONE, &Event::Wake);
    assert!(
        rig.log.borrow().is_empty(),
        "the app never saw the driver's timer"
    );
}

#[test]
fn a_step_may_restart_its_own_value_without_a_duplicate() {
    let rig = Rig::new();
    let value = Rc::new(Animated::new(0.0));
    let calls = Rc::new(Cell::new(0));
    let (ui, node, again, count) = (
        rig.ui.clone(),
        rig.node,
        Rc::clone(&value),
        Rc::clone(&calls),
    );
    rig.ui
        .animate_with(rig.node, &value, 10.0, LINEAR, move |v| {
            count.set(count.get() + 1);
            if v == 10.0 {
                // Bounce back: the same value, started from inside its own step.
                ui.animate(node, &again, 0.0, LINEAR);
            }
        });
    rig.frame(ms(100));
    assert_eq!(calls.get(), 1);
    assert_eq!(value.target(), 0.0);

    let painted = rig.backend.invalidations();
    rig.frame(ms(50));
    assert_eq!(rig.read(&value), 5.0);
    assert_eq!(rig.backend.invalidations(), painted + 1, "one entry only");
    assert_eq!(calls.get(), 1, "the replaced step is not called again");
}

#[test]
fn a_step_that_lands_a_value_instantly_silences_its_old_step() {
    let rig = Rig::new();
    let (first, second) = (Rc::new(Animated::new(0.0)), Rc::new(Animated::new(0.0)));
    let late_calls = Rc::new(Cell::new(0));
    let count = Rc::clone(&late_calls);
    // `second` is started after `first`, so its entry runs later in a frame.
    let (ui, node, target) = (rig.ui.clone(), rig.node, Rc::clone(&second));
    rig.ui
        .animate_with(rig.node, &first, 1.0, LINEAR, move |_| {
            ui.set_animations(false);
            ui.animate(node, &target, 3.0, LINEAR);
            ui.set_animations(true);
        });
    rig.ui
        .animate_with(rig.node, &second, 9.0, LINEAR, move |_| {
            count.set(count.get() + 1)
        });

    rig.frame(ms(50));
    assert_eq!(rig.read(&second), 3.0, "landed where the step sent it");
    assert_eq!(late_calls.get(), 0, "its old step never ran");
}

#[test]
fn destroying_a_node_from_a_step_stops_what_else_animates_on_it() {
    let rig = Rig::new();
    let (closing, hover) = (Rc::new(Animated::new(1.0)), Rc::new(Animated::new(0.0)));
    // The hover is started first, so it has already run when the step that
    // destroys the node does.
    rig.ui
        .animate(rig.node, &hover, 1.0, Motion::new(ms(500), Easing::Linear));
    let (ui, node) = (rig.ui.clone(), rig.node);
    rig.ui
        .animate_with(rig.node, &closing, 0.0, LINEAR, move |v| {
            if v == 0.0 {
                ui.destroy(node);
            }
        });
    rig.frame(ms(100));
    assert_eq!(
        rig.backend.frame_requests(),
        1,
        "nothing left: no next frame"
    );

    let painted = rig.backend.invalidations();
    rig.frame(ms(16));
    assert_eq!(rig.backend.invalidations(), painted);
}

#[test]
fn a_value_shared_by_two_nodes_animates_on_both() {
    let rig = Rig::new();
    let other = rig
        .backend
        .create(
            ParentRef::Window(rig.ui.window()),
            &NodeSpec::new(NodeKind::Custom, Rect::default()),
        )
        .unwrap();
    let value = Rc::new(Animated::new(0.0));
    rig.ui.animate(rig.node, &value, 1.0, LINEAR);
    rig.ui.animate(other, &value, 1.0, LINEAR);
    let painted = rig.backend.invalidations();
    rig.frame(ms(50));
    assert_eq!(rig.backend.invalidations(), painted + 2);
}

#[test]
fn a_backend_that_can_neither_pace_nor_time_makes_the_change_instant() {
    let rig = Rig::new();
    rig.backend.set_frame_pacing(false);
    rig.backend.set_timers_available(false);
    let value = Rc::new(Animated::new(0.0));
    let painted = rig.backend.invalidations();
    rig.ui.animate(rig.node, &value, 10.0, LINEAR);
    assert_eq!(rig.read(&value), 10.0);
    assert_eq!(rig.backend.invalidations(), painted + 1);
}

#[test]
fn a_clock_lost_mid_flight_lands_everything() {
    let rig = Rig::new();
    let value = Rc::new(Animated::new(0.0));
    let stepped = Rc::new(Cell::new(f32::NAN));
    let sink = Rc::clone(&stepped);
    rig.ui
        .animate_with(rig.node, &value, 10.0, LINEAR, move |v| sink.set(v));
    rig.frame(ms(30));
    rig.backend.clear_now();
    assert!(rig.runtime.deliver(WidgetId::NONE, &Event::Frame));
    assert_eq!(rig.read(&value), 10.0);
    assert_eq!(stepped.get(), 10.0);
    assert_eq!(
        rig.backend.frame_requests(),
        2,
        "and nothing is asked for after"
    );
}

#[test]
fn dropping_the_runtime_releases_a_step_that_holds_the_ui() {
    let rig = Rig::new();
    let value = Rc::new(Animated::new(0.0));
    let held = Rc::new(());
    let (ui, token) = (rig.ui.clone(), Rc::clone(&held));
    rig.ui
        .animate_with(rig.node, &value, 1.0, LINEAR, move |_| {
            let _ = (&ui, &token);
        });
    assert_eq!(Rc::strong_count(&held), 2);
    let Rig { runtime, .. } = rig;
    drop(runtime);
    assert_eq!(Rc::strong_count(&held), 1, "the step went with the window");
}
