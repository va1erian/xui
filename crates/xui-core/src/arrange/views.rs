#![forbid(unsafe_code)]

//! Builders for radio groups, trees, colour choosers and panels.

use super::{Build, Entry, Handle, IntoEntry, Layout, build};
use crate::Color;
use crate::widget::{
    ColorPanel, ColorPicker, NodeId, Panel, RadioGroup, TreeModel, TreeRow, TreeView,
};

/// A column of radio options, one per entry of `labels`, the first selected.
pub fn radio_group<M: 'static>(labels: &[&str]) -> Build<RadioGroup<M>, M> {
    let labels: Vec<String> = labels.iter().map(|label| label.to_string()).collect();
    build(move |ui| {
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        RadioGroup::new(ui, Default::default(), &labels)
    })
}

impl<M: 'static> Build<RadioGroup<M>, M> {
    /// Starts with option `index` selected.
    pub fn selected(self, index: usize) -> Build<RadioGroup<M>, M> {
        self.then(move |group| {
            group.select(index);
            group
        })
    }

    /// Raises `f(index)` when the user picks an option.
    pub fn on_select(self, f: impl Fn(usize) -> M + 'static) -> Build<RadioGroup<M>, M> {
        self.then(move |group| group.on_select(move |index| Some(f(index))))
    }
}

/// A tree; fill it with [`rows`](Build::rows) or [`model`](Build::model).
pub fn tree_view<M: 'static>() -> Build<TreeView<M>, M> {
    build(|ui| TreeView::new(ui, Default::default(), &[]))
}

impl<M: 'static> Build<TreeView<M>, M> {
    /// Shows `rows`, a flattened tree (each row carries its depth).
    pub fn rows(self, rows: impl Into<Vec<TreeRow>>) -> Build<TreeView<M>, M> {
        let rows = rows.into();
        self.then(move |tree| {
            tree.set_rows(&rows);
            tree
        })
    }

    /// Shows `model`, loading a branch's children the first time it expands.
    pub fn model(self, model: impl TreeModel + 'static) -> Build<TreeView<M>, M> {
        self.then(move |tree| {
            tree.set_model(model);
            tree
        })
    }

    /// Raises `f(node)` when the selection moves to `node`.
    pub fn on_select(self, f: impl Fn(NodeId) -> M + 'static) -> Build<TreeView<M>, M> {
        self.then(move |tree| tree.on_select(move |node| Some(f(node))))
    }

    /// Raises `f(node)` when `node` is activated (double-click or Enter).
    pub fn on_activate(self, f: impl Fn(NodeId) -> M + 'static) -> Build<TreeView<M>, M> {
        self.then(move |tree| tree.on_activate(move |node| Some(f(node))))
    }
}

/// A grid of colour swatches choosing one of `colors`.
pub fn color_picker<M: 'static>(colors: &[Color]) -> Build<ColorPicker<M>, M> {
    let colors = colors.to_vec();
    build(move |ui| ColorPicker::new(ui, Default::default(), &colors))
}

impl<M: 'static> Build<ColorPicker<M>, M> {
    /// Shows `columns` swatches per row.
    pub fn columns(self, columns: usize) -> Build<ColorPicker<M>, M> {
        self.then(move |picker| picker.columns(columns))
    }

    /// Starts with `color` selected.
    pub fn selected(self, color: Color) -> Build<ColorPicker<M>, M> {
        self.then(move |picker| picker.selected(color))
    }

    /// Raises `f(color)` when the user picks a swatch.
    pub fn on_select(self, f: impl Fn(Color) -> M + 'static) -> Build<ColorPicker<M>, M> {
        self.then(move |picker| picker.on_select(move |color| Some(f(color))))
    }
}

/// A full colour chooser: swatches, a saturation/value field, a hue strip and
/// value boxes.
pub fn color_panel<M: 'static>() -> Build<ColorPanel<M>, M> {
    build(|ui| ColorPanel::new(ui, Default::default()))
}

impl<M: 'static> Build<ColorPanel<M>, M> {
    /// Starts at `color`.
    pub fn color(self, color: Color) -> Build<ColorPanel<M>, M> {
        self.then(move |panel| panel.with_color(color))
    }

    /// Raises `f(color)` while the colour changes.
    pub fn on_change(self, f: impl Fn(Color) -> M + 'static) -> Build<ColorPanel<M>, M> {
        self.then(move |panel| panel.on_change(move |color| Some(f(color))))
    }
}

/// A card holding `content`: a section of its own, laid out inside it.
pub fn panel<M: 'static>(content: Layout<M>) -> PanelBuild<M> {
    PanelBuild {
        card: true,
        handle: None,
        content,
    }
}

/// The builder [`panel`] returns.
pub struct PanelBuild<M: 'static> {
    card: bool,
    handle: Option<Handle<Panel<M>>>,
    content: Layout<M>,
}

impl<M: 'static> PanelBuild<M> {
    /// Draws nothing of its own: a container to show or hide as one.
    pub fn plain(mut self) -> PanelBuild<M> {
        self.card = false;
        self
    }

    /// Fills `handle` with the panel when it is created.
    pub fn bind(mut self, handle: &Handle<Panel<M>>) -> PanelBuild<M> {
        self.handle = Some(handle.clone());
        self
    }
}

impl<M: 'static> IntoEntry<M> for PanelBuild<M> {
    fn into_entry(self) -> Entry<M> {
        let (card, content) = (self.card, self.content);
        let mut panel = build(move |ui| {
            if card {
                Panel::new(ui, Default::default())
            } else {
                Panel::plain(ui, Default::default())
            }
        })
        .then_with(move |panel, _| {
            panel.set_layout(content)?;
            Ok(panel)
        });
        if let Some(handle) = &self.handle {
            panel = panel.bind(handle);
        }
        panel.into_entry()
    }
}
