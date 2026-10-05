#![forbid(unsafe_code)]

//! [`BuildCx`]: what a factory receives when it describes a widget.

use std::rc::Rc;

use xui_core::app::Ui;
use xui_core::arrange::{IntoEntry, Layout, absolute};
use xui_core::layout::Placement;

use super::live::{Common, Live, WidgetProps};
use super::{Binder, Created, EventHandler, EventRef};
use crate::doc::Node;
use crate::schema::{Catalog, WidgetSpec};
use crate::value::Value;

/// What a factory receives when it describes a widget.
pub struct BuildCx<'a, M: 'static> {
    ui: &'a Ui<M>,
    node: &'a Node,
    spec: &'a WidgetSpec,
    binder: &'a dyn Binder<M>,
    design_mode: bool,
    catalog: Rc<Catalog>,
    placement: Placement,
    content: Option<Layout<M>>,
}

impl<'a, M: 'static> BuildCx<'a, M> {
    /// The context for one node.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        ui: &'a Ui<M>,
        node: &'a Node,
        spec: &'a WidgetSpec,
        binder: &'a dyn Binder<M>,
        design_mode: bool,
        catalog: Rc<Catalog>,
        placement: Placement,
        content: Option<Layout<M>>,
    ) -> BuildCx<'a, M> {
        BuildCx {
            ui,
            node,
            spec,
            binder,
            design_mode,
            catalog,
            placement,
            content,
        }
    }

    /// The [`Ui`] the form is built through.
    pub fn ui(&self) -> &Ui<M> {
        self.ui
    }

    /// The node being built.
    pub fn node(&self) -> &Node {
        self.node
    }

    /// The node's widget spec.
    pub fn spec(&self) -> &WidgetSpec {
        self.spec
    }

    /// Where the node sits in its parent: its design rectangle and anchor.
    /// The build places the factory's entry with it.
    pub fn placement(&self) -> &Placement {
        &self.placement
    }

    /// Whether this is a design-mode build.
    pub fn design_mode(&self) -> bool {
        self.design_mode
    }

    /// For a container, the layout of its children, to place inside it (an
    /// empty layout for a leaf, or once taken).
    pub fn take_content(&mut self) -> Layout<M> {
        self.content.take().unwrap_or_else(absolute)
    }

    /// The raw value of a property, if set.
    pub fn prop(&self, name: &str) -> Option<&Value> {
        self.node.props.get(name)
    }

    /// A text property, or the empty string.
    pub fn text(&self, name: &str) -> String {
        self.prop(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }

    /// A bool property, or `default`.
    pub fn bool(&self, name: &str, default: bool) -> bool {
        self.prop(name).and_then(Value::as_bool).unwrap_or(default)
    }

    /// An int property, or `default`.
    pub fn int(&self, name: &str, default: i64) -> i64 {
        self.prop(name).and_then(Value::as_int).unwrap_or(default)
    }

    /// A float property, or `default`.
    pub fn float(&self, name: &str, default: f64) -> f64 {
        self.prop(name).and_then(Value::as_float).unwrap_or(default)
    }

    /// A list property, or an empty vector.
    pub fn list(&self, name: &str) -> Vec<String> {
        self.prop(name)
            .and_then(Value::as_list)
            .map(<[String]>::to_vec)
            .unwrap_or_default()
    }

    /// The handler for `event`, if the binder supplied one.
    ///
    /// In design mode this is always `None`, so the designer's preview does not
    /// run the host's event code.
    pub fn handler(&self, event: &str) -> Option<EventHandler<M>> {
        if self.design_mode {
            return None;
        }
        let spec = self.spec.event(event)?;
        self.binder.bind(EventRef {
            node: &self.node.name,
            event,
            spec,
        })
    }

    /// Pairs `entry` with a built-in widget's property surface, adding the
    /// common properties and the schema's access and type rules.
    pub(crate) fn live<W: WidgetProps<M>>(&self, entry: impl IntoEntry<M>, inner: W) -> Created<M> {
        Created::new(
            entry,
            Live {
                common: Common::new(self.ui.clone(), self.placement.clone()),
                inner,
                catalog: Rc::clone(&self.catalog),
                kind: self.spec.kind.clone(),
                design_mode: self.design_mode,
            },
        )
    }
}
