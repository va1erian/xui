#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Declarative forms for [xui](https://github.com/va1erian/xui).
//!
//! A form is a window's content described as data: a `.lfm` file in
//! [RON](https://github.com/ron-rs/ron) holding a tree of layouts and
//! widgets.
//!
//! * The **model** ([`Form`], [`Node`] and one struct per kind) is plain
//!   serde types: the file format and, through the macros that declare
//!   them, the **schema** ([`Catalog`]) are one definition.
//! * [`load`] reads a form, reporting a misspelt field or kind with a "did
//!   you mean"; [`Form::to_ron`] writes it back canonically, defaults
//!   omitted, byte-stable.
//! * [`Form::validate`] reports every remaining problem as a [`Diagnostic`].
//! * [`describe`] turns a form into the same `xui_core::arrange` builders
//!   Rust code writes, and [`build`] mounts them into a [`LiveForm`] whose
//!   widgets read and write their properties by name. Events map to the
//!   app's messages through a [`Binder`], such as [`Handlers`].
//!
//! ```
//! let form = xui_form::load(
//!     r#"Form(
//!         title: "Hello",
//!         root: Column(padding: 16, gap: 8, children: [
//!             Edit(name: "name_edit", placeholder: "Your name"),
//!             Button(name: "greet_button", text: "Greet"),
//!         ]),
//!     )"#,
//! )
//! .expect("the form loads");
//! assert_eq!(form.title, "Hello");
//! assert!(form.validate(&xui_form::Catalog::xui()).is_empty());
//! assert!(form.to_ron().starts_with("Form(title: \"Hello\", size: (320, 200), root: Column("));
//! ```
//!
//! A control array is one node with `array: n` (or several with `index: i`):
//! its elements are `name[0]`, `name[1]`, …, and their events carry the
//! element's index.

pub mod build;
mod codec;
mod factories;
#[cfg(feature = "migrate")]
pub mod migrate;
pub mod model;
pub mod schema;
mod suggest;
pub mod validate;
pub mod value;

pub use build::{
    Binder, BuildCx, BuildError, BuildOptions, Controls, Created, Described, EventHandler,
    EventRef, Factories, Handlers, LiveForm, LiveWidget, SetError, WidgetFactory, build,
    build_with, describe,
};
pub use codec::{LoadError, format, load};
pub use model::{FORMAT_VERSION, Form, Node};
pub use schema::{
    Access, ArgSpec, Catalog, EventSpec, FieldSpec, LayoutSpec, PropertySpec, WidgetSpec,
};
pub use suggest::closest;
pub use validate::{Diagnostic, Severity};
pub use value::{Value, ValueType};
