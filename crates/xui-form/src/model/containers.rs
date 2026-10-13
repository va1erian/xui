#![forbid(unsafe_code)]

//! The layout nodes (rows, columns, wraps, grids and absolute layouts) and
//! the container widgets (panels, group boxes and tabs).

use super::macros::{node, node_inner, widget};
use super::{Align, Length, Node, Track};
use crate::value::ValueType;

node! {
    /// A row, a column or a wrap of nodes.
    Stack {
        /// The gap between adjacent entries (and between a wrap's lines), in
        /// design units.
        gap: Length = Length(0.0),
        /// Space inside the edges, in design units.
        padding: Length = Length(0.0),
        /// Where entries sit across the main axis unless they set their own
        /// `align`.
        align_items: Option<Align> = None,
        /// Where entries sit along the main axis when none of them fills it.
        justify: Option<Align> = None,
        /// The entries, in order.
        children: Vec<Node> = Vec::new(),
    }
}

node! {
    /// A grid: its entries flow into `columns`, left to right and row by row.
    Grid {
        /// The columns' tracks: `Auto`, `Fixed(width)` or `Fill(weight)`.
        columns: Vec<Track> = Vec::new(),
        /// The gap between rows and between columns, in design units.
        gap: Length = Length(0.0),
        /// Space inside the edges, in design units.
        padding: Length = Length(0.0),
        /// Where entries sit within their cells unless they set their own
        /// `align`.
        align_items: Option<Align> = None,
        /// The entries, in order.
        children: Vec<Node> = Vec::new(),
    }
}

node! {
    /// Free placement: each entry at its `at` rectangle, following the layout
    /// with its `anchor` as the layout grows or shrinks from `size`.
    Absolute {
        /// The size the positions were designed at; the smallest that holds
        /// every entry when omitted.
        size: Option<(Length, Length)> = None,
        /// Space inside the edges, in design units.
        padding: Length = Length(0.0),
        /// The entries, in order.
        children: Vec<Node> = Vec::new(),
    }
}

node_inner! {
    /// One page of a `Tabs`.
    Page {
        /// The tab's title.
        title: String = String::new(),
        /// The page's content.
        content: Node = Node::default(),
    }
}

widget! {
    /// A container: a section of its own with `content` laid out inside it.
    Panel {
        /// Whether it draws a card; a plain panel draws nothing of its own.
        card: bool = false; Appearance (design),
    }
    events {}
    content {
        /// The panel's content.
        content: Box<Node> = Box::default(),
    }
}

widget! {
    /// A titled frame with `content` laid out inside it, below the title.
    Group {
        /// The frame's title.
        text: String = String::new(); Appearance,
    }
    events {}
    content {
        /// The frame's content.
        content: Box<Node> = Box::default(),
    }
}

widget! {
    /// A paged container: one tab per page.
    Tabs {
        /// The selected page's index.
        selected: i64 = 0; Data => ValueType::Int { min: Some(0), max: None },
    }
    events {
        /// Raised with the index of the page the user selects.
        Change(index: i64),
    }
    content {
        /// The pages, in tab order.
        pages: Vec<Page> = Vec::new(),
    }
}
