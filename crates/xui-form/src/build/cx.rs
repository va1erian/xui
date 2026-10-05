#![forbid(unsafe_code)]

//! [`BuildCx`]: what a factory receives when it describes a widget.

use std::collections::BTreeMap;
use std::rc::Rc;

use xui_core::app::Ui;
use xui_core::arrange::{IntoEntry, Layout, column};
use xui_core::layout::Placement;

use super::live::{Common, Live, WidgetProps};
use super::{Binder, Created, EventHandler, EventRef};
use crate::schema::{Catalog, WidgetSpec};
use crate::value::Value;

/// What a factory receives when it describes a widget: the widget's
/// properties, its events' handlers and, for a container, its content.
pub struct BuildCx<'a, M: 'static> {
    pub(super) ui: &'a Ui<M>,
    /// The name the live form knows the widget by (`digit[3]` for an element
    /// of a control array).
    pub(super) name: &'a str,
    /// The name written in the file (the array's name for an element).
    pub(super) base: &'a str,
    pub(super) index: Option<usize>,
    pub(super) props: BTreeMap<String, Value>,
    pub(super) spec: &'a WidgetSpec,
    pub(super) binder: &'a dyn Binder<M>,
    pub(super) design_mode: bool,
    pub(super) catalog: Rc<Catalog>,
    pub(super) placement: Option<Placement>,
    pub(super) content: Option<Layout<M>>,
    pub(super) pages: Vec<(String, Layout<M>)>,
}

impl<M: 'static> BuildCx<'_, M> {
    /// The [`Ui`] the form is built through.
    pub fn ui(&self) -> &Ui<M> {
        self.ui
    }

    /// The name the live form knows the widget by.
    pub fn name(&self) -> &str {
        self.name
    }

    /// For an element of a control array, its index.
    pub fn index(&self) -> Option<usize> {
        self.index
    }

    /// The widget kind's spec.
    pub fn spec(&self) -> &WidgetSpec {
        self.spec
    }

    /// For an entry of an `Absolute` layout, where it sits: its design
    /// rectangle and anchor, which the build places it with.
    pub fn placement(&self) -> Option<&Placement> {
        self.placement.as_ref()
    }

    /// Whether this is a design-mode build.
    pub fn design_mode(&self) -> bool {
        self.design_mode
    }

    /// For a panel or a group, the layout of its content (an empty column
    /// otherwise, or once taken).
    pub fn take_content(&mut self) -> Layout<M> {
        self.content.take().unwrap_or_else(column)
    }

    /// For a tab control, its pages' titles and layouts (empty otherwise, or
    /// once taken).
    pub fn take_pages(&mut self) -> Vec<(String, Layout<M>)> {
        std::mem::take(&mut self.pages)
    }

    /// The value of a property; every declared property has one.
    pub fn prop(&self, name: &str) -> Option<&Value> {
        self.props.get(name)
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
    /// run the host's event code; an unnamed widget raises nothing either.
    pub fn handler(&self, event: &str) -> Option<EventHandler<M>> {
        if self.design_mode || self.base.is_empty() {
            return None;
        }
        let spec = self.spec.event(event)?;
        self.binder.bind(EventRef {
            node: self.base,
            index: self.index,
            event,
            spec,
        })
    }

    /// Pairs `entry` with a built-in widget's property surface, adding the
    /// common properties and the schema's access and type rules.
    pub(crate) fn live<W: WidgetProps<M>>(&self, entry: impl IntoEntry<M>, inner: W) -> Created<M> {
        let common = Common::new(self.ui.clone(), self.placement.clone());
        common.init(self.bool("visible", true), self.bool("enabled", true));
        Created::new(
            entry,
            Live {
                common,
                inner,
                catalog: Rc::clone(&self.catalog),
                kind: self.spec.kind.clone(),
                design_mode: self.design_mode,
            },
        )
    }
}
