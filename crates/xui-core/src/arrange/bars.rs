#![forbid(unsafe_code)]

//! Builders for item views (`grid_view`, `icon_view`) and bars (`toolbar`,
//! `top_bar`, `menu_bar`, `material_status_bar`), and for `flow_text`.
//!
//! Options with no builder method go through [`Build::then`]:
//! `top_bar().then(|bar| bar.label(ID, "Ready"))`.

use super::{Build, build};
use crate::icon::IconRef;
use crate::widget::{
    FlowText, GridModel, GridView, IconModel, IconView, MaterialStatusBar, Menu, MenuId, MenuScope,
    Run, Toolbar, TopBar, TopBarId,
};

/// Owned copies of `items`, for a constructor that runs at mount time.
fn owned(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

/// Implements the selection builders of a view that maps an item index.
macro_rules! index_events {
    ($widget:ident) => {
        impl<M: 'static> Build<$widget<M>, M> {
            /// Raises `f(index)` when the selection moves to item `index`.
            pub fn on_select(self, f: impl Fn(usize) -> M + 'static) -> Build<$widget<M>, M> {
                self.then(move |view| view.on_select(move |index| Some(f(index))))
            }

            /// Raises `f(index)` when item `index` is activated (double-click
            /// or Enter).
            pub fn on_activate(self, f: impl Fn(usize) -> M + 'static) -> Build<$widget<M>, M> {
                self.then(move |view| view.on_activate(move |index| Some(f(index))))
            }
        }
    };
}

/// A grid of named tiles.
pub fn grid_view<M: 'static>(items: &[&str]) -> Build<GridView<M>, M> {
    let items = owned(items);
    build(move |ui| {
        let items: Vec<&str> = items.iter().map(String::as_str).collect();
        GridView::new(ui, Default::default(), &items)
    })
}

/// A grid of tiles from `model`, painted on demand for any number of them.
pub fn grid_view_with<M: 'static>(model: impl GridModel + 'static) -> Build<GridView<M>, M> {
    build(move |ui| GridView::with_model(ui, Default::default(), model))
}

index_events!(GridView);

/// A view of named icons, like a file browser's.
pub fn icon_view<M: 'static>(items: &[&str]) -> Build<IconView<M>, M> {
    let items = owned(items);
    build(move |ui| {
        let items: Vec<&str> = items.iter().map(String::as_str).collect();
        IconView::new(ui, Default::default(), &items)
    })
}

/// A view of icons from `model`, loaded on demand for any number of them.
pub fn icon_view_with<M: 'static>(model: impl IconModel + 'static) -> Build<IconView<M>, M> {
    build(move |ui| IconView::with_model(ui, Default::default(), model))
}

index_events!(IconView);

/// A strip of text buttons, one per entry of `labels`, each as wide as its
/// label.
pub fn text_toolbar<M: 'static>(labels: &[&str]) -> Build<Toolbar<M>, M> {
    let labels = owned(labels);
    build(move |ui| {
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        Toolbar::new(ui, Default::default(), &labels)
    })
}

/// An empty strip of buttons; add them with [`item`](Build::item).
pub fn toolbar<M: 'static>() -> Build<Toolbar<M>, M> {
    build(|ui| Toolbar::empty(ui, Default::default()))
}

impl<M: 'static> Build<Toolbar<M>, M> {
    /// Appends an icon button with a hover tooltip.
    pub fn item(
        self,
        icon: impl Into<IconRef>,
        tooltip: impl Into<String>,
    ) -> Build<Toolbar<M>, M> {
        let (icon, tooltip) = (icon.into(), tooltip.into());
        self.then(move |bar| bar.item(icon, &tooltip))
    }

    /// Appends an icon button with a label and a hover tooltip.
    pub fn item_with_text(
        self,
        icon: impl Into<IconRef>,
        tooltip: impl Into<String>,
        text: impl Into<String>,
    ) -> Build<Toolbar<M>, M> {
        let (icon, tooltip, text) = (icon.into(), tooltip.into(), text.into());
        self.then(move |bar| bar.item_with_text(icon, &tooltip, &text))
    }

    /// Appends a thin line between groups of buttons.
    pub fn separator(self) -> Build<Toolbar<M>, M> {
        self.then(Toolbar::separator)
    }

    /// Raises `f(index)` when button `index` is clicked.
    pub fn on_click(self, f: impl Fn(usize) -> M + 'static) -> Build<Toolbar<M>, M> {
        self.then(move |bar| bar.on_click(move |index| Some(f(index))))
    }
}

