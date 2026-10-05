use super::*;

#[test]
fn the_catalog_has_every_widget_kind() {
    let catalog = Catalog::xui();
    let kinds: Vec<&str> = catalog.kinds().collect();
    assert_eq!(
        kinds,
        [
            "Button",
            "CheckBox",
            "ComboBox",
            "Edit",
            "Group",
            "Hyperlink",
            "Label",
            "ListView",
            "MultilineEdit",
            "NumberField",
            "Panel",
            "ProgressBar",
            "RadioGroup",
            "Separator",
            "Slider",
            "Tabs",
            "ToggleButton",
        ]
    );
    let layouts: Vec<&str> = catalog.layouts().iter().map(|l| l.kind.as_str()).collect();
    assert_eq!(layouts, ["Row", "Column", "Wrap", "Grid", "Absolute"]);
}

#[test]
fn a_spec_comes_from_the_declaration() {
    let catalog = Catalog::xui();
    let edit = catalog.get("Edit").expect("Edit exists");
    assert_eq!(edit.description, "A single-line text field.");
    let placeholder = edit.property("placeholder").expect("placeholder exists");
    assert_eq!(placeholder.access, Access::DesignOnly);
    assert_eq!(placeholder.category, CATEGORY_APPEARANCE);
    assert_eq!(
        edit.default_event().map(|e| e.name.as_str()),
        Some("Change")
    );
    let fields: Vec<&str> = edit.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(&fields[..4], ["name", "array", "index", "text"]);
    assert!(fields.contains(&"fill") && fields.contains(&"anchor"));
}

#[test]
fn common_properties_apply_to_every_kind() {
    let catalog = Catalog::xui();
    for kind in catalog.kinds() {
        assert!(catalog.property(kind, "visible").is_some());
        assert!(catalog.property(kind, "left").is_some());
        assert!(
            catalog
                .get(kind)
                .and_then(|s| s.property("visible"))
                .is_none()
        );
    }
    assert_eq!(
        catalog.property("ListView", "selected").map(|p| p.default),
        Some(Value::Int(-1))
    );
}

#[test]
fn field_defaults_are_ron() {
    let catalog = Catalog::xui();
    let max = catalog
        .get("NumberField")
        .and_then(|spec| spec.fields.iter().find(|f| f.name == "max"))
        .expect("max is a field");
    assert_eq!((max.ty.as_str(), max.default.as_str()), ("f64", "100.0"));
    let fill = &catalog.layout_fields()[0];
    assert_eq!(
        (fill.name.as_str(), fill.ty.as_str()),
        ("fill", "Option<u32>")
    );
}

#[test]
fn the_catalog_serialises_to_json() {
    let json = serde_json::to_string(&Catalog::xui()).expect("the catalog serialises");
    assert!(json.contains("\"Button\""));
    assert!(json.contains("BottomRight") || json.contains("anchor"));
}
