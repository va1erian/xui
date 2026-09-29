#![forbid(unsafe_code)]

//! The [`Menu`](super::Menu) entry tree: its kinds, labels, mnemonics and
//! lookups. Layout lives in [`super::layout`].

use super::{MenuId, SEPARATOR};
use crate::icon::IconRef;

/// What a menu entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// A parent that opens its children as a submenu.
    Submenu,
    /// A command that raises its id.
    Command,
    /// A checkable command.
    Check,
    /// A radio command; picking one clears its radio siblings.
    Radio,
    /// A divider between groups.
    Separator,
}

/// One node of the entry tree.
pub(super) struct Node {
    /// The entry's opaque id ([`SEPARATOR`] for a divider).
    pub(super) id: MenuId,
    /// The label with `&`-markers removed.
    pub(super) text: String,
    /// The character index of the mnemonic in `text`, if any.
    pub(super) mnemonic: Option<usize>,
    /// The lowercased mnemonic key, if any.
    pub(super) key: Option<char>,
    /// What the entry is.
    pub(super) kind: Kind,
    /// Whether the entry accepts input and paints at full strength.
    pub(super) enabled: bool,
    /// Whether a check or radio entry is checked.
    pub(super) checked: bool,
    /// The entry's leading icon, if any.
    pub(super) icon: Option<IconRef>,
    /// The children a [`Submenu`](Kind::Submenu) opens.
    pub(super) children: Vec<Node>,
}

/// The plain facts an event mapper needs about one entry.
#[derive(Clone, Copy)]
pub(super) struct Info {
    /// The entry's id.
    pub(super) id: MenuId,
    /// What the entry is.
    pub(super) kind: Kind,
    /// Whether the entry accepts input.
    pub(super) enabled: bool,
    /// Whether a check or radio entry is checked.
    pub(super) checked: bool,
}

impl Node {
    fn new(id: MenuId, text: &str, kind: Kind, checked: bool) -> Node {
        let (text, mnemonic, key) = label(text);
        Node {
            id,
            text,
            mnemonic,
            key,
            kind,
            enabled: true,
            checked,
            icon: None,
            children: Vec::new(),
        }
    }

    /// A command entry.
    pub(super) fn command(id: MenuId, text: &str) -> Node {
        Node::new(id, text, Kind::Command, false)
    }

    /// A checkable entry.
    pub(super) fn check(id: MenuId, text: &str, checked: bool) -> Node {
        Node::new(id, text, Kind::Check, checked)
    }

    /// A radio entry.
    pub(super) fn radio(id: MenuId, text: &str, checked: bool) -> Node {
        Node::new(id, text, Kind::Radio, checked)
    }

    /// A submenu parent.
    pub(super) fn submenu(id: MenuId, text: &str) -> Node {
        Node::new(id, text, Kind::Submenu, false)
    }

    /// A divider.
    pub(super) fn separator() -> Node {
        Node {
            id: SEPARATOR,
            text: String::new(),
            mnemonic: None,
            key: None,
            kind: Kind::Separator,
            enabled: true,
            checked: false,
            icon: None,
            children: Vec::new(),
        }
    }

    /// The plain facts of this entry.
    pub(super) fn info(&self) -> Info {
        Info {
            id: self.id,
            kind: self.kind,
            enabled: self.enabled,
            checked: self.checked,
        }
    }
}

/// Splits `raw` into its display text and its `&`-marked mnemonic.
///
/// A single `&` marks the following character; `&&` is a literal ampersand,
/// matching the Win32 menu convention.
fn label(raw: &str) -> (String, Option<usize>, Option<char>) {
    let mut text = String::new();
    let mut mnemonic = None;
    let mut key = None;
    let mut chars = raw.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '&' {
            text.push(character);
            continue;
        }
        if chars.peek() == Some(&'&') {
            // `&&` is a literal ampersand.
            text.push('&');
            chars.next();
            continue;
        }
        if let Some(marked) = chars.next() {
            key = Some(marked.to_ascii_lowercase());
            mnemonic = Some(text.chars().count());
            text.push(marked);
        } else {
            text.push('&');
        }
    }
    (text, mnemonic, key)
}
/// Calls `visit` with the entry list at `path` (the root when `path` is empty).
pub(super) fn with_entries<T>(
    nodes: &[Node],
    path: &[usize],
    visit: impl FnOnce(&[Node]) -> T,
) -> Option<T> {
    match path.split_first() {
        None => Some(visit(nodes)),
        Some((index, rest)) => with_entries(nodes.get(*index)?.children.as_slice(), rest, visit),
    }
}

/// Calls `visit` with the mutable entry list at `path`.
pub(super) fn with_entries_mut<T>(
    nodes: &mut [Node],
    path: &[usize],
    visit: impl FnOnce(&mut [Node]) -> T,
) -> Option<T> {
    match path.split_first() {
        None => Some(visit(nodes)),
        Some((index, rest)) => {
            with_entries_mut(nodes.get_mut(*index)?.children.as_mut_slice(), rest, visit)
        }
    }
}

