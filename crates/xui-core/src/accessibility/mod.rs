#![forbid(unsafe_code)]

//! The portable accessibility tree model: a widget describes itself with a
//! [`Node`] tree, and a client performs [`Action`]s on it. Each backend turns
//! the tree into its platform's accessibility API.

mod node;

pub use node::{Action, Node, RangeValue, Role};
