#![forbid(unsafe_code)]

//! The toolbar's entries and the state its painter and event mapper share.

use std::cell::{Cell, RefCell};

use crate::icon::IconRef;

use super::layout::Mode;

/// One toolbar item: an optional icon, an optional label and an optional
/// tooltip. At least one of the icon and the label is shown; a text-only item
/// has no icon.
pub(crate) struct Item {
    /// The item's leading icon.
    pub(crate) icon: Option<IconRef>,
    /// The item's text label, or `None` for an icon-only item.
    pub(crate) label: Option<String>,
    /// The item's hover tooltip, if any.
    pub(crate) tooltip: Option<String>,
}

impl Item {
    /// A text-only item.
    pub(super) fn text(label: &str) -> Item {
        Item {
            icon: None,
            label: Some(label.to_string()),
            tooltip: None,
        }
    }
}

/// One slot in the strip: a clickable item, or a separator line.
pub(super) enum Entry {
    /// A clickable item; it owns the next item index.
    Item(Item),
    /// A thin vertical line: it takes room but never an item index.
    Separator,
}

/// The strip's entries in order.
///
/// Every public index (`on_click`, `icon`, `label`, `tooltip`, `len`) counts
/// [`Entry::Item`]s only, so adding a separator never renumbers the items.
#[derive(Default)]
pub(super) struct Entries(Vec<Entry>);

impl Entries {
    /// Appends `entry`.
    pub(super) fn push(&mut self, entry: Entry) {
        self.0.push(entry);
    }

    /// Every entry, items and separators, in strip order.
    pub(super) fn all(&self) -> &[Entry] {
        &self.0
    }

    /// The number of items, not counting separators.
    pub(super) fn item_count(&self) -> usize {
        self.items().count()
    }

    /// The items in order, skipping separators.
    pub(super) fn items(&self) -> impl Iterator<Item = &Item> {
        self.0.iter().filter_map(|entry| match entry {
            Entry::Item(item) => Some(item),
            Entry::Separator => None,
        })
    }

    /// Item number `index` (separators not counted).
    pub(super) fn item(&self, index: usize) -> Option<&Item> {
        self.items().nth(index)
    }
}

/// What the painter and the event mapper both read.
pub(super) struct State {
    /// The strip's entries.
    pub(super) entries: RefCell<Entries>,
    /// How items are sized and placed.
    pub(super) mode: Cell<Mode>,
    /// The hovered (or keyboard-focused) item index.
    pub(super) hover: Cell<Option<usize>>,
    /// The item index under a held mouse button.
    pub(super) pressed: Cell<Option<usize>>,
    /// Whether the toolbar accepts input.
    pub(super) enabled: Cell<bool>,
}

impl State {
    /// An empty, enabled, compact strip.
    pub(super) fn new() -> State {
        State {
            entries: RefCell::new(Entries::default()),
            mode: Cell::new(Mode::Compact),
            hover: Cell::new(None),
            pressed: Cell::new(None),
            enabled: Cell::new(true),
        }
    }
}
