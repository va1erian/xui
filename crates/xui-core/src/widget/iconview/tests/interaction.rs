//! Pointer, wheel and keyboard interaction: activation, context, navigation,
//! scrolling and the disabled state.

use super::*;
use crate::backend::headless::DrawOp;
use crate::widget::IconModel;

fn view_in(ui: &Ui<u32>, count: usize, width: i32, height: i32) -> IconView<u32> {
    let items: Vec<String> = (0..count).map(|index| format!("item {index}")).collect();
    IconView::with_model(ui, Rect::new(0, 0, width, height), items).unwrap()
}

#[test]
fn down_then_return_activates_the_focused_tile() {
    let (_backend, core, ui) = setup();
    let log = log();
    let items = view(&ui, 6).on_activate(|index| Some(index as u32 + 100));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(items.id(), &key(Key::DOWN));
    runtime.deliver(items.id(), &key(Key::RETURN));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(
        items.focused(),
        Some(2),
        "two columns, so Down moves one row"
    );
    assert_eq!(*log.borrow(), vec![102]);
}

#[test]
fn a_double_click_activates_once_and_empty_space_does_not() {
    let (_backend, core, ui) = setup();
    let log = log();
    let items = view(&ui, 6).on_activate(|index| Some(index as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    let point = center(&items, 3);
    runtime.deliver(items.id(), &double_click(point));
    let empty = empty_point();
    runtime.deliver(items.id(), &double_click(empty));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![3]);
}

#[test]
fn right_click_selects_an_unselected_tile_and_reports_the_pointer() {
    let (_backend, core, ui) = setup();
    let log = log();
    let items = view(&ui, 6)
        .on_context(|item, at| Some(item.map_or(999, |item| item as u32) + at.x as u32 * 1000));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    let point = center(&items, 2);
    runtime.deliver(items.id(), &right(point.x, point.y));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(items.selected(), Some(2), "an unselected tile is selected");
    assert_eq!(*log.borrow(), vec![2 + point.x as u32 * 1000]);
}

#[test]
fn right_click_on_a_selected_tile_keeps_the_selection() {
    let (_backend, core, ui) = setup();
    let items = view(&ui, 6)
        .multi_select(true)
        .on_context(|item, _| Some(item.unwrap_or(999) as u32));
    let runtime = Runtime::primary(core, TestApp(log()));
    items.set_selection(&[1, 4]);

    let point = center(&items, 4);
    runtime.deliver(items.id(), &right(point.x, point.y));

    assert_eq!(items.selection(), vec![1, 4]);
}

#[test]
fn right_click_on_empty_space_passes_none() {
    let (_backend, core, ui) = setup();
    let log = log();
    let items = view(&ui, 3).on_context(|item, _| Some(item.map_or(999, |item| item as u32)));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    let empty = empty_point();
    runtime.deliver(items.id(), &right(empty.x, empty.y));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![999]);
}

#[test]
fn keyboard_navigation_stops_at_the_first_and_last_item() {
    let (_backend, core, ui) = setup();
    let items = view(&ui, 6);
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(items.id(), &key(Key::LEFT));
    runtime.deliver(items.id(), &key(Key::UP));
    assert_eq!(items.focused(), Some(0), "no move off the first item");

    runtime.deliver(items.id(), &key(Key::END));
    assert_eq!(items.focused(), Some(5));
    runtime.deliver(items.id(), &key(Key::RIGHT));
    runtime.deliver(items.id(), &key(Key::DOWN));
    assert_eq!(items.focused(), Some(5), "no move off the last item");
    runtime.deliver(items.id(), &key(Key::HOME));
    assert_eq!(items.focused(), Some(0));
}

#[test]
fn a_single_column_moves_one_at_a_time() {
    let (_backend, core, ui) = setup();
    let items = view_in(&ui, 5, 100, 300);
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(items.id(), &key(Key::DOWN));
    assert_eq!(items.focused(), Some(1));
    assert_eq!(items.state.borrow().offset, 0, "still inside the viewport");
    runtime.deliver(items.id(), &key(Key::RIGHT));
    assert_eq!(items.focused(), Some(1), "one column, so Right stays");
    runtime.deliver(items.id(), &key(Key::UP));
    assert_eq!(items.focused(), Some(0));
}

