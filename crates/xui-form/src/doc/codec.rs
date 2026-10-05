#![forbid(unsafe_code)]

//! The format-1 TOML codec: [`FormDoc::from_toml`] and [`FormDoc::to_toml`].

use std::collections::BTreeMap;
use std::fmt;

use super::{FormDoc, Node, WindowNode};
use crate::FORMAT_VERSION;
use crate::schema::Catalog;
use crate::value::Value;

impl FormDoc {
    /// Parses a form from TOML, decoding each property against `catalog`.
    pub fn from_toml(text: &str, catalog: &Catalog) -> Result<FormDoc, LoadError> {
        let root: toml::Table = toml::from_str(text).map_err(|error| syntax_error(text, &error))?;

        let format = match root.get("format").and_then(toml::Value::as_integer) {
            Some(format) => {
                u32::try_from(format).map_err(|_| LoadError::plain("`format` must be positive"))?
            }
            None => {
                return Err(LoadError::at(
                    "missing or non-integer `format`",
                    find_key_line(text, Section::Root, "format"),
                ));
            }
        };
        if format != FORMAT_VERSION {
            return Err(LoadError::plain(format!(
                "unsupported form format {format}; this build understands format {FORMAT_VERSION}"
            )));
        }

        let window_table = root
            .get("window")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| LoadError::plain("missing `[window]` table"))?;
        let window_name = window_table
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| LoadError::plain("`[window]` must have a string `name`"))?
            .to_owned();
        let window = WindowNode {
            name: window_name,
            props: decode_window_props(window_table, catalog, text)?,
        };

        let mut nodes = Vec::new();
        if let Some(raw_nodes) = root.get("node") {
            let raw_nodes = raw_nodes
                .as_array()
                .ok_or_else(|| LoadError::plain("`node` must be an array of tables"))?;
            for (index, raw) in raw_nodes.iter().enumerate() {
                let table = raw
                    .as_table()
                    .ok_or_else(|| LoadError::plain("each `node` entry must be a table"))?;
                nodes.push(decode_node(table, catalog, text, index)?);
            }
        }

        Ok(FormDoc {
            format,
            window,
            nodes,
        })
    }

    /// Serialises the form to canonical TOML.
    pub fn to_toml(&self, catalog: &Catalog) -> String {
        let mut out = String::new();
        out.push_str(&format!("format = {}\n\n", self.format));

        out.push_str("[window]\n");
        out.push_str(&format!("name = {}\n", toml_string(&self.window.name)));
        write_props(&mut out, &self.window.props, |name| {
            catalog
                .window_spec()
                .property(name)
                .map(|spec| spec.default.clone())
        });

        for node in &self.nodes {
            out.push_str("\n[[node]]\n");
            out.push_str(&format!("kind = {}\n", toml_string(&node.kind)));
            out.push_str(&format!("name = {}\n", toml_string(&node.name)));
            if let Some(parent) = &node.parent {
                out.push_str(&format!("parent = {}\n", toml_string(parent)));
            }
            write_common_props(&mut out, node, catalog);
            write_widget_props(&mut out, node, catalog);
        }
        out
    }
}

/// A failure to load a form document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadError {
    /// A human-readable description.
    message: String,
    /// The one-based TOML line, when it could be located.
    line: Option<usize>,
}

impl LoadError {
    /// An error with no line.
    fn plain(message: impl Into<String>) -> Self {
        LoadError {
            message: message.into(),
            line: None,
        }
    }

    /// An error at a known line.
    fn at(message: impl Into<String>, line: Option<usize>) -> Self {
        LoadError {
            message: message.into(),
            line,
        }
    }

    /// The description.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The one-based TOML line, when located.
    pub fn line(&self) -> Option<usize> {
        self.line
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(formatter, "line {line}: {}", self.message),
            None => formatter.write_str(&self.message),
        }
    }
}

impl std::error::Error for LoadError {}

/// Decodes the window properties (everything but `name`).
fn decode_window_props(
    table: &toml::Table,
    catalog: &Catalog,
    text: &str,
) -> Result<BTreeMap<String, Value>, LoadError> {
    let mut props = BTreeMap::new();
    for (key, raw) in table {
        if key == "name" {
            continue;
        }
        let spec = catalog
            .window_spec()
            .property(key)
            .ok_or_else(|| unknown_property("Window", key, None, text, Section::Window))?;
        props.insert(
            key.clone(),
            decode(raw, spec.ty.clone(), key, None, text, Section::Window)?,
        );
    }
    Ok(props)
}

