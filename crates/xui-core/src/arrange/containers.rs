#![forbid(unsafe_code)]

//! Builders for containers that hold layouts: [`group`], [`scroll`],
//! [`split`] and [`tabs`].

use std::cell::Cell;
use std::rc::Rc;

use super::{Build, Entry, Handle, IntoEntry, Kind, Layout, build};
use crate::units::{Dip, Px};
use crate::widget::{GroupBox, ScrollView, Split, Tabs};

/// A titled frame with `content` laid out inside it. The frame and its
/// content are one unit: [`bind`](GroupBuild::bind) the frame and hide it
/// with [`Ui::set_visible`](crate::app::Ui::set_visible), and the content
/// hides with it and gives up its space.
pub fn group<M: 'static>(title: impl Into<String>, content: Layout<M>) -> GroupBuild<M> {
    let title = title.into();
    GroupBuild {
        build: build(move |ui| GroupBox::new(ui, Default::default(), &title)),
        content,
    }
}

/// The builder [`group`] returns.
pub struct GroupBuild<M: 'static> {
    build: Build<GroupBox<M>, M>,
    content: Layout<M>,
}

impl<M: 'static> GroupBuild<M> {
    /// Fills `handle` with the frame when it is created.
    pub fn bind(mut self, handle: &Handle<GroupBox<M>>) -> GroupBuild<M> {
        self.build = self.build.bind(handle);
        self
    }
}

impl<M: 'static> IntoEntry<M> for GroupBuild<M> {
    fn into_entry(self) -> Entry<M> {
        Entry::new(Kind::Framed(self.build.realizer(), self.content))
    }
}

/// A scrolling area holding `content`: the content is measured at the area's
/// width with no bound on its height, and what does not fit scrolls. Give it
/// [`fill`](super::LayoutExt::fill) (or a fixed height) so it takes less
/// room than its content.
pub fn scroll<M: 'static>(content: Layout<M>) -> ScrollBuild<M> {
    ScrollBuild {
        build: build(|ui| ScrollView::new(ui, Default::default())),
        content,
    }
}

/// The builder [`scroll`] returns.
pub struct ScrollBuild<M: 'static> {
    build: Build<ScrollView<M>, M>,
    content: Layout<M>,
}

impl<M: 'static> ScrollBuild<M> {
    /// Raises `f(offset)` whenever the view scrolls.
    pub fn on_scroll(mut self, f: impl Fn(Px) -> M + 'static) -> ScrollBuild<M> {
        self.build = self
            .build
            .then(move |view| view.on_scroll(move |offset| Some(f(offset))));
        self
    }

    /// Fills `handle` with the scroll view when it is created.
    pub fn bind(mut self, handle: &Handle<ScrollView<M>>) -> ScrollBuild<M> {
        self.build = self.build.bind(handle);
        self
    }
}

impl<M: 'static> IntoEntry<M> for ScrollBuild<M> {
    fn into_entry(self) -> Entry<M> {
        let content = self.content;
        self.build
            .then_with(move |view, _| {
                view.set_layout(content)?;
                Ok(view)
            })
            .into_entry()
    }
}

/// Two panes side by side with a draggable divider between them: `a` on the
/// left, `b` on the right ([`stacked`](SplitBuild::stacked) puts `a` above).
pub fn split<M: 'static>(a: Layout<M>, b: Layout<M>) -> SplitBuild<M> {
    let stacked = Rc::new(Cell::new(false));
    let vertical = Rc::clone(&stacked);
    SplitBuild {
        build: build(move |ui| {
            if vertical.get() {
                Split::column(ui, Default::default())
            } else {
                Split::row(ui, Default::default())
            }
        }),
        stacked,
        panes: (a, b),
    }
}

/// The builder [`split`] returns.
pub struct SplitBuild<M: 'static> {
    build: Build<Split<M>, M>,
    stacked: Rc<Cell<bool>>,
    panes: (Layout<M>, Layout<M>),
}

impl<M: 'static> SplitBuild<M> {
    /// Stacks the panes, the first above the second.
    pub fn stacked(self) -> SplitBuild<M> {
        self.stacked.set(true);
        self
    }

    /// Starts with the first pane `extent` design units wide (or tall); the
    /// divider centres without it.
    pub fn position(mut self, extent: impl Into<Dip>) -> SplitBuild<M> {
        let extent = extent.into();
        self.build = self.build.then(move |split| {
            split.set_position(extent);
            split
        });
        self
    }

    /// Keeps the panes at least `a` and `b` design units.
    pub fn min(mut self, a: impl Into<Dip>, b: impl Into<Dip>) -> SplitBuild<M> {
        let (a, b) = (a.into(), b.into());
        self.build = self.build.then(move |split| {
            split.set_min(a, b);
            split
        });
        self
    }

    /// Raises `f(extent)` when the user drags the divider.
    pub fn on_moved(mut self, f: impl Fn(Dip) -> M + 'static) -> SplitBuild<M> {
        self.build = self
            .build
            .then(move |split| split.on_moved(move |extent| Some(f(extent))));
        self
    }

    /// Fills `handle` with the split when it is created.
    pub fn bind(mut self, handle: &Handle<Split<M>>) -> SplitBuild<M> {
        self.build = self.build.bind(handle);
        self
    }
}

impl<M: 'static> IntoEntry<M> for SplitBuild<M> {
    fn into_entry(self) -> Entry<M> {
        let (a, b) = self.panes;
        self.build
            .then_with(move |split, _| {
                split.set_layouts(a, b)?;
                Ok(split)
            })
            .into_entry()
    }
}

/// A paged container: one tab per [`page`](TabsBuild::page).
pub fn tabs<M: 'static>() -> TabsBuild<M> {
    TabsBuild {
        build: build(|ui| Tabs::new(ui, Default::default())),
        pages: Vec::new(),
    }
}

/// The builder [`tabs`] returns.
pub struct TabsBuild<M: 'static> {
    build: Build<Tabs<M>, M>,
    pages: Vec<(String, Layout<M>)>,
}

impl<M: 'static> TabsBuild<M> {
    /// Appends a page titled `title` holding `content`.
    pub fn page(mut self, title: impl Into<String>, content: Layout<M>) -> TabsBuild<M> {
        self.pages.push((title.into(), content));
        self
    }

    /// Raises `f(index)` when the user selects a page.
    pub fn on_change(mut self, f: impl Fn(usize) -> M + 'static) -> TabsBuild<M> {
        self.build = self
            .build
            .then(move |tabs| tabs.on_change(move |index| Some(f(index))));
        self
    }

    /// Maps a page selection through `f`, which may raise nothing.
    pub fn on_change_with(mut self, f: impl Fn(usize) -> Option<M> + 'static) -> TabsBuild<M> {
        self.build = self.build.then(move |tabs| tabs.on_change(f));
        self
    }

    /// Fills `handle` with the container when it is created.
    pub fn bind(mut self, handle: &Handle<Tabs<M>>) -> TabsBuild<M> {
        self.build = self.build.bind(handle);
        self
    }
}

impl<M: 'static> IntoEntry<M> for TabsBuild<M> {
    fn into_entry(self) -> Entry<M> {
        let pages = self.pages;
        self.build
            .then_with(move |tabs, _| {
                for (title, content) in pages {
                    tabs.add_layout_page(&title, content)?;
                }
                Ok(tabs)
            })
            .into_entry()
    }
}
