#![forbid(unsafe_code)]

//! Turning a form's tree into `arrange` builders.

use std::rc::Rc;

use xui_core::app::Ui;
use xui_core::arrange::{Entry, Layout, LayoutExt, absolute, column, grid, row, wrap};
use xui_core::layout::Placement;

use super::cx::BuildCx;
use super::{Binder, BuildError, BuildOptions, Factories, LiveWidget};
use crate::model::{Length, Node, Place, Stack, Widget};
use crate::schema::Catalog;
use crate::value::Value;

/// A named widget a walk described.
pub(crate) type Named<M> = (String, String, Box<dyn LiveWidget<M>>);

/// One walk over a form's tree.
pub(super) struct Walk<'a, M: 'static> {
    ui: &'a Ui<M>,
    catalog: Rc<Catalog>,
    factories: &'a Factories<M>,
    binder: &'a dyn Binder<M>,
    options: BuildOptions,
    widgets: Vec<Named<M>>,
}

impl<'a, M: 'static> Walk<'a, M> {
    pub(super) fn new(
        ui: &'a Ui<M>,
        catalog: Rc<Catalog>,
        factories: &'a Factories<M>,
        binder: &'a dyn Binder<M>,
        options: BuildOptions,
    ) -> Walk<'a, M> {
        Walk {
            ui,
            catalog,
            factories,
            binder,
            options,
            widgets: Vec::new(),
        }
    }

    /// Every named widget described, in tree order: its name, kind and live
    /// widget.
    pub(super) fn into_widgets(self) -> Vec<Named<M>> {
        self.widgets
    }

    /// `node` as a layout: a layout node as itself, anything else as the one
    /// entry of a column (the content of a panel, a group or a page).
    pub(super) fn content(&mut self, node: &Node) -> Result<Layout<M>, BuildError> {
        match self.layout(node)? {
            Some(layout) => Ok(layout),
            None => {
                let mut layout = column();
                for entry in self.entries(node, false)? {
                    layout = layout.child(entry);
                }
                Ok(layout)
            }
        }
    }

    /// The layout a layout node describes, or `None` for a widget.
    fn layout(&mut self, node: &Node) -> Result<Option<Layout<M>>, BuildError> {
        let (layout, children, absolute_children) = match node {
            Node::Row(stack) => (stacked(row(), stack), &stack.children, false),
            Node::Column(stack) => (stacked(column(), stack), &stack.children, false),
            Node::Wrap(stack) => (stacked(wrap(), stack), &stack.children, false),
            Node::Grid(spec) => {
                let tracks: Vec<_> = spec.columns.iter().map(|track| (*track).into()).collect();
                let mut layout = grid(tracks).gap(spec.gap.dip()).padding(spec.padding.0);
                if let Some(align) = spec.align_items {
                    layout = layout.align(align.into());
                }
                (layout, &spec.children, false)
            }
            Node::Absolute(spec) => {
                let mut layout = absolute().padding(spec.padding.0);
                if let Some((width, height)) = spec.size {
                    layout = layout.design_size(width.dip(), height.dip());
                }
                (layout, &spec.children, true)
            }
            _ => return Ok(None),
        };
        let mut layout = layout;
        for child in children {
            for entry in self.entries(child, absolute_children)? {
                layout = layout.child(entry);
            }
        }
        Ok(Some(layout))
    }

    /// The entries a node becomes in its parent: one, or one per element of
    /// a control array, each sized and placed by the node's layout fields.
    fn entries(&mut self, node: &Node, in_absolute: bool) -> Result<Vec<Entry<M>>, BuildError> {
        let place = node.place();
        if let Some(layout) = self.layout(node)? {
            let placement = placement(&place, in_absolute);
            return Ok(vec![placed(layout, &place, placement.as_ref())]);
        }
        let widget = node.widget().expect("a node is a layout or a widget");
        let elements: Vec<Option<u32>> = match (widget.array(), widget.index()) {
            (Some(count), _) => (0..count).map(Some).collect(),
            (None, index) => vec![index],
        };
        let mut entries = Vec::with_capacity(elements.len());
        for index in elements {
            let placement = placement(&place, in_absolute);
            let entry = self.widget(node, widget, index, placement.clone())?;
            entries.push(placed(entry, &place, placement.as_ref()));
        }
        Ok(entries)
    }

    /// One widget (or one element of a control array) through its factory.
    fn widget(
        &mut self,
        node: &Node,
        widget: &dyn Widget,
        index: Option<u32>,
        placement: Option<Placement>,
    ) -> Result<Entry<M>, BuildError> {
        let kind = widget.kind();
        let name = match index {
            Some(index) => format!("{}[{index}]", widget.name()),
            None => widget.name().to_owned(),
        };
        let factory = self
            .factories
            .get(kind)
            .ok_or_else(|| BuildError::UnknownFactory {
                kind: kind.to_owned(),
                node: name.clone(),
            })?;
        let mut props = widget.props();
        if let Some(index) = index {
            for value in props.values_mut() {
                if let Value::Text(text) = value {
                    *text = text.replace("{index}", &index.to_string());
                }
            }
        }
        let (content, pages) = self.nested(node)?;
        let spec = self
            .catalog
            .get(kind)
            .cloned()
            .expect("every widget kind is in the catalog");
        let mut cx = BuildCx {
            ui: self.ui,
            name: &name,
            base: widget.name(),
            index: index.map(|index| index as usize),
            props,
            spec: &spec,
            binder: self.binder,
            design_mode: self.options.design_mode,
            catalog: Rc::clone(&self.catalog),
            placement,
            content,
            pages,
        };
        let created = factory.create(&mut cx);
        if !widget.name().is_empty() && !self.widgets.iter().any(|(known, ..)| *known == name) {
            self.widgets.push((name, kind.to_owned(), created.widget));
        }
        Ok(created.entry)
    }

    /// The layouts a container widget holds: a panel's or group's content, or
    /// a tab control's pages.
    #[allow(clippy::type_complexity)]
    fn nested(
        &mut self,
        node: &Node,
    ) -> Result<(Option<Layout<M>>, Vec<(String, Layout<M>)>), BuildError> {
        Ok(match node {
            Node::Panel(panel) => (Some(self.content(&panel.content)?), Vec::new()),
            Node::Group(group) => (Some(self.content(&group.content)?), Vec::new()),
            Node::Tabs(tabs) => {
                let mut pages = Vec::with_capacity(tabs.pages.len());
                for page in &tabs.pages {
                    pages.push((page.title.clone(), self.content(&page.content)?));
                }
                (None, pages)
            }
            _ => (None, Vec::new()),
        })
    }
}

