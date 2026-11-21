use super::*;

#[test]
fn a_click_maps_to_the_apps_message() {
    let (_backend, core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok")
        .unwrap()
        .on_click(|| Some(7));
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    click(&runtime, button.id());
    assert_eq!(*log.borrow(), vec![7]);
}

#[test]
fn a_disabled_button_ignores_clicks() {
    let (_backend, core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok")
        .unwrap()
        .on_click(|| Some(7));
    button.set_enabled(false);
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );

    click(&runtime, button.id());
    assert!(log.borrow().is_empty(), "a disabled button raised nothing");
}

#[test]
fn a_button_paints_a_face_and_its_label() {
    let (backend, _core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Send").unwrap();

    backend.render(button.id());
    let ops = backend.ops(button.id());
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Rounded(..))),
        "a button face was painted: {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "Send")),
        "the button label was painted: {ops:?}"
    );
}

#[test]
fn hovering_repaints_once_then_stays_quiet() {
    let (backend, core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok").unwrap();
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::new(RefCell::new(Vec::new())),
        },
    );
    let mv = |x| Event::MouseMove {
        x,
        y: 5,
        modifiers: Modifiers::NONE,
    };

    let before = backend.invalidations();
    runtime.deliver(button.id(), &mv(4));
    assert_eq!(
        backend.invalidations(),
        before + 1,
        "entering hover repaints the face"
    );
    for x in 5..25 {
        runtime.deliver(button.id(), &mv(x));
    }
    assert_eq!(
        backend.invalidations(),
        before + 1,
        "moving inside a hovered button repaints nothing"
    );

    runtime.deliver(button.id(), &Event::MouseLeave);
    assert_eq!(backend.invalidations(), before + 2, "leaving repaints");
}

/// The fill of the button's face in the last render.
fn face(backend: &HeadlessBackend, button: &Button<u32>) -> crate::Color {
    backend.render(button.id());
    backend
        .ops(button.id())
        .iter()
        .find_map(|op| match op {
            DrawOp::Rounded(_, _, color) => Some(*color),
            _ => None,
        })
        .expect("a face was painted")
}

#[test]
fn a_click_flashes_the_pressed_colour_and_fades_back() {
    use std::time::Duration;

    let (backend, core, ui) = setup();
    backend.set_now(Duration::ZERO);
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );
    let theme = ui.theme();

    // The keyboard gives no held-down state at all: the flash is its feedback.
    runtime.deliver(
        button.id(),
        &Event::KeyDown {
            key: Key::SPACE,
            modifiers: Modifiers::NONE,
            repeat: 1,
            system: false,
        },
    );
    assert_eq!(
        face(&backend, &button),
        theme.pressed,
        "the flash starts full"
    );

    backend.advance(Duration::from_millis(20));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
    let fading = face(&backend, &button);
    assert_ne!(fading, theme.pressed);
    assert_ne!(fading, theme.surface);

    backend.advance(Duration::from_millis(200));
    runtime.deliver(WidgetId::NONE, &Event::Frame);
    assert_eq!(face(&backend, &button), theme.surface, "back at rest");
    assert_eq!(backend.frame_requests(), 2, "and no frame asked for after");
}

#[test]
fn without_animation_a_click_paints_as_it_always_did() {
    // No clock on the headless backend: the change is instant.
    let (backend, core, ui) = setup();
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );
    click(&runtime, button.id());
    assert_eq!(face(&backend, &button), ui.theme().hover);
    assert_eq!(backend.frame_requests(), 0);
}

#[test]
fn the_hover_colour_fades_in_and_out() {
    use std::time::Duration;

    let (backend, core, ui) = setup();
    backend.set_now(Duration::ZERO);
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );
    let theme = ui.theme();
    let frame = |millis| {
        backend.advance(Duration::from_millis(millis));
        runtime.deliver(WidgetId::NONE, &Event::Frame);
    };

    runtime.deliver(
        button.id(),
        &Event::MouseMove {
            x: 5,
            y: 5,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(face(&backend, &button), theme.surface, "it starts cold");
    frame(60);
    let warming = face(&backend, &button);
    assert_ne!(warming, theme.surface);
    assert_ne!(warming, theme.hover);
    frame(300);
    assert_eq!(face(&backend, &button), theme.hover);

    runtime.deliver(button.id(), &Event::MouseLeave);
    assert_eq!(face(&backend, &button), theme.hover, "and cools from there");
    frame(60);
    assert_ne!(face(&backend, &button), theme.hover);
    frame(300);
    assert_eq!(face(&backend, &button), theme.surface);
    let asked = backend.frame_requests();
    frame(16);
    assert_eq!(backend.frame_requests(), asked, "nothing moves any more");
}

#[test]
fn a_button_disabled_mid_flash_paints_the_plain_disabled_face() {
    use std::time::Duration;

    let (backend, core, ui) = setup();
    backend.set_now(Duration::ZERO);
    let button = Button::new(&ui, Rect::new(0, 0, 80, 28), "Ok").unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = Runtime::primary(
        core,
        TestApp {
            log: Rc::clone(&log),
        },
    );
    click(&runtime, button.id());
    // What a submit handler does to its own button.
    button.set_enabled(false);
    assert_eq!(face(&backend, &button), ui.theme().surface);
}
