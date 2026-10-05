//! The portable kitchen-sink app: its `Msg`, the handles it keeps, and the
//! `update` dispatch. The widgets are assembled in [`build`](super::build).

use std::rc::Rc;

use xui_core::Theme;
use xui_core::app::{App as AppTrait, Ui};
use xui_core::arrange::Handle;
use xui_core::color::Color;
use xui_core::geometry::Point;
use xui_core::units::Dip;
use xui_core::widget::{
    Dialog, DialogAction, ListView, Menu, Panel, SortDirection, StatusBar, Tooltip, TreeView,
};

use super::data::{Track, TrackModel};

/// The navigator row that selects each page, and the page a row selects.
const ROW_TO_PAGE: [usize; 6] = [0, 0, 0, 1, 2, 3];
const PAGE_TO_ROW: [usize; 4] = [0, 3, 4, 5];

#[derive(Clone)]
pub(crate) enum Msg {
    Navigate(usize),
    Select(Vec<usize>),
    Activate(usize),
    Sort(usize),
    Resize(usize, Dip),
    AlbumSelect(usize),
    AlbumActivate(usize),
    Search(String),
    Tool(usize),
    Folder(usize),
    Number(f64),
    Check(bool),
    Toggle(bool),
    Combo(usize),
    Slide(f64),
    Swatch(Color),
    Link,
    Theme(usize),
    DialogOpen,
    DialogAction(DialogAction),
    Context(usize, Point),
    Autoclose,
}

/// The app state. The window's root layout owns the widgets; the app keeps
/// handles to the ones it changes, and the window-level dialog, context menu
/// and tooltips that no layout holds.
pub(crate) struct App {
    pub(super) status: Handle<StatusBar<Msg>>,
    pub(super) nav: Handle<TreeView<Msg>>,
    pub(super) dialog: Dialog<Msg>,
    pub(super) context: Menu<Msg>,
    pub(super) list: Handle<ListView<Msg>>,
    pub(super) tracks: Rc<Vec<Track>>,
    pub(super) order: Vec<usize>,
    pub(super) sort: Option<(usize, bool)>,
    pub(super) pages: Vec<Handle<Panel<Msg>>>,
    pub(super) _tips: Vec<Tooltip<Msg>>,
}

impl App {
    fn model(&self) -> TrackModel {
        TrackModel::new(Rc::clone(&self.tracks), self.order.clone())
    }

    fn sort_by(&mut self, column: usize) {
        let ascending = match self.sort {
            Some((sorted, was)) if sorted == column => !was,
            _ => true,
        };
        let list = self.list.get();
        if let Some((sorted, _)) = self.sort
            && sorted != column
        {
            list.clear_sort_indicator(sorted);
        }
        let tracks = Rc::clone(&self.tracks);
        let key = |&row: &usize| &tracks[row];
        match column {
            1 => self.order.sort_by(|a, b| key(a).artist.cmp(&key(b).artist)),
            2 => self.order.sort_by(|a, b| key(a).album.cmp(&key(b).album)),
            3 => self.order.sort_by(|a, b| key(a).genre.cmp(&key(b).genre)),
            4 => self.order.sort_by(|a, b| key(a).year.cmp(&key(b).year)),
            5 => self.order.sort_by(|a, b| key(a).time.cmp(&key(b).time)),
            6 => self.order.sort_by(|a, b| key(a).format.cmp(&key(b).format)),
            7 => self.order.sort_by(|a, b| key(a).plays.cmp(&key(b).plays)),
            _ => self.order.sort_by(|a, b| key(a).title.cmp(&key(b).title)),
        }
        if !ascending {
            self.order.reverse();
        }
        self.sort = Some((column, ascending));
        list.set_sort_indicator(
            column,
            if ascending {
                SortDirection::Ascending
            } else {
                SortDirection::Descending
            },
        );
        list.set_model(self.model());
    }

    pub(super) fn show_page(&self, page: usize, ui: &Ui<Msg>) {
        for (index, panel) in self.pages.iter().enumerate() {
            ui.set_visible(panel.get().id(), index == page);
        }
    }

    fn note(&self, text: &str) {
        self.status.get().set_text(1, text);
    }
}

impl AppTrait for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Navigate(page) => {
                let page = page.min(self.pages.len().saturating_sub(1));
                self.show_page(page, ui);
                self.nav.get().select(Some(PAGE_TO_ROW[page]));
                self.note(&format!("page {}", page + 1));
            }
            Msg::Select(rows) => {
                self.status
                    .get()
                    .set_text(0, &format!("{}/{} selected", rows.len(), self.order.len()));
                self.note("selection changed");
            }
            Msg::Activate(row) => self.note(&format!("play row {row}")),
            Msg::Sort(column) => {
                self.sort_by(column);
                self.note(&format!("sorted by column {column}"));
            }
            Msg::Resize(column, width) => {
                self.note(&format!("column {column} → {}px", width.value()))
            }
            Msg::AlbumSelect(index) => self.note(&format!("album {index} selected")),
            Msg::AlbumActivate(index) => self.note(&format!("open album {index}")),
            Msg::Search(text) => self.note(&format!("search: {text}")),
            Msg::Tool(index) => self.note(&format!("toolbar {index}")),
            Msg::Folder(row) => self.note(&format!("folder row {row}")),
            Msg::Number(value) => self.note(&format!("number: {value}")),
            Msg::Check(checked) => self.note(&format!("check: {checked}")),
            Msg::Toggle(checked) => self.note(&format!("toggle: {checked}")),
            Msg::Combo(index) => self.note(&format!("combo #{index}")),
            Msg::Slide(value) => self.note(&format!("slider: {value:.0}")),
            Msg::Swatch(color) => self.note(&format!(
                "accent #{:02X}{:02X}{:02X}",
                color.r, color.g, color.b
            )),
            Msg::Link => self.note("link clicked"),
            Msg::Theme(choice) => {
                ui.set_theme(if choice == 1 {
                    Theme::dark()
                } else {
                    Theme::light()
                });
                self.note(if choice == 1 {
                    "theme: dark"
                } else {
                    "theme: light"
                });
            }
            Msg::DialogOpen => {
                self.dialog.open();
                self.note("dialog opened");
            }
            Msg::DialogAction(action) => match action {
                DialogAction::Accept(text) if text.is_empty() => self.note("dialog accepted"),
                DialogAction::Accept(text) => self.note(&format!("dialog accepted: {text}")),
                DialogAction::Cancel => self.note("dialog cancelled"),
            },
            Msg::Context(_row, at) => {
                self.context.show_context(at.x, at.y);
                self.note("context menu");
            }
            Msg::Autoclose => ui.quit(),
        }
    }
}

/// The page a navigator row selects.
pub(crate) fn page_for_row(row: usize) -> usize {
    ROW_TO_PAGE.get(row).copied().unwrap_or(0)
}
