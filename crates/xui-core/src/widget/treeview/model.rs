#![forbid(unsafe_code)]

//! The tree view's data: flat [`TreeRow`]s, the virtual, lazily-loaded
//! [`TreeModel`], the [`TreeNode`] it returns and a row's [`CheckState`].

/// A node's stable identity in a [`TreeModel`].
///
/// Ids are opaque to the tree, but they must be unique across the whole model
/// so that an event callback can name the selected, toggled or checked node.
pub type NodeId = usize;

/// The state of a row's checkbox.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CheckState {
    /// An empty box.
    #[default]
    Unchecked,
    /// A checked box.
    Checked,
    /// A mixed box, used by the tri-state mode for a partly checked branch.
    Indeterminate,
}

impl CheckState {
    /// The state a click moves to. `tri_state` inserts [`Indeterminate`] between
    /// checked and unchecked; otherwise the box toggles between the two.
    ///
    /// [`Indeterminate`]: CheckState::Indeterminate
    pub fn cycled(self, tri_state: bool) -> CheckState {
        match (self, tri_state) {
            (CheckState::Unchecked, _) => CheckState::Checked,
            (CheckState::Checked, true) => CheckState::Indeterminate,
            (CheckState::Checked, false) | (CheckState::Indeterminate, _) => CheckState::Unchecked,
        }
    }
}

/// One flattened tree row, in display order (the simple, non-virtual model).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeRow {
    /// The row's label.
    pub label: String,
    /// The row's indent depth (`0` for a root).
    pub depth: u16,
    /// Whether the row has children and can be collapsed.
    pub expandable: bool,
    /// Whether the row is expanded.
    pub expanded: bool,
    /// The initial state of the row's checkbox.
    pub checked: CheckState,
}

impl TreeRow {
    /// A collapsed, non-expandable row of `depth`.
    pub fn new(label: impl Into<String>, depth: u16) -> TreeRow {
        TreeRow {
            label: label.into(),
            depth,
            ..Default::default()
        }
    }

    /// Sets whether the row can be collapsed.
    pub fn expandable(mut self, expandable: bool) -> TreeRow {
        self.expandable = expandable;
        self
    }

    /// Sets whether the row starts expanded.
    pub fn expanded(mut self, expanded: bool) -> TreeRow {
        self.expanded = expanded;
        self
    }

    /// Sets the row's initial checkbox state.
    pub fn checked(mut self, checked: CheckState) -> TreeRow {
        self.checked = checked;
        self
    }
}

/// One node handed back by a [`TreeModel`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeNode {
    /// Stable identity of the node; unique across the model.
    pub id: NodeId,
    /// The node's label.
    pub label: String,
    /// Whether to show an expand button before the children load.
    pub has_children: bool,
    /// The initial state of the node's checkbox.
    pub checked: CheckState,
}

impl TreeNode {
    /// A leaf: no expand button.
    pub fn leaf(id: NodeId, label: impl Into<String>) -> TreeNode {
        TreeNode {
            id,
            label: label.into(),
            has_children: false,
            checked: CheckState::Unchecked,
        }
    }

    /// A branch that can be expanded to load its children.
    pub fn branch(id: NodeId, label: impl Into<String>) -> TreeNode {
        TreeNode {
            has_children: true,
            ..TreeNode::leaf(id, label)
        }
    }

    /// Sets the node's initial checkbox state.
    pub fn checked(mut self, checked: CheckState) -> TreeNode {
        self.checked = checked;
        self
    }
}

/// Supplies a [`TreeView`](super::TreeView) with nodes, loaded lazily.
///
/// Only the roots are read when the tree is built: [`children`](TreeModel::children)
/// is called for a node's children the first time that node is expanded. An
/// unexpanded branch therefore costs nothing to keep.
pub trait TreeModel {
    /// The children of `parent`, or the roots when `parent` is `None`.
    fn children(&self, parent: Option<NodeId>) -> Vec<TreeNode>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_state_cycles_depend_on_tri_state() {
        assert_eq!(
            CheckState::Unchecked.cycled(false),
            CheckState::Checked,
            "a two-state box checks on click"
        );
        assert_eq!(
            CheckState::Checked.cycled(false),
            CheckState::Unchecked,
            "and unchecks on the next"
        );
        assert_eq!(
            CheckState::Unchecked.cycled(true),
            CheckState::Checked,
            "tri-state starts like two-state"
        );
        assert_eq!(
            CheckState::Checked.cycled(true),
            CheckState::Indeterminate,
            "tri-state visits the mixed state"
        );
        assert_eq!(
            CheckState::Indeterminate.cycled(true),
            CheckState::Unchecked,
            "and returns to empty"
        );
    }

    #[test]
    fn node_builders_mark_leaves_and_branches() {
        let leaf = TreeNode::leaf(1, "a");
        assert!(!leaf.has_children);
        assert_eq!(leaf.checked, CheckState::Unchecked);
        let branch = TreeNode::branch(2, "b").checked(CheckState::Checked);
        assert!(branch.has_children);
        assert_eq!(branch.checked, CheckState::Checked);
    }

    #[test]
    fn row_builders_set_expansion_and_check() {
        let row = TreeRow::new("x", 1)
            .expandable(true)
            .expanded(true)
            .checked(CheckState::Indeterminate);
        assert!(row.expandable && row.expanded);
        assert_eq!(row.checked, CheckState::Indeterminate);
    }
}
