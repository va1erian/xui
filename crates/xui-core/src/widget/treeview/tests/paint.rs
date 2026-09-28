use super::*;

#[test]
fn guides_blend_into_the_row_highlight() {
    for theme in [Theme::light(), Theme::dark()] {
        assert_eq!(
            guide_color(&theme, None),
            theme.border,
            "a plain row keeps the border guide"
        );
        let on_accent = guide_color(&theme, Some(theme.accent));
        assert_ne!(on_accent, theme.border, "a selected row blends the guide");
        assert!(
            on_accent.contrast_ratio(theme.accent) < theme.border.contrast_ratio(theme.accent),
            "the blend lowers the guide's contrast over the selection"
        );
        let on_hover = guide_color(&theme, Some(theme.hover));
        assert!(
            on_hover.contrast_ratio(theme.hover) < theme.border.contrast_ratio(theme.hover),
            "and over the hover fill"
        );
    }
}

#[test]
fn indent_guides_can_be_turned_off() {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    let _runtime = Runtime::primary(core, TestApp);

    // The trailing root keeps "root"'s guide running through "leaf".
    let rows = [
        TreeRow::new("root", 0).expandable(true).expanded(true),
        TreeRow::new("leaf", 1),
        TreeRow::new("next", 0),
    ];
    let with = TreeView::new(&ui, Rect::new(0, 0, 120, 66), &rows).unwrap();
    let without = TreeView::new(&ui, Rect::new(0, 70, 120, 66), &rows)
        .unwrap()
        .indent_guides(false);
    backend.render(with.id());
    backend.render(without.id());

    let guide_lines = |id: WidgetId| {
        backend
            .ops(id)
            .iter()
            .filter(|op| matches!(op, DrawOp::Line(from, to, _, _) if from.x == to.x))
            .count()
    };
    assert!(
        guide_lines(with.id()) > 0,
        "a depth-1 row draws its ancestor's guide"
    );
    assert_eq!(
        guide_lines(without.id()),
        0,
        "indent_guides(false) draws no vertical guides"
    );
}

#[test]
fn a_row_icon_paints_before_the_label() {
    let backend = Rc::new(HeadlessBackend::new());
    let window = backend.open_window(&PlatformSpec::new("t")).unwrap();
    let core = Core::new(backend.clone(), window);
    let ui = Ui::new(Rc::clone(&core));
    let _runtime = Runtime::primary(core, TestApp);

    let image = Image::from_rgba(1, 1, vec![255, 0, 0, 255]).unwrap();
    let plain = TreeView::new(&ui, Rect::new(0, 0, 120, 22), &[TreeRow::new("Folder", 0)]).unwrap();
    let glyph = TreeView::new(
        &ui,
        Rect::new(0, 24, 120, 46),
        &[TreeRow::new("Folder", 0).icon(Glyph::Folder)],
    )
    .unwrap();
    let bitmap = TreeView::new(
        &ui,
        Rect::new(0, 48, 120, 70),
        &[TreeRow::new("Art", 0).icon(image)],
    )
    .unwrap();

    backend.render(plain.id());
    backend.render(glyph.id());
    backend.render(bitmap.id());

    assert!(
        backend.ops(glyph.id()).len() > backend.ops(plain.id()).len(),
        "a glyph row paints more than a text-only row"
    );
    assert!(
        backend
            .ops(bitmap.id())
            .iter()
            .any(|op| matches!(op, DrawOp::Image(..))),
        "a bitmap row icon draws the image"
    );
}
