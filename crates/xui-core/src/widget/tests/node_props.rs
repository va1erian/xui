//! The `enabled` and `visible` properties every widget reports, round-tripped
//! through each widget's `Properties` and checked against the `Ui`.

use super::*;
use crate::widget::{
    ColorPicker, GridView, GroupBox, Hyperlink, IconView, ListView, MaterialStatusBar, Menu,
    MultilineEdit, NumberField, ScrollView, Separator, Split, StatusBar, Tabs, ToggleButton,
    TreeView,
};

/// Hides and shows `widget`, then disables it when `has_enabled`, checking
/// each change through both its properties and the `Ui`; checks too that its
/// schema lists its properties in the same order.
fn round_trip(ui: &Ui<u32>, widget: &dyn Properties, id: WidgetId, has_enabled: bool) {
    assert_eq!(widget.property("visible"), Some(Value::Bool(true)));
    assert!(widget.set_property("visible", Value::Bool(false)));
    assert!(!ui.is_visible(id));
    assert_eq!(widget.property("visible"), Some(Value::Bool(false)));
    assert!(widget.set_property("visible", Value::Bool(true)));
    assert!(ui.is_visible(id));
    assert!(!widget.set_property("visible", Value::Integer(0)));

    assert_eq!(widget.property("enabled").is_some(), has_enabled);
    assert_eq!(
        widget.set_property("enabled", Value::Bool(false)),
        has_enabled
    );
    assert_eq!(ui.is_enabled(id), !has_enabled);
    assert!(!widget.set_property("enabled", Value::Integer(0)));

    // A property with no value yet (a colour picker's colour) is in the
    // schema only, so compare the schema's entries that have one.
    let props: Vec<_> = widget.properties().iter().map(|p| p.name).collect();
    let mut schema: Vec<_> = widget.schema().iter().map(|info| info.name).collect();
    schema.retain(|name| props.contains(name));
    assert_eq!(schema, props);
}

#[test]
fn every_widget_round_trips_visible_and_enabled() {
    let (_backend, _core, ui) = setup();
    let r = Rect::new(0, 0, 160, 120);

    let button = Button::new(&ui, r, "Ok").unwrap();
    round_trip(&ui, &button, button.id(), true);
    let check = CheckBox::new(&ui, r, "Check").unwrap();
    round_trip(&ui, &check, check.id(), true);
    let toggle = ToggleButton::new(&ui, r, "Bold").unwrap();
    round_trip(&ui, &toggle, toggle.id(), true);
    let link = Hyperlink::new(&ui, r, "link").unwrap();
    round_trip(&ui, &link, link.id(), true);
    let text = MultilineEdit::new(&ui, r, "text").unwrap();
    round_trip(&ui, &text, text.id(), true);
    let number = NumberField::new(&ui, r, 0.0, 10.0, 1.0).unwrap();
    round_trip(&ui, &number, number.id(), true);
    let progress = ProgressBar::new(&ui, r, 100).unwrap();
    round_trip(&ui, &progress, progress.id(), true);
    let status = StatusBar::new(&ui, r, &["a", "b"]).unwrap();
    round_trip(&ui, &status, status.id(), true);
    let material = MaterialStatusBar::new(&ui, r, &["a"]).unwrap();
    round_trip(&ui, &material, material.id(), true);
    let list = ListView::new(&ui, r, &["a", "b"]).unwrap();
    round_trip(&ui, &list, list.id(), true);
    let grid = GridView::new(&ui, r, &["a", "b"]).unwrap();
    round_trip(&ui, &grid, grid.id(), true);
    let icons = IconView::new(&ui, r, &["a", "b"]).unwrap();
    round_trip(&ui, &icons, icons.id(), true);
    let tree = TreeView::new(&ui, r, &[]).unwrap();
    round_trip(&ui, &tree, tree.id(), true);
    let tabs = Tabs::new(&ui, r).unwrap();
    round_trip(&ui, &tabs, tabs.id(), true);
    let scroll = ScrollView::new(&ui, r).unwrap();
    round_trip(&ui, &scroll, scroll.id(), true);
    let split = Split::row(&ui, r).unwrap();
    round_trip(&ui, &split, split.id(), true);

    let edit = Edit::new(&ui, r, "text").unwrap();
    round_trip(&ui, &edit, edit.id(), false);
    let group = GroupBox::new(&ui, r, "Group").unwrap();
    round_trip(&ui, &group, group.id(), false);
    let separator = Separator::new(&ui, r).unwrap();
    round_trip(&ui, &separator, separator.id(), false);
    let colors = ColorPicker::new(&ui, r, &[crate::Color::rgb(1, 2, 3)]).unwrap();
    round_trip(&ui, &colors, colors.id(), false);
    let menu = Menu::bar(&ui, r).unwrap();
    round_trip(&ui, &menu, menu.id().unwrap(), false);
}
