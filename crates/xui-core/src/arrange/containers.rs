#![forbid(unsafe_code)]

//! Builders for containers that hold layouts: [`group`] and [`tabs`].

use super::{Build, Entry, Handle, IntoEntry, Kind, Layout, build};
use crate::widget::{GroupBox, Tabs};

/// A titled frame with `content` laid out inside it.
pub fn group<M: 'static>(title: impl Into<String>, content: Layout<M>) -> Entry<M> {
    let title = title.into();
    let frame = build(move |ui| GroupBox::new(ui, Default::default(), &title));
    Entry::new(Kind::Framed(frame.realizer(), content))
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
