use super::*;
use crate::app::{App, Core, Runtime};
use crate::backend::headless::{DrawOp, HeadlessBackend};
use crate::backend::{Backend, PlatformSpec};
use crate::message::{Modifiers, MouseButton};
use crate::widget::{Button, Label};

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

fn runtime(core: Rc<Core<u32>>) -> (Rc<Runtime<TestApp>>, Rc<RefCell<Vec<u32>>>) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let rt = Runtime::primary(core, TestApp(Rc::clone(&log)));
    (rt, log)
}

fn on_move(rt: &Runtime<TestApp>, id: WidgetId) {
    rt.deliver(
        id,
        &Event::MouseMove {
            x: 5,
            y: 5,
            modifiers: Modifiers::NONE,
        },
    );
}

fn visible(backend: &HeadlessBackend, id: WidgetId) -> bool {
    backend.node(id).expect("node").3
}

#[test]
fn shows_after_the_delay_and_hides_on_leave() {
    let (backend, core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "target").unwrap();
    let tooltip = Tooltip::attach(&ui, label.id(), "tip").unwrap();
    let (rt, _log) = runtime(core);

    assert!(!visible(&backend, tooltip.id()), "hidden before hover");
    on_move(&rt, label.id());
    let timer = tooltip.shared.pending.get().expect("the delay was armed");
    assert!(
        !visible(&backend, tooltip.id()),
        "not shown before the tick"
    );
    rt.deliver(WidgetId::NONE, &Event::Timer { id: timer });
    assert!(visible(&backend, tooltip.id()), "shown after the tick");

    let bounds = backend.node(tooltip.id()).unwrap().1;
    assert!(
        bounds.top >= 30,
        "the popup sits under the label: {bounds:?}"
    );

    rt.deliver(label.id(), &Event::MouseLeave);
    assert!(!visible(&backend, tooltip.id()), "hidden on leave");
}

#[test]
fn leaving_before_the_delay_cancels_the_show() {
    let (backend, core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "target").unwrap();
    let tooltip = Tooltip::attach(&ui, label.id(), "tip").unwrap();
    let (rt, _log) = runtime(core);

    on_move(&rt, label.id());
    assert!(tooltip.shared.pending.get().is_some());
    rt.deliver(label.id(), &Event::MouseLeave);
    assert!(
        tooltip.shared.pending.get().is_none(),
        "the timer was killed"
    );
    let id = tooltip.id();
    rt.deliver(WidgetId::NONE, &Event::Timer { id: TimerId(1) });
    assert!(!visible(&backend, id), "a stale tick shows nothing");
}

#[test]
fn a_new_hover_re_arms_the_delay() {
    let (backend, core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "target").unwrap();
    let tooltip = Tooltip::attach(&ui, label.id(), "tip").unwrap();
    let (rt, _log) = runtime(core);

    on_move(&rt, label.id());
    let first = tooltip.shared.pending.get().unwrap();
    rt.deliver(WidgetId::NONE, &Event::Timer { id: first });
    assert!(visible(&backend, tooltip.id()));
    rt.deliver(label.id(), &Event::MouseLeave);
    assert!(!visible(&backend, tooltip.id()));

    on_move(&rt, label.id());
    let second = tooltip.shared.pending.get().expect("re-armed");
    assert!(second.0 != 0);
    rt.deliver(WidgetId::NONE, &Event::Timer { id: second });
    assert!(visible(&backend, tooltip.id()), "shown again");
}

