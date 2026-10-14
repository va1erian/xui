#![forbid(unsafe_code)]

//! The conversion proper: a checked format-1 node list, walked from the
//! window down, each container's children becoming one layout.

use std::collections::BTreeMap;

use super::MigrateError;
use super::grid;
use super::v1::{Rect, V1Node, pascal, rect};
use crate::model::{Absolute, Length, MAX_DEPTH, Node};

/// The state of one conversion.
pub(super) struct Converter<'a> {
    /// Each parent's children, in file order; the window's under `None`.
    children: BTreeMap<Option<&'a str>, Vec<&'a V1Node>>,
    /// What was dropped or changed on the way, one line each.
    pub(super) notes: Vec<String>,
}

impl<'a> Converter<'a> {
    /// A conversion of `nodes`, whose names and parents are already checked.
    pub(super) fn new(nodes: &'a [V1Node]) -> Converter<'a> {
        let mut children: BTreeMap<Option<&'a str>, Vec<&'a V1Node>> = BTreeMap::new();
        for node in nodes {
            children
                .entry(node.parent.as_deref())
                .or_default()
                .push(node);
        }
        Converter {
            children,
            notes: Vec::new(),
        }
    }

    /// The children of `parent` (the window for `None`), designed at `size`:
    /// a grid when they line up, an absolute layout otherwise. The layout is
    /// `depth` levels down the tree.
    pub(super) fn level(
        &mut self,
        parent: Option<&str>,
        size: (i64, i64),
        depth: usize,
    ) -> Result<Node, MigrateError> {
        if depth > MAX_DEPTH {
            return Err(too_deep(parent.unwrap_or_default()));
        }
        let children: Vec<&V1Node> = self.children.get(&parent).cloned().unwrap_or_default();
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
                nodes.push((cell.order, self.node(child, fields, depth + 1)?));
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
            nodes.push(self.node(child, fields, depth + 1)?);
        }
        Ok(Node::Absolute(Absolute {
            size: Some((Length(size.0 as f32), Length(size.1 as f32))),
            children: nodes,
            ..Absolute::default()
        }))
    }

    /// One node with its placement `fields`, its content converted. The node
    /// is `depth` levels down the tree.
    fn node(
        &mut self,
        v1: &V1Node,
        mut fields: toml::Table,
        depth: usize,
    ) -> Result<Node, MigrateError> {
        if depth > MAX_DEPTH {
            return Err(too_deep(&v1.name));
        }
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
            toml::Value::Table(wrapped)
                .try_into()
                .map_err(|error: toml::de::Error| MigrateError::Node {
                    node: v1.name.clone(),
                    message: error.to_string(),
                })?;
        let (.., width, height) = rect(v1);
        match &mut node {
            Node::Group(group) => {
                *group.content = self.level(Some(&v1.name), (width, height), depth + 1)?;
            }
            Node::Panel(panel) => {
                *panel.content = self.level(Some(&v1.name), (width, height), depth + 1)?;
            }
            _ => {}
        }
        Ok(node)
    }
}

/// The error for a node nested more deeply than format 2 can hold: a form
/// nests at most [`MAX_DEPTH`] levels, and a container takes two of them (the
/// container and the layout of its content).
fn too_deep(node: &str) -> MigrateError {
    MigrateError::Node {
        node: node.to_owned(),
        message: format!(
            "is nested too deeply for format 2: a form nests at most {MAX_DEPTH} levels, \
             and each container takes two"
        ),
    }
}
