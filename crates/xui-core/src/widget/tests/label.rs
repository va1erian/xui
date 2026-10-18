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

#[test]
fn a_font_override_reaches_the_measure_and_the_property() {
    use crate::property::{Font, PropertyType};
    use crate::units::Dip;

    let (_backend, _core, ui) = setup();
    let label = Label::new(&ui, Rect::new(0, 0, 120, 30), "hello").unwrap();
    let small = label.text_size(&ui, 96);
    assert_eq!(
        label.property("font"),
        Some(Value::Font(Font::new(Dip(12.0)))),
        "the body font is reported before any override"
    );

    assert!(label.set_property("font", Value::Font(Font::new(Dip(24.0)).bold())));
    assert_eq!(label.current_font(), Font::new(Dip(24.0)).bold());
    let large = label.text_size(&ui, 96);
    assert!(
        large.width > small.width && large.height > small.height,
        "24 DIP measures bigger than 12: {small:?} then {large:?}"
    );
    assert!(
        !label.set_property("font", Value::Text("no".to_owned())),
        "a mismatched value is refused"
    );
    assert_eq!(
        label
            .schema()
            .iter()
            .map(|info| (info.name, info.ty))
            .collect::<Vec<_>>(),
        [("text", PropertyType::Text), ("font", PropertyType::Font)]
    );

    let title = Label::new(&ui, Rect::new(0, 0, 120, 30), "Big")
        .unwrap()
        .title();
    assert!(title.set_property("font", Value::Font(Font::new(Dip(18.0)))));
    assert!(
        title.current_font().bold,
        "a title stays bold under a regular override"
    );

    let caption = Label::new(&ui, Rect::new(0, 0, 120, 30), "cap")
        .unwrap()
        .caption();
    assert!(caption.current_font().bold, "a caption is bold by default");
    assert!(caption.set_property("font", Value::Font(Font::new(Dip(10.0)).italic())));
    let caption_font = caption.current_font();
    assert!(
        caption_font.bold && caption_font.italic,
        "a caption stays bold under an italic override: {caption_font:?}"
    );

    label.set_font(None);
    assert_eq!(
        label.current_font(),
        Font::new(Dip(12.0)),
        "clearing the override returns to the kind's theme font"
    );
}
