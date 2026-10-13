#![forbid(unsafe_code)]

//! Converting a format-1 form (flat TOML, absolute rectangles) to format 2.
//!
//! Each container's children become an `Absolute` layout, every node at its
//! old rectangle with its old anchor, or a `Grid` when the rectangles line up
//! in rows and columns and none is anchored. There is no format-1 loader at
//! runtime: forms are converted once, with `xui-form migrate`.

use serde::Deserialize;

use crate::model::{Absolute, Form, Length, Node};

mod grid;

/// A converted form, with what the conversion could not carry over.
#[derive(Clone, Debug, PartialEq)]
pub struct Migration {
    /// The format-2 form.
    pub form: Form,
    /// Properties dropped or changed on the way, one line each.
    pub notes: Vec<String>,
}

/// A format-1 form that could not be converted.
#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    /// The text is not valid format-1 TOML.
    #[error("not a format-1 form: {0}")]
    Toml(String),
    /// A node could not be expressed in format 2.
    #[error("node `{node}`: {message}")]
    Node {
        /// The node's name.
        node: String,
        /// What went wrong.
        message: String,
    },
}

/// One `[[node]]` of a format-1 form.
#[derive(Clone, Debug, Deserialize)]
struct V1Node {
    kind: String,
    name: String,
    parent: Option<String>,
    #[serde(flatten)]
    props: toml::Table,
}

/// A format-1 form.
#[derive(Debug, Deserialize)]
struct V1Form {
    format: u32,
    window: toml::Table,
    #[serde(default, rename = "node")]
    nodes: Vec<V1Node>,
}

/// A rectangle in design units: left, top, width, height.
type Rect = (i64, i64, i64, i64);

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

/// Converts format-1 `text` to a format-2 form.
pub fn from_v1(text: &str) -> Result<Migration, MigrateError> {
    let v1: V1Form = toml::from_str(text).map_err(|error| MigrateError::Toml(error.to_string()))?;
    if v1.format != 1 {
        return Err(MigrateError::Toml(format!("format {} is not 1", v1.format)));
    }
    let int = |key: &str, default: i64| {
        v1.window
            .get(key)
            .and_then(toml::Value::as_integer)
            .unwrap_or(default)
    };
    let text = |key: &str| {
        v1.window
            .get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let size = (int("width", 320), int("height", 200));
    let mut converter = Converter {
        nodes: &v1.nodes,
        notes: Vec::new(),
    };
    let root = converter.level(None, size)?;
    if let Some(orphan) = v1.nodes.iter().find(|node| {
        node.parent
            .as_ref()
            .is_some_and(|parent| !v1.nodes.iter().any(|other| &other.name == parent))
    }) {
        return Err(MigrateError::Node {
            node: orphan.name.clone(),
            message: "its parent does not exist".to_owned(),
        });
    }
    let form = Form {
        name: text("name"),
        title: text("title"),
        size: (Length(size.0 as f32), Length(size.1 as f32)),
        resizable: v1
            .window
            .get("resizable")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true),
        root,
    };
    Ok(Migration {
        form,
        notes: converter.notes,
    })
}

/// The state of one conversion.
struct Converter<'a> {
    nodes: &'a [V1Node],
    notes: Vec<String>,
}

impl Converter<'_> {
    /// The children of `parent` (the window for `None`), designed at `size`:
    /// a grid when they line up, an absolute layout otherwise.
    fn level(&mut self, parent: Option<&str>, size: (i64, i64)) -> Result<Node, MigrateError> {
        let children: Vec<&V1Node> = self
            .nodes
            .iter()
            .filter(|node| node.parent.as_deref() == parent)
            .collect();
        let rects: Vec<Rect> = children.iter().map(|node| rect(node)).collect();
        let anchored = children.iter().any(|node| {
            node.props
                .get("anchor")
                .and_then(toml::Value::as_str)
                .is_some_and(|anchor| anchor != "top_left")
        });
        if !anchored && let Some(layout) = grid::detect(&rects, size) {
            let mut nodes = Vec::with_capacity(children.len());
            for (cell, &child) in layout.cells.iter().zip(&children) {
                let mut fields = toml::Table::new();
                fields.insert("height".into(), toml::Value::Integer(cell.height));
                if cell.span > 1 {
                    fields.insert("span".into(), toml::Value::Integer(cell.span as i64));
                }
                nodes.push((cell.order, self.node(child, fields)?));
            }
            nodes.sort_by_key(|(order, _)| *order);
            return Ok(Node::Grid(
                layout.into_grid(nodes.into_iter().map(|(_, n)| n).collect()),
            ));
        }
        let mut nodes = Vec::with_capacity(children.len());
        for (&child, (left, top, width, height)) in children.iter().zip(rects) {
            let mut fields = toml::Table::new();
            let at = [left, top, width, height].map(toml::Value::Integer);
            fields.insert("at".into(), toml::Value::Array(at.to_vec()));
            if let Some(anchor) = child.props.get("anchor").and_then(toml::Value::as_str)
                && anchor != "top_left"
            {
                fields.insert("anchor".into(), toml::Value::String(pascal(anchor)));
            }
            nodes.push(self.node(child, fields)?);
        }
        Ok(Node::Absolute(Absolute {
            size: Some((Length(size.0 as f32), Length(size.1 as f32))),
            children: nodes,
            ..Absolute::default()
        }))
    }

    /// One node with its placement `fields`, its content converted.
    fn node(&mut self, v1: &V1Node, mut fields: toml::Table) -> Result<Node, MigrateError> {
        let kind = match v1.kind.as_str() {
            "GroupBox" => "Group",
            other => other,
        };
        fields.insert("name".into(), toml::Value::String(v1.name.clone()));
        for (key, value) in &v1.props {
            match key.as_str() {
                "left" | "top" | "width" | "height" | "anchor" => {}
                "tab_index" => self.notes.push(format!(
                    "`{}`: dropped `tab_index`; the tab order is the tree order",
                    v1.name
                )),
                "cue" => {
                    fields.insert("placeholder".into(), value.clone());
                }
                "orientation" => {
                    let name = value.as_str().map(pascal).unwrap_or_default();
                    fields.insert(key.clone(), toml::Value::String(name));
                }
                _ => {
                    fields.insert(key.clone(), value.clone());
                }
            }
        }
        let mut wrapped = toml::Table::new();
        wrapped.insert(kind.to_owned(), toml::Value::Table(fields));
        let mut node =
            Node::deserialize(toml::Value::Table(wrapped)).map_err(|error| MigrateError::Node {
                node: v1.name.clone(),
                message: error.to_string(),
            })?;
        let (.., width, height) = rect(v1);
        match &mut node {
            Node::Group(group) => {
                *group.content = self.level(Some(&v1.name), (width, height))?;
            }
            Node::Panel(panel) => {
                *panel.content = self.level(Some(&v1.name), (width, height))?;
            }
            _ => {}
        }
        Ok(node)
    }
}

/// A format-1 node's rectangle, with its kind's default size filled in.
fn rect(node: &V1Node) -> Rect {
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

/// `snake_case` as `PascalCase`: `bottom_right` becomes `BottomRight`.
fn pascal(name: &str) -> String {
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

#[cfg(test)]
mod tests;