#[test]
fn a_press_hides_and_suppresses_until_release() {
    let (backend, core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "target").unwrap();
    let tooltip = Tooltip::attach(&ui, label.id(), "tip").unwrap();
    let (rt, _log) = runtime(core);
    let modifiers = Modifiers::NONE;

    on_move(&rt, label.id());
    rt.deliver(
        WidgetId::NONE,
        &Event::Timer {
            id: tooltip.shared.pending.get().unwrap(),
        },
    );
    assert!(visible(&backend, tooltip.id()));

    rt.deliver(
        label.id(),
        &Event::MouseDown {
            x: 5,
            y: 5,
            button: MouseButton::Left,
            modifiers,
        },
    );
    assert!(!visible(&backend, tooltip.id()), "a press hides the tip");
    on_move(&rt, label.id());
    assert!(
        tooltip.shared.pending.get().is_none(),
        "a drag does not re-arm the tip"
    );

    rt.deliver(
        label.id(),
        &Event::MouseUp {
            x: 5,
            y: 5,
            button: MouseButton::Left,
            modifiers,
        },
    );
    on_move(&rt, label.id());
    assert!(tooltip.shared.pending.get().is_some(), "re-arms on release");
}

#[test]
fn a_tooltip_maps_no_messages() {
    let (_backend, core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "target").unwrap();
    let tooltip = Tooltip::attach(&ui, label.id(), "tip").unwrap();
    let (rt, log) = runtime(core);

    on_move(&rt, label.id());
    let id = tooltip.shared.pending.get().unwrap();
    rt.deliver(WidgetId::NONE, &Event::Timer { id });
    rt.deliver(label.id(), &Event::MouseLeave);
    rt.deliver(WidgetId::NONE, &Event::Wake);
    assert!(log.borrow().is_empty(), "the tooltip raised no message");
}

#[test]
fn attaching_keeps_the_targets_own_events() {
    let (_backend, core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok")
        .unwrap()
        .on_click(|| Some(7));
    let _tooltip = Tooltip::attach(&ui, button.id(), "tip").unwrap();
    let (rt, log) = runtime(core);
    let modifiers = Modifiers::NONE;

    rt.deliver(
        button.id(),
        &Event::MouseDown {
            x: 5,
            y: 5,
            button: MouseButton::Left,
            modifiers,
        },
    );
    rt.deliver(
        button.id(),
        &Event::MouseUp {
            x: 5,
            y: 5,
            button: MouseButton::Left,
            modifiers,
        },
    );
    rt.deliver(WidgetId::NONE, &Event::Wake);
    assert_eq!(*log.borrow(), vec![7], "the button still clicked");
}

#[test]
fn the_popup_paints_its_text_and_face() {
    let (backend, _core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "target").unwrap();
    let tooltip = Tooltip::attach(&ui, label.id(), "tip text").unwrap();

    backend.render(tooltip.id());
    let ops = backend.ops(tooltip.id());
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Clear(_))),
        "a popup face was painted: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "tip text")),
        "the tip text was painted: {ops:?}"
    );
}

#[test]
fn set_text_updates_the_painted_text() {
    let (backend, _core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "target").unwrap();
    let tooltip = Tooltip::attach(&ui, label.id(), "one").unwrap();

    assert_eq!(HasText::text(&tooltip), "one");
    tooltip.set_text("two");
    backend.render(tooltip.id());
    let ops = backend.ops(tooltip.id());
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "two")),
        "the new tip text was painted: {ops:?}"
    );
}

#[test]
fn show_and_hide_drive_the_popup_directly() {
    let (backend, _core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "target").unwrap();
    let tooltip = Tooltip::attach(&ui, label.id(), "tip").unwrap();

    assert!(!tooltip.is_visible());
    tooltip.show();
    assert!(tooltip.is_visible());
    assert!(visible(&backend, tooltip.id()));
    tooltip.hide();
    assert!(!tooltip.is_visible());
    assert!(!visible(&backend, tooltip.id()));
}

#[test]
fn dropping_the_tooltip_unregisters_its_listeners() {
    let (_backend, core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "target").unwrap();
    let tooltip = Tooltip::attach(&ui, label.id(), "tip").unwrap();
    assert!(!core.router().is_empty(), "the listener is registered");

    drop(tooltip);
    assert!(
        core.router().is_empty(),
        "the tooltip's listeners were removed"
    );
}
