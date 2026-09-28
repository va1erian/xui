use super::*;

#[test]
fn a_label_paints_its_text_from_the_theme() {
    let (backend, _core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "hello").unwrap();

    backend.render(label.id());
    let ops = backend.ops(label.id());
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "hello")),
        "the label painted its text: {ops:?}"
    );
}

#[test]
fn changing_a_labels_text_repaints_it() {
    let (backend, _core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "one").unwrap();
    label.set_text("two");

    backend.render(label.id());
    let ops = backend.ops(label.id());
    assert!(
        ops.iter()
            .any(|op| matches!(op, DrawOp::Text(_, text, _) if text == "two")),
        "the new text was painted: {ops:?}"
    );
}
