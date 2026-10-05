#![forbid(unsafe_code)]

//! The form document: a window, a flat list of nodes, and the TOML codec.
//!
//! A `.lfm`-style file is `format = 1`, a `[window]` table and one `[[node]]`
//! table per widget. Children name their container through `parent = "…"`, so
//! the list stays flat and diffs cleanly. Only non-default properties are
//! written, in a fixed key order, so a load/save cycle is byte-identical for a
//! canonical file; a value written explicitly equal to its default is dropped on
//! save.
//!
//! The document is plain data. The designer builds on [`FormDoc::node`],
//! [`FormDoc::insert`], [`FormDoc::remove`], [`FormDoc::rename`] and
//! [`FormDoc::reparent`]; the runtime feeds it to [`crate::build`].

use std::collections::BTreeMap;

use crate::FORMAT_VERSION;
use crate::schema::{Access, Catalog};

mod codec;

use crate::value::Value;
pub use codec::LoadError;

/// A form: the format version, the window and the nodes in creation order.
#[derive(Clone, Debug, PartialEq)]
pub struct FormDoc {
    /// The document format; loading a newer one is an error.
    pub format: u32,
    /// The window (`name` plus the window properties).
    pub window: WindowNode,
    /// The nodes, flat and in creation (z) order.
    pub nodes: Vec<Node>,
}

/// The window's name and properties; the window is not a widget node.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowNode {
    /// The form/window name.
    pub name: String,
    /// The window properties, only non-defaults after a canonical load.
    pub props: BTreeMap<String, Value>,
}

impl WindowNode {
    /// A window named `name` with no properties set.
    pub fn new(name: impl Into<String>) -> Self {
        WindowNode {
            name: name.into(),
            props: BTreeMap::new(),
        }
    }

    /// The property named `name`, if set.
    pub fn prop(&self, name: &str) -> Option<&Value> {
        self.props.get(name)
    }

    /// Sets a property, returning the previous value.
    pub fn set_prop(&mut self, name: impl Into<String>, value: Value) -> Option<Value> {
        self.props.insert(name.into(), value)
    }
}

/// One widget node in a form.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// The widget kind (a catalog kind or an alias).
    pub kind: String,
    /// The node name; unique within the form.
    pub name: String,
    /// The parent container's name, or `None` for the window.
    pub parent: Option<String>,
    /// The common and widget properties.
    pub props: BTreeMap<String, Value>,
}

impl Node {
    /// A node of `kind` named `name`, parented to the window.
    pub fn new(kind: impl Into<String>, name: impl Into<String>) -> Self {
        Node {
            kind: kind.into(),
            name: name.into(),
            parent: None,
            props: BTreeMap::new(),
        }
    }

    /// The property named `name`, if set.
    pub fn prop(&self, name: &str) -> Option<&Value> {
        self.props.get(name)
    }

    /// Sets a property, returning the previous value.
    pub fn set_prop(&mut self, name: impl Into<String>, value: Value) -> Option<Value> {
        self.props.insert(name.into(), value)
    }
}

impl FormDoc {
    /// An empty form named `name` with a 320x200 window.
    pub fn new(name: impl Into<String>) -> Self {
        let mut window = WindowNode::new(name);
        window.set_prop("width", Value::Int(320));
        window.set_prop("height", Value::Int(200));
        FormDoc {
            format: FORMAT_VERSION,
            window,
            nodes: Vec::new(),
        }
    }

    /// The node named `name`, if any.
    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.name == name)
    }

    /// The mutable node named `name`, if any.
    pub fn node_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.name == name)
    }

    /// The nodes whose parent is `parent`.
    pub fn children_of(&self, parent: &str) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|node| node.parent.as_deref() == Some(parent))
            .collect()
    }

    /// The nodes parented to the window.
    pub fn roots(&self) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|node| node.parent.is_none())
            .collect()
    }

    /// Appends `node` to the form.
    ///
    /// The caller owns uniqueness and parenting; [`FormDoc::validate`] reports a
    /// problem if they are wrong.
    pub fn insert(&mut self, node: Node) {
        self.nodes.push(node);
    }

    /// Removes the node named `name` and every descendant, returning whether a
    /// node was removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let mut doomed = vec![name.to_owned()];
        let mut index = 0;
        while index < doomed.len() {
            let parent = doomed[index].clone();
            index += 1;
            for node in &self.nodes {
                if node.parent.as_deref() == Some(parent.as_str()) && !doomed.contains(&node.name) {
                    doomed.push(node.name.clone());
                }
            }
        }
        let before = self.nodes.len();
        self.nodes
            .retain(|node| !doomed.iter().any(|doomed| doomed == &node.name));
        self.nodes.len() != before
    }

    /// Renames the node named `old` to `new`, updating its children's `parent`
    /// references, and returns whether a node was renamed.
    pub fn rename(&mut self, old: &str, new: &str) -> bool {
        if self.node(new).is_some() {
            return false;
        }
        let mut renamed = false;
        for node in &mut self.nodes {
            if node.name == old {
                node.name = new.to_owned();
                renamed = true;
            }
        }
        if renamed {
            for node in &mut self.nodes {
                if node.parent.as_deref() == Some(old) {
                    node.parent = Some(new.to_owned());
                }
            }
        }
        renamed
    }

    /// Reparents the node named `name` to `parent` (`None` is the window),
    /// returning whether the node existed.
    ///
    /// Cycles are not prevented here; [`FormDoc::validate`] reports them.
    pub fn reparent(&mut self, name: &str, parent: Option<String>) -> bool {
        match self.node_mut(name) {
            Some(node) => {
                node.parent = parent;
                true
            }
            None => false,
        }
    }
}

/// The access rule for `property` on `kind`, if known (used by tests and the
/// designer).
pub fn property_access(catalog: &Catalog, kind: &str, property: &str) -> Option<Access> {
    if kind == "Window" {
        return catalog
            .window_spec()
            .property(property)
            .map(|spec| spec.access);
    }
    catalog.property(kind, property).map(|spec| spec.access)
}

#[cfg(test)]
mod tests;