/// An empty material band; add items with its own builders through
/// [`then`](Build::then), and map them here.
pub fn top_bar<M: 'static>() -> Build<TopBar<M>, M> {
    build(|ui| TopBar::new(ui, Default::default()))
}

impl<M: 'static> Build<TopBar<M>, M> {
    /// Raises `f(id)` when icon button `id` is clicked.
    pub fn on_click(self, f: impl Fn(TopBarId) -> M + 'static) -> Build<TopBar<M>, M> {
        self.then(move |bar| bar.on_click(move |id| Some(f(id))))
    }

    /// Raises `f(id, on)` when toggle `id` flips.
    pub fn on_toggle(self, f: impl Fn(TopBarId, bool) -> M + 'static) -> Build<TopBar<M>, M> {
        self.then(move |bar| bar.on_toggle(move |id, on| Some(f(id, on))))
    }

    /// Raises `f(id, value)` while slider `id` moves.
    pub fn on_change(self, f: impl Fn(TopBarId, f64) -> M + 'static) -> Build<TopBar<M>, M> {
        self.then(move |bar| bar.on_change(move |id, value| Some(f(id, value))))
    }
}

/// A menu bar whose menus `fill` describes.
pub fn menu_bar<M: 'static>(fill: impl FnOnce(&mut MenuScope<'_>) + 'static) -> Build<Menu<M>, M> {
    build(move |ui| Ok(Menu::bar(ui, Default::default())?.build(fill)))
}

impl<M: 'static> Build<Menu<M>, M> {
    /// Raises `f(id)` when command `id` is chosen.
    pub fn on_select(self, f: impl Fn(MenuId) -> M + 'static) -> Build<Menu<M>, M> {
        self.then(move |menu| menu.on_select(move |id| Some(f(id))))
    }

    /// Maps a chosen command through `f`, which may raise nothing (a
    /// submenu's id, say).
    pub fn on_select_with(self, f: impl Fn(MenuId) -> Option<M> + 'static) -> Build<Menu<M>, M> {
        self.then(move |menu| menu.on_select(f))
    }

    /// Raises `f(id, checked)` when check item `id` flips.
    pub fn on_toggle(self, f: impl Fn(MenuId, bool) -> M + 'static) -> Build<Menu<M>, M> {
        self.then(move |menu| menu.on_toggle(move |id, on| Some(f(id, on))))
    }
}

/// A status line of `parts`, in the material style.
pub fn material_status_bar<M: 'static>(parts: &[&str]) -> Build<MaterialStatusBar<M>, M> {
    let parts = owned(parts);
    build(move |ui| {
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        MaterialStatusBar::new(ui, Default::default(), &parts)
    })
}

/// A line of styled runs that wraps to the width it is given; add them with
/// [`run`](Build::run).
pub fn flow_text<M: 'static>() -> Build<FlowText<M>, M> {
    build(|ui| FlowText::new(ui, Default::default()))
}

impl<M: 'static> Build<FlowText<M>, M> {
    /// Appends `run`.
    pub fn run(self, run: Run<M>) -> Build<FlowText<M>, M> {
        self.then(move |text| text.run(run))
    }

    /// Appends a weak separator run (`" · "`, say).
    pub fn separator(self, text: impl Into<String>) -> Build<FlowText<M>, M> {
        let text = text.into();
        self.then(move |flow| flow.separator(&text))
    }
}
