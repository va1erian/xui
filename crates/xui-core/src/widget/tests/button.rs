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
