#![forbid(unsafe_code)]

//! The form model: plain serde types that are at once the `.lfm` format 2
//! (RON) and, through the macros that declare them, the [`Catalog`]'s
//! schema.
//!
//! A [`Form`] holds one root [`Node`]. A node is a layout (`Row`, `Column`,
//! `Wrap`, `Grid`, `Absolute`), a container widget (`Panel`, `Group`, `Tabs`)
//! or a widget (`Button`, `Edit`, …), each an enum variant written
//! `Kind(field: value, …)`:
//!
//! ```ron
//! Form(
//!     title: "Hello",
//!     size: (360, 160),
//!     root: Column(padding: 16, gap: 8, children: [
//!         Row(gap: 8, children: [
//!             Edit(name: "name_edit", placeholder: "Your name", fill: 1),
//!             Button(name: "greet_button", text: "Greet"),
//!         ]),
//!         Label(name: "result_label"),
//!     ]),
//! )
//! ```
//!
//! Every field has a default and a default is never written, so a file says
//! only what differs.
//!
//! [`Catalog`]: crate::Catalog

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::value::Value;

mod containers;
pub(crate) mod macros;
mod prop;
mod types;
mod widgets;

pub use containers::{Absolute, Grid, Group, Page, Panel, Stack, Tabs};
pub use prop::PropType;
pub use types::{Align, Anchor, Length, Orientation, Track};
pub use widgets::{
    Button, CheckBox, ComboBox, Edit, Hyperlink, Label, ListView, MultilineEdit, NumberField,
    ProgressBar, RadioGroup, Separator, Slider, ToggleButton,
};

/// The `.lfm` format this crate reads and writes: 2, a RON tree. Format 1 (flat
/// TOML) is converted by `xui-form migrate`.
pub const FORMAT_VERSION: u32 = 2;

/// A form: the window it opens and its root node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Form {
    /// The form's name, which scripts and multi-form hosts address it by.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The window title.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// The window's initial client size, in design units.
    pub size: (Length, Length),
    /// Whether the window can be resized.
    #[serde(skip_serializing_if = "is_true")]
    pub resizable: bool,
    /// The window's content.
    pub root: Node,
}

impl Default for Form {
    fn default() -> Form {
        Form {
            name: String::new(),
            title: String::new(),
            size: (Length(320.0), Length(200.0)),
            resizable: true,
            root: Node::default(),
        }
    }
}

/// Whether `value` is `true`, for `skip_serializing_if`.
fn is_true(value: &bool) -> bool {
    *value
}

/// One node of a form's tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Node {
    /// Entries left to right.
    Row(Stack),
    /// Entries top to bottom.
    Column(Stack),
    /// Entries left to right at their natural sizes, wrapping into lines.
    Wrap(Stack),
    /// Entries flowing into columns.
    Grid(Grid),
    /// Entries at fixed rectangles, following anchors.
    Absolute(Absolute),
    /// A container widget.
    Panel(Panel),
    /// A titled frame.
    Group(Group),
    /// A paged container.
    Tabs(Tabs),
    /// A static text label.
    Label(Label),
    /// A push button.
    Button(Button),
    /// A clickable link label.
    Hyperlink(Hyperlink),
    /// A check box.
    CheckBox(CheckBox),
    /// A button that latches.
    ToggleButton(ToggleButton),
    /// A single-line text field.
    Edit(Edit),
    /// A multi-line text area.
    MultilineEdit(MultilineEdit),
    /// A numeric field.
    NumberField(NumberField),
    /// A range control.
    Slider(Slider),
    /// A progress indicator.
    ProgressBar(ProgressBar),
    /// Radio options.
    RadioGroup(RadioGroup),
    /// A drop-down list.
    ComboBox(ComboBox),
    /// A list of rows.
    ListView(ListView),
    /// A divider line.
    Separator(Separator),
}

impl Default for Node {
    fn default() -> Node {
        Node::Column(Stack::default())
    }
}

