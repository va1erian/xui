//! The header: sorting and column resizing.

use super::*;

#[test]
fn a_header_click_toggles_the_sort_and_calls_the_hook() {
    let (_backend, core, ui) = setup();
    let log = log();
    let model: Vec<Vec<String>> = vec![vec!["a".into(), "b".into()]];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 300, 120), model)
        .unwrap()
        .column("A", super::Fill)
        .column("B", Dip::new(50.0))
        .on_sort(|column| Some(column as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(list.id(), &down(275, 12));
    assert_eq!(
        list.sort_indicator(),
        Some((1, super::SortDirection::Ascending))
    );
    runtime.deliver(list.id(), &down(275, 12));
    assert_eq!(
        list.sort_indicator(),
        Some((1, super::SortDirection::Descending))
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(*log.borrow(), vec![1, 1]);
}

#[test]
fn dragging_a_header_boundary_resizes_the_column() {
    let (_backend, core, ui) = setup();
    let log = log();
    let model: Vec<Vec<String>> = vec![vec!["a".into(), "b".into()]];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 300, 120), model)
        .unwrap()
        .column("A", Dip::new(100.0))
        .column("B", super::Fill)
        .on_resize(|column, width| Some(column as u32 * 1000 + width.value() as u32));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(list.id(), &down(100, 12));
    runtime.deliver(
        list.id(),
        &Event::MouseMove {
            x: 140,
            y: 12,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(
        list.id(),
        &Event::MouseUp {
            x: 140,
            y: 12,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(
        list.column_width(0),
        Some(super::ColumnWidth::Fixed(Dip::new(140.0)))
    );
    assert_eq!(*log.borrow(), vec![140]);
}

#[test]
fn dragging_a_fill_boundary_converts_it_to_fixed() {
    let (_backend, core, ui) = setup();
    let model: Vec<Vec<String>> = vec![vec!["a".into(), "b".into()]];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 300, 120), model)
        .unwrap()
        .column("A", super::Fill)
        .column("B", super::Fill);
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(list.id(), &down(150, 12));
    runtime.deliver(
        list.id(),
        &Event::MouseMove {
            x: 200,
            y: 12,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(
        list.id(),
        &Event::MouseUp {
            x: 200,
            y: 12,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(
        list.column_width(0),
        Some(super::ColumnWidth::Fixed(Dip::new(200.0)))
    );
    assert_eq!(list.column_width(1), Some(super::ColumnWidth::Fill));
}

#[test]
fn a_double_click_on_a_boundary_auto_sizes_the_column() {
    let (_backend, core, ui) = setup();
    let model: Vec<Vec<String>> = vec![vec!["abcdefgh".into(), "b".into()]];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 300, 120), model)
        .unwrap()
        .column("Header", Dip::new(40.0))
        .column("B", super::Fill);
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(
        list.id(),
        &Event::MouseDoubleClick {
            x: 40,
            y: 12,
            button: MouseButton::Left,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    // The widest cell ("abcdefgh", 8 chars at 6px) plus a 6px inset on each
    // side of the headless measurement.
    assert_eq!(
        list.column_width(0),
        Some(super::ColumnWidth::Fixed(Dip::new(60.0)))
    );
}

#[test]
fn a_plain_header_click_still_sorts() {
    let (_backend, core, ui) = setup();
    let log = log();
    let model: Vec<Vec<String>> = vec![vec!["a".into(), "b".into()]];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 300, 120), model)
        .unwrap()
        .column("A", Dip::new(100.0))
        .column("B", super::Fill)
        .on_sort(|column| Some(column as u32))
        .on_resize(|_, _| Some(999));
    let runtime = Runtime::primary(core, TestApp(Rc::clone(&log)));

    runtime.deliver(list.id(), &down(40, 12));
    runtime.deliver(WidgetId::NONE, &Event::Wake);

    assert_eq!(
        list.sort_indicator(),
        Some((0, super::SortDirection::Ascending))
    );
    assert_eq!(*log.borrow(), vec![0]);
}

#[test]
fn the_sort_arrow_does_not_overlap_a_long_sorted_column_title() {
    let (backend, _core, ui) = setup();
    let model: Vec<Vec<String>> = vec![vec!["a".into(), "b".into()]];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 300, 120), model)
        .unwrap()
        .column("A quite long column title", Dip::new(120.0))
        .column("B", super::Fill);
    list.set_sort_indicator(0, super::SortDirection::Ascending);

    backend.render(list.id());
    let ops = backend.ops(list.id());
    let arrow_left = ops
        .iter()
        .find_map(|op| match op {
            DrawOp::Polygon(points, _) => points.iter().map(|p| p.x).min(),
            _ => None,
        })
        .expect("the sorted column draws an arrow");
    let title_right = ops
        .iter()
        .find_map(|op| match op {
            DrawOp::Text(rect, text, _) if text.starts_with('A') => Some(rect.right),
            _ => None,
        })
        .expect("the sorted column's title is drawn");
    assert!(
        title_right <= arrow_left,
        "the title (ending at {title_right}) must leave room for the arrow (starting at {arrow_left})"
    );
}