/// Decodes one `[[node]]` table.
fn decode_node(
    table: &toml::Table,
    catalog: &Catalog,
    text: &str,
    index: usize,
) -> Result<Node, LoadError> {
    let section = Section::Node(index);
    let kind = table
        .get("kind")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| LoadError::plain("each node must have a string `kind`"))?
        .to_owned();
    let name = table
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| LoadError::plain("each node must have a string `name`"))?
        .to_owned();
    let parent = table
        .get("parent")
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| LoadError::plain("`parent` must be a string"))
        })
        .transpose()?;

    let _spec = catalog.get(&kind).ok_or_else(|| {
        LoadError::at(
            format!("`{kind}` is not a known widget kind"),
            find_key_line(text, section, "kind"),
        )
    })?;

    let mut props = BTreeMap::new();
    for (key, raw) in table {
        if matches!(key.as_str(), "kind" | "name" | "parent") {
            continue;
        }
        let spec = catalog
            .property(&kind, key)
            .ok_or_else(|| unknown_property(&kind, key, Some(&name), text, section))?;
        props.insert(
            key.clone(),
            decode(raw, spec.ty.clone(), key, Some(&name), text, section)?,
        );
    }

    Ok(Node {
        kind,
        name,
        parent,
        props,
    })
}

/// Decodes one raw literal against a type, mapping the error to a located
/// [`LoadError`].
fn decode(
    raw: &toml::Value,
    ty: crate::value::ValueType,
    key: &str,
    node: Option<&str>,
    text: &str,
    section: Section,
) -> Result<Value, LoadError> {
    Value::from_toml(raw, &ty).map_err(|source| {
        let subject = match node {
            Some(node) => format!("`{key}` on `{node}`"),
            None => format!("`{key}`"),
        };
        LoadError::at(
            format!("invalid {subject}: {source}"),
            find_key_line(text, section, key),
        )
    })
}

/// Builds an "unknown property" error.
fn unknown_property(
    kind: &str,
    key: &str,
    node: Option<&str>,
    text: &str,
    section: Section,
) -> LoadError {
    let where_ = match node {
        Some(node) => format!("`{key}` is not a property of {kind} (node `{node}`)"),
        None => format!("`{key}` is not a property of {kind}"),
    };
    LoadError::at(where_, find_key_line(text, section, key))
}

/// Turns a `toml` parse failure into a located [`LoadError`].
fn syntax_error(text: &str, error: &toml::de::Error) -> LoadError {
    let line = error
        .span()
        .map(|span| line_for_offset(text, span.start))
        .unwrap_or(1);
    LoadError::at(error.message().to_owned(), Some(line))
}

/// The one-based line an absolute byte offset falls on.
fn line_for_offset(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text.as_bytes()[..offset]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

/// Finds the one-based line of a top-level `key = value` entry, if present.
/// The part of a form file a key is looked up in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    /// The keys before any table header (`format`).
    Root,
    /// The `[window]` table.
    Window,
    /// The `index`-th `[[node]]` table.
    Node(usize),
}

/// The one-based line of `key` inside `section`, so an error in the third
/// node points at that node rather than at the first `key` in the file.
fn find_key_line(text: &str, section: Section, key: &str) -> Option<usize> {
    let mut current = Some(Section::Root);
    let mut nodes_seen = 0;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            current = match trimmed {
                "[[node]]" => {
                    nodes_seen += 1;
                    Some(Section::Node(nodes_seen - 1))
                }
                "[window]" => Some(Section::Window),
                _ => None,
            };
            continue;
        }
        if current != Some(section) {
            continue;
        }
        let found = trimmed.split('=').next().map(str::trim);
        if found == Some(key) {
            return Some(index + 1);
        }
    }
    None
}

/// Writes the window's non-default properties, in map order.
fn write_props(
    out: &mut String,
    props: &BTreeMap<String, Value>,
    default: impl Fn(&str) -> Option<Value>,
) {
    for (name, value) in props {
        if default(name).as_ref() == Some(value) {
            continue;
        }
        out.push_str(&format!("{name} = {}\n", value.to_toml()));
    }
}

/// Writes a node's non-default common properties, in the documented order.
fn write_common_props(out: &mut String, node: &Node, catalog: &Catalog) {
    for spec in catalog.common_properties() {
        let Some(value) = node.props.get(&spec.name) else {
            continue;
        };
        let default = catalog
            .property(&node.kind, &spec.name)
            .map(|property| property.default);
        if default.as_ref() == Some(value) {
            continue;
        }
        out.push_str(&format!("{} = {}\n", spec.name, value.to_toml()));
    }
}

/// Writes a node's remaining non-default properties, alphabetically.
fn write_widget_props(out: &mut String, node: &Node, catalog: &Catalog) {
    let common: Vec<&str> = catalog
        .common_properties()
        .iter()
        .map(|spec| spec.name.as_str())
        .collect();
    for (name, value) in &node.props {
        if common.contains(&name.as_str()) {
            continue;
        }
        let default = catalog
            .property(&node.kind, name)
            .map(|property| property.default);
        if default.as_ref() == Some(value) {
            continue;
        }
        out.push_str(&format!("{name} = {}\n", value.to_toml()));
    }
}

/// A TOML basic-string literal for `value`.
fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}