/// Expands `$body` with `$widget` bound to the widget inside any widget
/// variant of `$node`, or evaluates `$other` for a layout variant.
macro_rules! each_widget {
    ($node:expr, $widget:ident => $body:expr, _ => $other:expr) => {
        match $node {
            Node::Panel($widget) => $body,
            Node::Group($widget) => $body,
            Node::Tabs($widget) => $body,
            Node::Label($widget) => $body,
            Node::Button($widget) => $body,
            Node::Hyperlink($widget) => $body,
            Node::CheckBox($widget) => $body,
            Node::ToggleButton($widget) => $body,
            Node::Edit($widget) => $body,
            Node::MultilineEdit($widget) => $body,
            Node::NumberField($widget) => $body,
            Node::Slider($widget) => $body,
            Node::ProgressBar($widget) => $body,
            Node::RadioGroup($widget) => $body,
            Node::ComboBox($widget) => $body,
            Node::ListView($widget) => $body,
            Node::Separator($widget) => $body,
            _ => $other,
        }
    };
}

impl Node {
    /// The variant's name, as written in a file.
    pub fn kind(&self) -> &'static str {
        match self {
            Node::Row(_) => "Row",
            Node::Column(_) => "Column",
            Node::Wrap(_) => "Wrap",
            Node::Grid(_) => "Grid",
            Node::Absolute(_) => "Absolute",
            other => each_widget!(other, widget => Widget::kind(widget), _ => unreachable!()),
        }
    }

    /// The widget inside a widget node, or `None` for a layout.
    pub fn widget(&self) -> Option<&dyn Widget> {
        each_widget!(self, widget => Some(widget as &dyn Widget), _ => None)
    }

    /// The node's layout fields.
    pub fn place(&self) -> Place {
        match self {
            Node::Row(stack) | Node::Column(stack) | Node::Wrap(stack) => stack.place(),
            Node::Grid(grid) => grid.place(),
            Node::Absolute(absolute) => absolute.place(),
            other => each_widget!(other, widget => Widget::place(widget), _ => unreachable!()),
        }
    }

    /// The nodes directly inside this one: a layout's children, a panel's or
    /// a group's content, or each page's content.
    pub fn children(&self) -> Vec<&Node> {
        match self {
            Node::Row(stack) | Node::Column(stack) | Node::Wrap(stack) => {
                stack.children.iter().collect()
            }
            Node::Grid(grid) => grid.children.iter().collect(),
            Node::Absolute(absolute) => absolute.children.iter().collect(),
            Node::Panel(panel) => vec![&*panel.content],
            Node::Group(group) => vec![&*group.content],
            Node::Tabs(tabs) => tabs.pages.iter().map(|page| &page.content).collect(),
            _ => Vec::new(),
        }
    }

    /// Calls `visit` on this node and every node below it, parents first,
    /// with the layout each one is a child of (`None` for the root).
    pub fn walk<'a>(&'a self, visit: &mut dyn FnMut(&'a Node, Option<&'a Node>)) {
        fn go<'a>(
            node: &'a Node,
            parent: Option<&'a Node>,
            visit: &mut dyn FnMut(&'a Node, Option<&'a Node>),
        ) {
            visit(node, parent);
            for child in node.children() {
                go(child, Some(node), visit);
            }
        }
        go(self, None, visit);
    }
}

/// The layout fields every node has: how it is sized and placed by the
/// layout it is in.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Place {
    /// A share of the parent's leftover space, by weight.
    pub fill: Option<u32>,
    /// An exact width.
    pub width: Option<Length>,
    /// An exact height.
    pub height: Option<Length>,
    /// A maximum width.
    pub max_width: Option<Length>,
    /// A maximum height.
    pub max_height: Option<Length>,
    /// The entry's own alignment.
    pub align: Option<Align>,
    /// In a grid, the columns covered.
    pub span: Option<u32>,
    /// In an absolute layout, the design rectangle.
    pub at: Option<(Length, Length, Length, Length)>,
    /// In an absolute layout, the anchor.
    pub anchor: Option<Anchor>,
}

/// What every widget node offers, whatever its kind.
pub trait Widget {
    /// The kind, as written in a file (`Button`).
    fn kind(&self) -> &'static str;
    /// The widget's name.
    fn name(&self) -> &str;
    /// For a control array written once, its element count.
    fn array(&self) -> Option<u32>;
    /// For one element of a control array, its index.
    fn index(&self) -> Option<u32>;
    /// Every property, by name, including the defaults, `visible` and
    /// `enabled`.
    fn props(&self) -> BTreeMap<String, Value>;
    /// The widget's layout fields.
    fn place(&self) -> Place;
}

/// A description from doc-comment lines: trimmed and joined with spaces.
pub(crate) fn doc(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join(" ")
}
