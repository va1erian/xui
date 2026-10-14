#![forbid(unsafe_code)]

//! The format-1 file, as TOML tables, and what its nodes mean.

use serde::Deserialize;

/// One `[[node]]` of a format-1 form.
#[derive(Clone, Debug, Deserialize)]
pub(super) struct V1Node {
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) parent: Option<String>,
    #[serde(flatten)]
    pub(super) props: toml::Table,
}

/// A format-1 form.
#[derive(Debug, Deserialize)]
pub(super) struct V1Form {
    pub(super) format: u32,
    pub(super) window: toml::Table,
    #[serde(default, rename = "node")]
    pub(super) nodes: Vec<V1Node>,
    /// Any other top-level key: format 1 ignored them, and so does the
    /// conversion, with a note.
    #[serde(flatten)]
    pub(super) other: toml::Table,
}

/// A rectangle in design units: left, top, width, height.
pub(super) type Rect = (i64, i64, i64, i64);

/// The design size a format-1 kind got when its width or height was omitted.
fn default_size(kind: &str) -> (i64, i64) {
    match kind {
        "Label" | "Hyperlink" => (120, 20),
        "Button" | "ToggleButton" => (100, 28),
        "CheckBox" => (120, 24),
        "RadioGroup" => (160, 96),
        "Edit" | "Slider" | "ComboBox" => (160, 24),
        "MultilineEdit" => (200, 100),
        "NumberField" => (120, 28),
        "ProgressBar" => (160, 12),
        "ListView" => (240, 140),
        "Separator" => (120, 8),
        _ => (200, 120),
    }
}

/// A format-1 node's rectangle, with its kind's default size filled in.
pub(super) fn rect(node: &V1Node) -> Rect {
    let (width, height) = default_size(&node.kind);
    let int = |key: &str, default: i64| {
        node.props
            .get(key)
            .and_then(toml::Value::as_integer)
            .unwrap_or(default)
    };
    (
        int("left", 0),
        int("top", 0),
        int("width", width),
        int("height", height),
    )
}

/// Whether a node of `kind` can hold other nodes: only `Panel` and
/// `GroupBox` (or `Group`, as format 2 spells it) could.
pub(super) fn is_container(kind: &str) -> bool {
    matches!(kind, "Panel" | "GroupBox" | "Group")
}

/// `snake_case` as `PascalCase`: `bottom_right` becomes `BottomRight`.
pub(super) fn pascal(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect()
}