/// Finds entry `id` anywhere in the tree.
pub(super) fn find(nodes: &[Node], id: MenuId) -> Option<&Node> {
    for node in nodes {
        if node.id == id {
            return Some(node);
        }
        if let Some(found) = find(&node.children, id) {
            return Some(found);
        }
    }
    None
}

/// Enables or disables entry `id`, returning whether it was found.
pub(super) fn set_enabled(nodes: &mut [Node], id: MenuId, enabled: bool) -> bool {
    for node in nodes.iter_mut() {
        if node.id == id {
            node.enabled = enabled;
            return true;
        }
        if set_enabled(&mut node.children, id, enabled) {
            return true;
        }
    }
    false
}

/// Checks (or unchecks) entry `id`, clearing its radio siblings when checking,
/// and returns whether any checked flag actually changed.
pub(super) fn set_checked(nodes: &mut [Node], id: MenuId, checked: bool) -> bool {
    if let Some(index) = nodes.iter().position(|node| node.id == id) {
        let mut changed = nodes[index].checked != checked;
        if checked && nodes[index].kind == Kind::Radio {
            for (sibling, node) in nodes.iter_mut().enumerate() {
                if sibling != index && node.kind == Kind::Radio && node.checked {
                    node.checked = false;
                    changed = true;
                }
            }
        }
        nodes[index].checked = checked;
        return changed;
    }
    for node in nodes.iter_mut() {
        if set_checked(&mut node.children, id, checked) {
            return true;
        }
    }
    false
}

/// The index of the first enabled, non-separator entry.
pub(super) fn first_selectable(nodes: &[Node]) -> Option<usize> {
    nodes
        .iter()
        .position(|node| node.enabled && node.kind != Kind::Separator)
}

/// The selectable entry `delta` steps from `current`, clamped at the ends.
pub(super) fn step(nodes: &[Node], current: Option<usize>, delta: i32) -> Option<usize> {
    let len = nodes.len();
    if len == 0 {
        return None;
    }
    let mut index = match current {
        Some(index) => index as i32,
        None => {
            if delta >= 0 {
                -1
            } else {
                len as i32
            }
        }
    };
    loop {
        index += delta;
        if index < 0 || index as usize >= len {
            return current;
        }
        let node = &nodes[index as usize];
        if node.enabled && node.kind != Kind::Separator {
            return Some(index as usize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Vec<Node> {
        vec![
            Node::command(MenuId::new(1), "&Open"),
            Node::separator(),
            Node::submenu(MenuId::new(2), "&Recent"),
            Node::check(MenuId::new(3), "Auto", false),
        ]
    }

    #[test]
    fn labels_extract_the_mnemonic() {
        let node = Node::command(MenuId::new(1), "&Open");
        assert_eq!(node.text, "Open");
        assert_eq!(node.mnemonic, Some(0));
        assert_eq!(node.key, Some('o'));

        let node = Node::command(MenuId::new(2), "Save && Exit");
        assert_eq!(node.text, "Save & Exit");
        assert_eq!(node.mnemonic, None, "`&&` is a literal ampersand");
    }

    #[test]
    fn navigation_skips_separators_and_disabled_rows() {
        let mut nodes = tree();
        nodes[3].enabled = false;
        assert_eq!(first_selectable(&nodes), Some(0));
        assert_eq!(
            step(&nodes, Some(0), 1),
            Some(2),
            "the separator is skipped"
        );
        assert_eq!(
            step(&nodes, Some(2), 1),
            Some(2),
            "a disabled row keeps the highlight in place"
        );
        assert_eq!(step(&nodes, None, 1), Some(0));
    }

    #[test]
    fn checking_a_radio_clears_its_siblings() {
        let mut nodes = vec![
            Node::radio(MenuId::new(1), "Left", true),
            Node::radio(MenuId::new(2), "Right", false),
        ];
        assert!(set_checked(&mut nodes, MenuId::new(2), true));
        assert!(!nodes[0].checked);
        assert!(nodes[1].checked);
    }

    #[test]
    fn set_checked_reports_only_a_real_change() {
        let mut nodes = tree();
        assert!(
            !set_checked(&mut nodes, MenuId::new(3), false),
            "setting the existing value is not a change"
        );
        assert!(set_checked(&mut nodes, MenuId::new(3), true));
        assert!(
            !set_checked(&mut nodes, MenuId::new(3), true),
            "a redundant set is not a change"
        );
        assert!(
            !set_checked(&mut nodes, MenuId::new(9), true),
            "an unknown id is not a change"
        );
    }

    #[test]
    fn set_checked_reports_a_radio_group_change() {
        let mut nodes = vec![
            Node::radio(MenuId::new(1), "Left", false),
            Node::radio(MenuId::new(2), "Right", false),
        ];
        assert!(set_checked(&mut nodes, MenuId::new(1), true));
        assert!(
            !set_checked(&mut nodes, MenuId::new(1), true),
            "re-checking the same radio is not a change"
        );
        assert!(
            set_checked(&mut nodes, MenuId::new(2), true),
            "moving the radio clears the old selection"
        );
        assert!(!nodes[0].checked);
        assert!(nodes[1].checked);
    }
}
