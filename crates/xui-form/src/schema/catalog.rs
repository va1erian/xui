#![forbid(unsafe_code)]

//! [`Catalog`]: every node kind's spec, gathered from the model.

use serde::Serialize;

use super::{FieldSpec, LayoutSpec, PropertySpec, Specs, WidgetSpec, common_properties};
use crate::model::{self, Absolute, Grid, Stack};

/// The schema of every node kind a form may hold: the widgets with their
/// properties and events, the layouts, the properties every widget has, the
/// layout fields every node has, and the fields of the form and of a page.
#[derive(Clone, Debug, Serialize)]
pub struct Catalog {
    widgets: Specs,
    layouts: Vec<LayoutSpec>,
    common: Vec<PropertySpec>,
    layout_fields: Vec<FieldSpec>,
    form_fields: Vec<FieldSpec>,
    page_fields: Vec<FieldSpec>,
}

impl Default for Catalog {
    fn default() -> Catalog {
        Catalog::xui()
    }
}

/// The names of the layout fields every node has.
const LAYOUT_FIELDS: [&str; 9] = [
    "fill",
    "width",
    "height",
    "max_width",
    "max_height",
    "align",
    "span",
    "at",
    "anchor",
];

impl Catalog {
    /// The catalog of the portable `xui-core` widgets and layouts.
    pub fn xui() -> Catalog {
        let widgets = [
            model::Panel::spec(),
            model::Group::spec(),
            model::Tabs::spec(),
            model::Label::spec(),
            model::Button::spec(),
            model::Hyperlink::spec(),
            model::CheckBox::spec(),
            model::ToggleButton::spec(),
            model::Edit::spec(),
            model::MultilineEdit::spec(),
            model::NumberField::spec(),
            model::Slider::spec(),
            model::ProgressBar::spec(),
            model::RadioGroup::spec(),
            model::ComboBox::spec(),
            model::ListView::spec(),
            model::Separator::spec(),
        ];
        let layout = |kind: &str, description: &str, fields: Vec<FieldSpec>| LayoutSpec {
            kind: kind.to_owned(),
            description: description.to_owned(),
            fields,
        };
        let layout_fields = Stack::fields()
            .into_iter()
            .filter(|field| LAYOUT_FIELDS.contains(&field.name.as_str()))
            .collect();
        Catalog {
            widgets: widgets
                .into_iter()
                .map(|spec| (spec.kind.clone(), spec))
                .collect(),
            layouts: vec![
                layout("Row", "Entries left to right.", Stack::fields()),
                layout("Column", "Entries top to bottom.", Stack::fields()),
                layout(
                    "Wrap",
                    "Entries left to right at their natural sizes, wrapping into lines.",
                    Stack::fields(),
                ),
                layout(
                    "Grid",
                    "Entries flowing into columns, row by row.",
                    Grid::fields(),
                ),
                layout(
                    "Absolute",
                    "Entries at fixed rectangles, following their anchors.",
                    Absolute::fields(),
                ),
            ],
            common: common_properties(),
            layout_fields,
            form_fields: form_fields(),
            page_fields: model::Page::fields(),
        }
    }

    /// The spec of the widget kind `kind`, if known.
    pub fn get(&self, kind: &str) -> Option<&WidgetSpec> {
        self.widgets.get(kind)
    }

    /// Whether `kind` is a known widget kind.
    pub fn contains(&self, kind: &str) -> bool {
        self.widgets.contains_key(kind)
    }

    /// The widget kinds, in sorted order.
    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.widgets.keys().map(String::as_str)
    }

    /// The layout kinds.
    pub fn layouts(&self) -> &[LayoutSpec] {
        &self.layouts
    }

    /// The runtime properties every widget has.
    pub fn common_properties(&self) -> &[PropertySpec] {
        &self.common
    }

    /// The layout fields every node has (`fill`, `align`, `at`, …).
    pub fn layout_fields(&self) -> &[FieldSpec] {
        &self.layout_fields
    }

    /// The fields of the `Form` itself.
    pub fn form_fields(&self) -> &[FieldSpec] {
        &self.form_fields
    }

    /// The fields of a `Tabs` page.
    pub fn page_fields(&self) -> &[FieldSpec] {
        &self.page_fields
    }

    /// The runtime property `name` of `kind`: the widget's own first, then
    /// the common ones.
    pub fn property(&self, kind: &str, name: &str) -> Option<PropertySpec> {
        self.get(kind)
            .and_then(|spec| spec.property(name))
            .or_else(|| self.common.iter().find(|property| property.name == name))
            .cloned()
    }

    /// The names of every runtime property of `kind`: the widget's own first,
    /// then the common ones.
    pub fn property_names(&self, kind: &str) -> Vec<String> {
        let own = self.get(kind).map(|spec| spec.properties.as_slice());
        own.unwrap_or_default()
            .iter()
            .chain(&self.common)
            .map(|property| property.name.clone())
            .collect()
    }
}

/// The `Form`'s fields.
fn form_fields() -> Vec<FieldSpec> {
    let defaults = model::Form::default();
    vec![
        FieldSpec::new(
            "name",
            "String",
            &defaults.name,
            &["The form's name, which scripts and multi-form hosts address it by."],
        ),
        FieldSpec::new("title", "String", &defaults.title, &["The window title."]),
        FieldSpec::new(
            "size",
            "(Length, Length)",
            &defaults.size,
            &["The window's initial client size, in design units."],
        ),
        FieldSpec::new(
            "resizable",
            "bool",
            &defaults.resizable,
            &["Whether the window can be resized."],
        ),
        FieldSpec::new("root", "Node", &defaults.root, &["The window's content."]),
    ]
}
