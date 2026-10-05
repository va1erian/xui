use super::*;

#[test]
fn the_builtin_catalog_has_the_documented_kinds() {
    let catalog = Catalog::xui();
    let kinds: Vec<&str> = catalog.kinds().collect();
    assert_eq!(
        kinds,
        [
            "Button",
            "CheckBox",
            "ComboBox",
            "Edit",
            "GroupBox",
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
            "ToggleButton",
        ]
    );
}

#[test]
fn aliases_resolve_to_the_canonical_kind() {
    let mut catalog = Catalog::xui();
    catalog.alias("CommandButton", "Button");
    assert_eq!(catalog.resolve("CommandButton"), Some("Button"));
    assert_eq!(
        catalog.get("CommandButton").map(|spec| spec.kind.as_str()),
        Some("Button")
    );
    assert!(catalog.contains("CommandButton"));
    assert_eq!(catalog.resolve("Nope"), None);
}

#[test]
fn common_properties_apply_to_every_kind() {
    let catalog = Catalog::xui();
    for kind in catalog.kinds() {
        let spec = catalog.get(kind).expect("kind exists");
        assert!(catalog.property(kind, "left").is_some());
        assert!(catalog.property(kind, "anchor").is_some());
        // The spec itself holds only widget-specific properties.
        assert!(spec.property("left").is_none());
    }
}

#[test]
fn width_defaults_to_the_widget_size() {
    let catalog = Catalog::xui();
    let width = catalog
        .property("Button", "width")
        .expect("width is common");
    assert_eq!(width.default, Value::Int(100));
    let height = catalog
        .property("Panel", "height")
        .expect("height is common");
    assert_eq!(height.default, Value::Int(120));
}

#[test]
fn only_named_widget_supports_its_events() {
    let catalog = Catalog::xui();
    let button = catalog.get("Button").expect("Button exists");
    assert!(button.event("Click").is_some());
    assert_eq!(
        button.default_event().map(|event| event.name.as_str()),
        Some("Click")
    );
    let label = catalog.get("Label").expect("Label exists");
    assert!(label.events.is_empty());
}

#[test]
fn container_rules_accept_children() {
    let catalog = Catalog::xui();
    assert!(catalog.is_container("Panel"));
    assert!(catalog.accepts_child("GroupBox", "Button"));
    assert!(!catalog.accepts_child("Button", "Label"));
    assert!(!catalog.is_container("Label"));
}

#[test]
fn the_catalog_serialises_to_json() {
    let catalog = Catalog::xui();
    let json = serde_json::to_string(&catalog).expect("catalog serialises");
    assert!(json.contains("\"Button\""));
    assert!(json.contains("stretch_horizontal"));
}
