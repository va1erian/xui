use super::*;

#[test]
fn clicking_a_row_selects_and_raises_a_message() {
    let (runtime, tree) = harness_flat();
    send(&runtime, &tree, &down(80, 5));
    assert_eq!(tree.selected(), Some(0));
    assert_eq!(log(), vec![0]);
}

#[test]
fn a_right_click_reports_the_row_and_pointer_position() {
    LOG.with(|log| log.borrow_mut().clear());
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    let at = Rc::new(Cell::new(None));
    let seen = Rc::clone(&at);
    let tree = TreeView::new(&ui, Rect::new(0, 0, 120, 88), &flat())
        .unwrap()
        .on_context(move |id, point| {
            seen.set(Some((id, point)));
            Some(7)
        });
    let runtime = Runtime::primary(core, TestApp);

    send(&runtime, &tree, &right(80, 5));

    assert_eq!(at.get(), Some((0, Point::new(80, 5))));
    assert_eq!(tree.selected(), Some(0));
}

#[test]
fn clicking_a_chevron_toggles_and_raises_a_message() {
    let (runtime, tree) = harness_flat();
    send(&runtime, &tree, &down(10, 11));
    send(&runtime, &tree, &down(10, 11));
    assert_eq!(tree.selected(), None, "a chevron click does not select");
    assert_eq!(log(), vec![101, 100]);
}

#[test]
fn the_arrow_keys_expand_then_collapse_a_selected_row() {
    let (runtime, tree) = harness_flat();
    tree.select(Some(0));
    send(&runtime, &tree, &key(Key::RIGHT));
    send(&runtime, &tree, &key(Key::LEFT));
    assert_eq!(log(), vec![101, 100]);
}

fn double(x: i32, y: i32) -> Event {
    Event::MouseDoubleClick {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn activating_tree() -> (Rc<Runtime<TestApp>>, TreeView<u32>) {
    LOG.with(|log| log.borrow_mut().clear());
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    let tree = TreeView::new(&ui, Rect::new(0, 0, 120, 88), &flat())
        .unwrap()
        .on_select(|id| Some(id as u32))
        .on_activate(|id| Some(500 + id as u32));
    (Runtime::primary(core, TestApp), tree)
}

#[test]
fn double_clicking_a_row_activates_it() {
    let (runtime, tree) = activating_tree();
    send(&runtime, &tree, &double(80, 5));
    assert_eq!(tree.selected(), Some(0));
    assert_eq!(log(), vec![500]);
}

#[test]
fn double_clicking_a_chevron_does_not_activate() {
    let (runtime, tree) = activating_tree();
    send(&runtime, &tree, &double(10, 11));
    assert!(log().is_empty());
}

#[test]
fn enter_activates_the_selected_row_when_a_mapper_is_set() {
    let (runtime, tree) = activating_tree();
    tree.select(Some(0));
    send(&runtime, &tree, &key(Key::RETURN));
    assert_eq!(log(), vec![500]);
}

#[test]
fn enter_falls_back_to_the_select_mapper_without_on_activate() {
    let (runtime, tree) = harness_flat();
    tree.select(Some(0));
    send(&runtime, &tree, &key(Key::RETURN));
    assert_eq!(log(), vec![0]);
}