/// A row, column or wrap with a stack's spacing and alignment.
fn stacked<M: 'static>(layout: Layout<M>, stack: &Stack) -> Layout<M> {
    let mut layout = layout.gap(stack.gap.dip()).padding(stack.padding.0);
    if let Some(align) = stack.align_items {
        layout = layout.align(align.into());
    }
    if let Some(justify) = stack.justify {
        layout = layout.justify(justify.into());
    }
    layout
}

/// The live placement of an entry of an absolute layout, from its `at` and
/// `anchor`; `None` elsewhere, or when it has no `at`.
fn placement(place: &Place, in_absolute: bool) -> Option<Placement> {
    let (x, y, width, height) = place.at.filter(|_| in_absolute)?;
    let anchor = place
        .anchor
        .map_or(xui_core::layout::Anchor::TopLeft, Into::into);
    Some(Placement::new(
        x.dip(),
        y.dip(),
        width.dip(),
        height.dip(),
        anchor,
    ))
}

/// `entry` sized and placed by `place`.
fn placed<M: 'static>(
    entry: impl LayoutExt<M>,
    place: &Place,
    placement: Option<&Placement>,
) -> Entry<M> {
    let mut entry = entry.into_entry();
    let dip = |length: Length| length.dip();
    entry = match (place.width.map(dip), place.height.map(dip)) {
        (Some(width), Some(height)) => entry.size(width, height),
        (Some(width), None) => entry.width(width),
        (None, Some(height)) => entry.height(height),
        (None, None) => entry,
    };
    if let Some(weight) = place.fill {
        entry = entry.fill(weight);
    }
    if let Some(width) = place.max_width {
        entry = entry.max_width(width.dip());
    }
    if let Some(height) = place.max_height {
        entry = entry.max_height(height.dip());
    }
    if let Some(align) = place.align {
        entry = entry.align(align.into());
    }
    if let Some(span) = place.span {
        entry = entry.span(span as usize);
    }
    match placement {
        Some(placement) => entry.placement(placement),
        None => match place.anchor {
            Some(anchor) => entry.anchor(anchor.into()),
            None => entry,
        },
    }
}
