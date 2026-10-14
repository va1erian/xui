#![forbid(unsafe_code)]

//! Checking a format-1 form before it is converted.
//!
//! Format 1 kept its nodes in a flat list that named each one's parent, so
//! nothing but these rules makes the list a tree: names are unique, a parent
//! exists and can hold nodes, and the parents lead to the window and not round
//! in a circle. A form that breaks one cannot be converted without dropping
//! nodes, or without recursing for ever, so each violation is reported with
//! its node and nothing is converted.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use super::v1::{V1Node, is_container};

/// One node that breaks a rule of format 1, or one thing in the window that
/// the conversion cannot read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeProblem {
    /// The node's name (`window` for the `[window]` table).
    pub node: String,
    /// What is wrong with it.
    pub message: String,
}

/// Every violation of the rules in `nodes`, in file order.
pub(super) fn check(nodes: &[V1Node]) -> Vec<NodeProblem> {
    let mut found: Vec<(usize, NodeProblem)> = Vec::new();
    let mut problem = |index: usize, message: String| {
        found.push((
            index,
            NodeProblem {
                node: nodes[index].name.clone(),
                message,
            },
        ));
    };

    // The first node of each name is the one a parent refers to.
    let mut first: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, node) in nodes.iter().enumerate() {
        match first.entry(&node.name) {
            Entry::Vacant(slot) => {
                slot.insert(index);
            }
            Entry::Occupied(_) => problem(index, "the name is used more than once".to_owned()),
        }
    }

    let mut parent_of: Vec<Option<usize>> = vec![None; nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        let Some(parent) = node.parent.as_deref() else {
            continue;
        };
        match first.get(parent) {
            None => problem(index, format!("its parent `{parent}` does not exist")),
            Some(&at) if !is_container(&nodes[at].kind) => problem(
                index,
                format!(
                    "its parent `{parent}` is a {}, which cannot hold nodes (only a `Panel` \
                     or a `GroupBox` can)",
                    nodes[at].kind
                ),
            ),
            Some(&at) => parent_of[index] = Some(at),
        }
    }

    for index in cycle_members(&parent_of) {
        problem(index, "it is part of a parent cycle".to_owned());
    }
    for (index, node) in nodes.iter().enumerate() {
        for key in ["left", "top", "width", "height"] {
            if node.props.get(key).is_some_and(|value| !value.is_integer()) {
                problem(index, format!("`{key}` must be an integer"));
            }
        }
        if node
            .props
            .get("anchor")
            .is_some_and(|value| !value.is_str())
        {
            problem(index, "`anchor` must be a string".to_owned());
        }
    }

    found.sort_by_key(|(index, _)| *index);
    found.into_iter().map(|(_, problem)| problem).collect()
}

/// The nodes that lie on a cycle of `parent_of`, in node order. Each node is
/// visited once, so the cost is linear however long the chains are.
fn cycle_members(parent_of: &[Option<usize>]) -> Vec<usize> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Unseen,
        OnPath,
        Done,
    }
    let mut marks = vec![Mark::Unseen; parent_of.len()];
    let mut members = Vec::new();
    for start in 0..parent_of.len() {
        let mut path = Vec::new();
        let mut current = Some(start);
        while let Some(node) = current {
            match marks[node] {
                Mark::Unseen => {
                    marks[node] = Mark::OnPath;
                    path.push(node);
                    current = parent_of[node];
                }
                Mark::OnPath => {
                    // `node` is on this walk: everything from it on is a cycle.
                    let at = path.iter().position(|&n| n == node).unwrap_or_default();
                    members.extend(&path[at..]);
                    break;
                }
                Mark::Done => break,
            }
        }
        for node in path {
            marks[node] = Mark::Done;
        }
    }
    members.sort_unstable();
    members
}
