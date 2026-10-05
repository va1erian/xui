#![forbid(unsafe_code)]

//! Builders for containers that hold layouts: [`group`], [`scroll`] and
//! [`tabs`].

use super::{Build, Entry, Handle, IntoEntry, Kind, Layout, build};
use crate::widget::{GroupBox, ScrollView, Tabs};

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