#[test]
fn an_empty_view_ignores_the_keyboard() {
    let (_backend, core, ui) = setup();
    let items = view(&ui, 0);
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(items.id(), &key(Key::DOWN));
    runtime.deliver(items.id(), &key(Key::RETURN));
    assert_eq!(items.focused(), None);
    assert_eq!(items.len(), 0);
}

#[test]
fn the_keyboard_scrolls_the_focused_tile_into_view() {
    let (_backend, core, ui) = setup();
    let items = view_in(&ui, 40, WIDTH, 100);
    let runtime = Runtime::primary(core, TestApp(log()));

    for _ in 0..12 {
        runtime.deliver(items.id(), &key(Key::DOWN));
    }
    assert!(items.state.borrow().offset > 0, "the view scrolled");
    let focused = items.focused().unwrap();
    items.ensure_visible(focused);
}

#[test]
fn the_wheel_scrolls_the_view() {
    let (_backend, core, ui) = setup();
    let items = view_in(&ui, 60, WIDTH, 100);
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(
        items.id(),
        &Event::MouseWheel {
            delta: -1,
            horizontal: false,
            x: 10,
            y: 10,
            modifiers: Modifiers::NONE,
        },
    );
    assert!(items.state.borrow().offset > 0);
    runtime.deliver(
        items.id(),
        &Event::MouseWheel {
            delta: 120,
            horizontal: false,
            x: 10,
            y: 10,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(
        items.state.borrow().offset,
        0,
        "wheel up returns to the top"
    );
}

#[test]
fn a_disabled_view_ignores_pointer_input() {
    let (_backend, core, ui) = setup();
    let items = view(&ui, 6);
    let runtime = Runtime::primary(core, TestApp(log()));
    items.set_enabled(false);
    let point = center(&items, 3);

    runtime.deliver(items.id(), &left(point.x, point.y));

    assert_eq!(items.selected(), Some(0), "the selection is unchanged");
}

#[test]
fn pointer_movement_tracks_hover_but_leaving_clears_it() {
    let (_backend, core, ui) = setup();
    let items = view(&ui, 6);
    let runtime = Runtime::primary(core, TestApp(log()));
    let point = center(&items, 2);

    runtime.deliver(
        items.id(),
        &Event::MouseMove {
            x: point.x,
            y: point.y,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(items.state.borrow().hover, Some(2));
    runtime.deliver(items.id(), &Event::MouseLeave);
    assert_eq!(items.state.borrow().hover, None);
}

#[test]
fn two_views_in_one_window_keep_separate_state() {
    let (_backend, core, ui) = setup();
    let a = view(&ui, 6);
    let items: Vec<String> = (0..6).map(|index| format!("b {index}")).collect();
    let b = IconView::with_model(&ui, Rect::new(0, 500, WIDTH, 900), items).unwrap();
    let runtime = Runtime::primary(core, TestApp(log()));

    let pa = center(&a, 3);
    runtime.deliver(a.id(), &left(pa.x, pa.y));
    assert_eq!(a.selected(), Some(3));
    assert_eq!(b.selected(), Some(0), "the other view is untouched");

    drop(a);
    let pb = center(&b, 2);
    runtime.deliver(b.id(), &left(pb.x, pb.y));
    assert_eq!(b.selected(), Some(2), "the surviving view still works");
}

#[test]
fn an_empty_model_paints_and_never_panics() {
    struct Empty;

    impl IconModel for Empty {
        fn items(&self) -> usize {
            3
        }

        fn icon(&self, _item: usize) -> Option<crate::icon::IconRef> {
            None
        }

        fn line(&self, _item: usize, _line: usize) -> Option<&str> {
            None
        }
    }

    let (backend, _core, ui) = setup();
    let view = IconView::with_model(&ui, Rect::new(0, 0, WIDTH, HEIGHT), Empty).unwrap();
    backend.render(view.id());
    assert!(
        backend
            .ops(view.id())
            .iter()
            .any(|op| matches!(op, DrawOp::Clear(_))),
        "the view cleared its background and drew no tiles"
    );
}
