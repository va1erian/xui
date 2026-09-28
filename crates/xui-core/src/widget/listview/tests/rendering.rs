//! Painting: virtualization, the wheel, columns and cell clipping.

use super::*;

#[test]
fn a_large_model_renders_only_visible_rows() {
    let (backend, _core, ui) = setup();
    let model: Vec<String> = (0..1000).map(|index| format!("row {index}")).collect();
    let list = ListView::with_model(&ui, Rect::new(0, 0, 200, 88), model).unwrap();

    backend.render(list.id());
    assert_eq!(
        texts(&backend, list.id()),
        vec!["row 0", "row 1", "row 2", "row 3"]
    );
}

#[test]
fn the_wheel_scrolls_the_virtual_window() {
    let (backend, core, ui) = setup();
    let model: Vec<String> = (0..1000).map(|index| format!("row {index}")).collect();
    let list = ListView::with_model(&ui, Rect::new(0, 0, 200, 88), model).unwrap();
    let runtime = Runtime::primary(core, TestApp(log()));

    runtime.deliver(
        list.id(),
        &Event::MouseWheel {
            delta: -1,
            horizontal: false,
            x: 5,
            y: 5,
            modifiers: Modifiers::NONE,
        },
    );
    runtime.deliver(WidgetId::NONE, &Event::Wake);
    backend.render(list.id());

    assert_eq!(
        texts(&backend, list.id()),
        vec!["row 3", "row 4", "row 5", "row 6"]
    );
}

#[test]
fn columns_with_a_fixed_and_a_fill_width_align() {
    let (backend, _core, ui) = setup();
    let model: Vec<Vec<String>> = vec![
        vec!["a0".into(), "b0".into()],
        vec!["a1".into(), "b1".into()],
    ];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 300, 120), model)
        .unwrap()
        .column("A", Dip::new(100.0))
        .column("B", super::Fill);

    backend.render(list.id());
    let ops = backend.ops(list.id());
    // The fill column begins right after the 100px fixed column (plus inset).
    let b0 = text_rect(&ops, "b0").expect("second column cell");
    let a0 = text_rect(&ops, "a0").expect("first column cell");
    assert_eq!(a0.left, 6);
    assert_eq!(b0.left, 106);
    assert!(text_rect(&ops, "B").is_some(), "the header is drawn");
}

#[test]
fn cell_text_is_clipped_to_its_column() {
    let (backend, _core, ui) = setup();
    let model: Vec<Vec<String>> = vec![
        vec!["a title far too long for its column".into(), "b".into()],
        vec!["short".into(), "c".into()],
    ];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 200, 120), model)
        .unwrap()
        .column("A", Dip::new(60.0))
        .column("B", super::Fill);

    backend.render(list.id());
    assert!(
        every_text_is_clipped(&backend.ops(list.id())),
        "a cell's text must sit inside the clip pushed for its column"
    );
}

#[test]
fn a_cell_too_narrow_for_its_text_is_ellipsized_not_clipped_bare() {
    let (backend, _core, ui) = setup();
    let model: Vec<Vec<String>> = vec![vec!["abcdefghij".into(), "b".into()]];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 200, 120), model)
        .unwrap()
        .column("A", Dip::new(40.0))
        .column("B", super::Fill);

    backend.render(list.id());
    // 10 chars at the headless measurer's 6px advance would take 60px, far
    // wider than the 28px the 40px-wide column leaves after its padding, so
    // the cell must be cut short and end with the ellipsis, not the raw text.
    let drawn = texts(&backend, list.id());
    assert!(
        drawn.iter().any(|text| text.ends_with('\u{2026}')),
        "expected an ellipsized cell among {drawn:?}"
    );
    assert!(
        !drawn.contains(&"abcdefghij".to_string()),
        "the overflowing text must not be drawn in full: {drawn:?}"
    );
}

#[test]
fn a_cell_that_fits_draws_unchanged() {
    let (backend, _core, ui) = setup();
    let model: Vec<Vec<String>> = vec![vec!["short".into(), "b".into()]];
    let list = ListView::with_model(&ui, Rect::new(0, 0, 300, 120), model)
        .unwrap()
        .column("A", Dip::new(200.0))
        .column("B", super::Fill);

    backend.render(list.id());
    assert!(texts(&backend, list.id()).contains(&"short".to_string()));
}

#[test]
fn a_model_can_attach_cell_data() {
    struct Row {
        text: &'static str,
        data: CellData,
    }
    struct Model(Vec<Row>);

    impl ListModel for Model {
        fn rows(&self) -> usize {
            self.0.len()
        }

        fn cell(&self, row: usize, _column: usize) -> Option<&str> {
            self.0.get(row).map(|row| row.text)
        }

        fn data(&self, row: usize, _column: usize) -> Option<CellData> {
            self.0.get(row).map(|row| Rc::clone(&row.data))
        }
    }

    let (_backend, _core, ui) = setup();
    let model = Model(vec![
        Row {
            text: "one",
            data: Rc::new(41u32),
        },
        Row {
            text: "two",
            data: Rc::new(42u32),
        },
    ]);
    let list = ListView::with_model(&ui, Rect::new(0, 0, 200, 120), model)
        .unwrap()
        .column("A", super::Fill);

    assert_eq!(list.cell_text(1, 0).as_deref(), Some("two"));
    let data = list.cell_data(1, 0).expect("data");
    assert_eq!(data.downcast_ref::<u32>(), Some(&42));
}
