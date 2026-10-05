#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Declarative forms for [xui](https://github.com/va1erian/xui).
//!
//! This crate adds a form layer on top of the portable `xui-core` widgets:
//!
//! * a **schema** ([`Catalog`], [`WidgetSpec`]) that describes each widget kind,
//!   its typed properties and the events it raises;
//! * a **document** ([`FormDoc`]) that stores a form as plain TOML and round
//!   trips byte-identically;
//! * **validation** ([`FormDoc::validate`]) that reports every problem as a
//!   [`Diagnostic`];
//! * a **builder** ([`build`]) that turns a document into live `xui` widgets,
//!   mapping events through a host-supplied [`Binder`].
//!
//! # Layout
//!
//! A format-1 form positions its nodes absolutely. [`build`] turns each
//! container's children into an `xui_core::arrange::absolute` layout, every
//! node an entry at its `left`/`top`/`width`/`height` with its `anchor`, so the
//! form re-anchors itself whenever the window is resized. A `GroupBox` places
//! its children inside its frame, below its title.
//!
//! # Extending
//!
//! The crate knows nothing about any particular application. A consumer
//! registers its own widget kinds (a [`WidgetSpec`] plus a [`WidgetFactory`])
//! or aliases a built-in one (see [`Catalog::alias`]) to expose
//! application-specific names.
//!
//! # Example
//!
//! ```
//! use xui_form::{Catalog, FormDoc, Value};
//!
//! let catalog = Catalog::xui();
//! let doc = FormDoc::from_toml(
//!     r#"
//! format = 1
//!
//! [window]
//! name = "main_form"
//! title = "Hello"
//!
//! [[node]]
//! kind = "Button"
//! name = "cmdGo"
//! text = "Go"
//! "#,
//!     &catalog,
//! )
//! .expect("the form loads");
//!
//! assert_eq!(doc.node("cmdGo").and_then(|n| n.prop("text")), Some(&Value::Text("Go".to_owned())));
//! assert_eq!(doc.to_toml(&catalog).contains("title = \"Hello\""), true);
//! ```

pub mod build;
pub mod doc;
pub mod schema;
pub mod validate;
pub mod value;

mod factories;

/// The current document format; loading anything newer is an error.
pub const FORMAT_VERSION: u32 = 1;

pub use build::{
    Binder, BuildCx, BuildError, BuildOptions, Created, EventHandler, EventRef, Factories,
    LiveForm, LiveWidget, SetError, WidgetFactory, build, build_with,
};
pub use doc::{FormDoc, LoadError, Node, WindowNode};
pub use schema::{Access, ArgSpec, Catalog, Children, EventSpec, PropertySpec, WidgetSpec};
pub use validate::{Diagnostic, Severity};
pub use value::{DecodeError, Value, ValueType};
