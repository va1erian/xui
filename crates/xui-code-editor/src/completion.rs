#![forbid(unsafe_code)]

//! Autocompletion: the host-facing [`Completer`] seam and the popup's state
//! machine.
//!
//! The editor knows nothing about any language. A host implements
//! [`Completer`] and the editor asks it for candidates when the user triggers
//! completion; the popup then filters, moves and accepts them here, with no
//! window and no backend, so every rule is unit-tested directly.
//!
//! Positions are **char** offsets into the buffer, like everywhere else in the
//! crate.

use std::rc::Rc;

/// The most rows the popup shows at once; a longer list scrolls.
pub(crate) const MAX_ROWS: usize = 8;

/// What a candidate is, for the small coloured marker beside its label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompletionKind {
    /// A language keyword.
    Keyword,
    /// A free function.
    Function,
    /// A variable or constant.
    Variable,
    /// A field or property of a value.
    Property,
    /// A method of a value.
    Method,
    /// A module or namespace.
    Module,
    /// A snippet that inserts more than a name.
    Snippet,
    /// Anything else.
    #[default]
    Other,
}

impl CompletionKind {
    /// The one-letter marker the popup draws for this kind.
    pub fn letter(self) -> char {
        match self {
            CompletionKind::Keyword => 'k',
            CompletionKind::Function => 'f',
            CompletionKind::Variable => 'v',
            CompletionKind::Property => 'p',
            CompletionKind::Method => 'm',
            CompletionKind::Module => 'M',
            CompletionKind::Snippet => 's',
            CompletionKind::Other => '\u{b7}',
        }
    }
}

/// One candidate in the popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionItem {
    /// The text shown in the list, and the text the filter matches against.
    pub label: String,
    /// The text that replaces the word being completed when the item is
    /// accepted. It is usually the label.
    pub insert: String,
    /// A dimmer note beside the label, such as a signature or a type.
    pub detail: Option<String>,
    /// What the item is.
    pub kind: CompletionKind,
}

impl CompletionItem {
    /// An item that shows and inserts `label`, with no detail.
    pub fn new(label: impl Into<String>, kind: CompletionKind) -> CompletionItem {
        let label = label.into();
        CompletionItem {
            insert: label.clone(),
            label,
            detail: None,
            kind,
        }
    }

    /// Sets the text inserted on accept, when it differs from the label.
    pub fn with_insert(mut self, insert: impl Into<String>) -> CompletionItem {
        self.insert = insert.into();
        self
    }

    /// Sets the dimmer note shown beside the label.
    pub fn with_detail(mut self, detail: impl Into<String>) -> CompletionItem {
        self.detail = Some(detail.into());
        self
    }
}

/// The candidates for one request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    /// The char offset where the word being completed starts. It must not be
    /// past the caret; the text from here to the caret is what the filter
    /// matches and what accepting replaces.
    pub start: usize,
    /// The candidates, in the order the host wants them listed.
    pub items: Vec<CompletionItem>,
}

/// A source of completions, supplied by the host.
///
/// The editor calls it from inside its event handler, so an implementation
/// must not call back into the [`Editor`](crate::Editor) that owns it.
pub trait Completer {
    /// The candidates for the word ending at `caret`, a char offset into
    /// `text`, or `None` when there is nothing to offer (inside a string, say).
    fn complete(&self, text: &str, caret: usize) -> Option<Completion>;
}

impl<T: Completer + ?Sized> Completer for Rc<T> {
    fn complete(&self, text: &str, caret: usize) -> Option<Completion> {
        (**self).complete(text, caret)
    }
}

impl<F> Completer for F
where
    F: Fn(&str, usize) -> Option<Completion>,
{
    fn complete(&self, text: &str, caret: usize) -> Option<Completion> {
        self(text, caret)
    }
}

/// The lower-cased chars of `text`, so comparisons ignore case.
fn folded(text: &str) -> Vec<char> {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// The indices of `items` that match `prefix`, best first.
///
/// Items whose label starts with the prefix (ignoring case) come first, then
/// those containing the prefix's chars in order (a subsequence), each group in
/// the host's order. An item whose `insert` already equals the prefix exactly
/// is left out: accepting it would change nothing. An empty prefix matches
/// everything.
pub(crate) fn rank(items: &[CompletionItem], prefix: &str) -> Vec<usize> {
    let wanted = folded(prefix);
    let mut by_prefix = Vec::new();
    let mut by_subsequence = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if !prefix.is_empty() && item.insert == prefix {
            continue;
        }
        let label = folded(&item.label);
        if label.starts_with(&wanted) {
            by_prefix.push(index);
        } else if is_subsequence(&wanted, &label) {
            by_subsequence.push(index);
        }
    }
    by_prefix.extend(by_subsequence);
    by_prefix
}

/// Whether `needle` occurs in `haystack` as a (not necessarily contiguous)
/// subsequence.
fn is_subsequence(needle: &[char], haystack: &[char]) -> bool {
    let mut rest = haystack.iter();
    needle.iter().all(|wanted| rest.any(|c| c == wanted))
}

/// The open popup: the candidates, which of them match, and the selection.
pub(crate) struct Popup {
    /// The char offset where the word being completed starts.
    start: usize,
    /// Every candidate the completer offered.
    items: Vec<CompletionItem>,
    /// The indices into `items` that match the current prefix, best first.
    shown: Vec<usize>,
    /// The selected row, an index into `shown`.
    selected: usize,
    /// The first visible row, an index into `shown`.
    first: usize,
    /// The prefix `shown` was last computed for.
    prefix: Option<String>,
}

impl Popup {
    /// Opens a popup over `completion` filtered by `prefix`, or `None` when
    /// nothing matches.
    pub(crate) fn open(completion: Completion, prefix: &str) -> Option<Popup> {
        let mut popup = Popup {
            start: completion.start,
            items: completion.items,
            shown: Vec::new(),
            selected: 0,
            first: 0,
            prefix: None,
        };
        popup.refilter(prefix).then_some(popup)
    }

    /// The char offset where the word being completed starts.
    pub(crate) fn start(&self) -> usize {
        self.start
    }

    /// Re-filters by `prefix`, selecting the best match. Returns whether any
    /// candidate remains. A prefix that has not changed keeps the selection.
    pub(crate) fn refilter(&mut self, prefix: &str) -> bool {
        if self.prefix.as_deref() == Some(prefix) {
            return !self.shown.is_empty();
        }
        self.prefix = Some(prefix.to_owned());
        self.shown = rank(&self.items, prefix);
        self.selected = 0;
        self.first = 0;
        !self.shown.is_empty()
    }

    /// How many candidates match.
    pub(crate) fn len(&self) -> usize {
        self.shown.len()
    }

    /// How many rows are visible: all of them up to [`MAX_ROWS`].
    pub(crate) fn visible_rows(&self) -> usize {
        self.shown.len().min(MAX_ROWS)
    }

    /// The first visible row.
    pub(crate) fn first(&self) -> usize {
        self.first
    }

    /// The selected row.
    pub(crate) fn selected(&self) -> usize {
        self.selected
    }

    /// The matching candidate at `row`.
    pub(crate) fn item(&self, row: usize) -> Option<&CompletionItem> {
        self.shown.get(row).and_then(|index| self.items.get(*index))
    }

    /// The selected candidate.
    pub(crate) fn current(&self) -> Option<&CompletionItem> {
        self.item(self.selected)
    }

    /// Moves the selection by `delta` rows. With `wrap`, stepping past either
    /// end continues from the other; without it the selection stops at the
    /// end. The selection stays in view.
    pub(crate) fn move_by(&mut self, delta: isize, wrap: bool) {
        let count = self.shown.len() as isize;
        if count == 0 {
            return;
        }
        let target = self.selected as isize + delta;
        self.selected = if wrap {
            target.rem_euclid(count)
        } else {
            target.clamp(0, count - 1)
        } as usize;
        self.reveal_selected();
    }

    /// Selects `row` if it exists.
    pub(crate) fn select(&mut self, row: usize) {
        if row < self.shown.len() {
            self.selected = row;
            self.reveal_selected();
        }
    }

    /// Scrolls the list by `rows` (negative is up) without moving the
    /// selection, clamped to the list.
    pub(crate) fn scroll_by(&mut self, rows: isize) {
        let max = self.shown.len().saturating_sub(MAX_ROWS) as isize;
        self.first = (self.first as isize + rows).clamp(0, max) as usize;
    }

    /// Scrolls the minimum needed to bring the selected row into view.
    fn reveal_selected(&mut self) {
        if self.selected < self.first {
            self.first = self.selected;
        } else if self.selected >= self.first + MAX_ROWS {
            self.first = self.selected + 1 - MAX_ROWS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(labels: &[&str]) -> Vec<CompletionItem> {
        labels
            .iter()
            .map(|label| CompletionItem::new(*label, CompletionKind::Other))
            .collect()
    }

    fn popup(labels: &[&str], prefix: &str) -> Popup {
        Popup::open(
            Completion {
                start: 0,
                items: items(labels),
            },
            prefix,
        )
        .expect("something matches")
    }

    fn shown(popup: &Popup) -> Vec<&str> {
        (0..popup.len())
            .map(|row| popup.item(row).expect("row").label.as_str())
            .collect()
    }

    #[test]
    fn prefix_matches_come_before_subsequence_matches() {
        let list = items(&["my_print", "print", "println", "Printer"]);
        let order: Vec<&str> = rank(&list, "pri")
            .into_iter()
            .map(|index| list[index].label.as_str())
            .collect();
        assert_eq!(order, ["print", "println", "Printer", "my_print"]);
    }

    #[test]
    fn matching_ignores_case_and_keeps_the_hosts_order_within_a_group() {
        let list = items(&["Beta", "bar", "BAZ"]);
        assert_eq!(rank(&list, "ba"), [1, 2, 0], "Beta only has b..a in order");
        assert_eq!(rank(&list, "BA"), [1, 2, 0]);
        assert_eq!(rank(&list, "b"), [0, 1, 2]);
    }

    #[test]
    fn a_subsequence_matches_when_no_prefix_does() {
        let list = items(&["to_string", "tail", "push_str"]);
        assert_eq!(rank(&list, "tstr"), [0], "t..s..t..r in order");
        assert_eq!(rank(&list, "zz"), Vec::<usize>::new());
    }

    #[test]
    fn an_empty_prefix_matches_everything() {
        let list = items(&["b", "a"]);
        assert_eq!(rank(&list, ""), [0, 1]);
    }

    #[test]
    fn an_item_that_equals_the_prefix_is_left_out() {
        let list = items(&["print", "println"]);
        assert_eq!(rank(&list, "print"), [1], "print would change nothing");
        let only = items(&["print"]);
        assert!(rank(&only, "print").is_empty());
    }

    #[test]
    fn filtering_compares_chars_not_bytes() {
        let list = items(&["éclair", "Écu", "eel", "ça"]);
        assert_eq!(rank(&list, "é"), [0, 1], "É folds to é");
        assert_eq!(rank(&list, "ça"), Vec::<usize>::new(), "an exact insert");
        assert_eq!(rank(&list, "ç"), [3]);
    }

    #[test]
    fn open_needs_at_least_one_match() {
        let completion = Completion {
            start: 3,
            items: items(&["alpha"]),
        };
        assert!(Popup::open(completion.clone(), "zz").is_none());
        let popup = Popup::open(completion, "al").expect("alpha matches");
        assert_eq!(popup.start(), 3);
        assert_eq!(popup.current().map(|i| i.label.as_str()), Some("alpha"));
    }

    #[test]
    fn refilter_narrows_and_resets_the_selection() {
        let mut popup = popup(&["apple", "apricot", "avocado"], "a");
        popup.move_by(2, false);
        assert_eq!(popup.selected(), 2);
        assert!(popup.refilter("ap"));
        assert_eq!(shown(&popup), ["apple", "apricot"]);
        assert_eq!(popup.selected(), 0);
        assert!(!popup.refilter("apx"), "nothing left");
        assert_eq!(popup.len(), 0);
        assert!(popup.current().is_none());
        // A wider prefix finds the earlier candidates again.
        assert!(popup.refilter("a"));
        assert_eq!(popup.len(), 3);
    }

    #[test]
    fn refiltering_by_the_same_prefix_keeps_the_selection() {
        let mut popup = popup(&["apple", "apricot", "avocado"], "a");
        popup.move_by(2, false);
        assert!(popup.refilter("a"));
        assert_eq!(popup.selected(), 2);
    }

    #[test]
    fn up_and_down_wrap_when_asked_and_clamp_otherwise() {
        let mut popup = popup(&["a1", "a2", "a3"], "a");
        popup.move_by(-1, true);
        assert_eq!(popup.selected(), 2, "up from the first wraps to the last");
        popup.move_by(1, true);
        assert_eq!(popup.selected(), 0, "down from the last wraps to the first");
        popup.move_by(-5, false);
        assert_eq!(popup.selected(), 0);
        popup.move_by(99, false);
        assert_eq!(popup.selected(), 2);
    }

    #[test]
    fn moving_on_an_empty_list_does_nothing() {
        let mut popup = popup(&["a1"], "a");
        popup.refilter("zz");
        popup.move_by(1, true);
        popup.move_by(-1, false);
        assert_eq!(popup.selected(), 0);
    }

    fn many(count: usize) -> Popup {
        let labels: Vec<String> = (0..count).map(|n| format!("item{n:02}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        popup(&refs, "item")
    }

    #[test]
    fn the_list_shows_at_most_eight_rows() {
        let popup = many(20);
        assert_eq!(popup.len(), 20);
        assert_eq!(popup.visible_rows(), MAX_ROWS);
        assert_eq!(popup.first(), 0);
        assert_eq!(many(3).visible_rows(), 3);
    }

    #[test]
    fn the_selection_scrolls_into_view() {
        let mut popup = many(20);
        popup.move_by(9, false);
        assert_eq!(popup.selected(), 9);
        assert_eq!(popup.first(), 2, "scrolled just enough to show row 9");
        popup.move_by(-8, false);
        assert_eq!(popup.selected(), 1);
        assert_eq!(popup.first(), 1);
        popup.move_by(-1, true);
        popup.move_by(-1, true);
        assert_eq!(popup.selected(), 19, "wrapped to the last row");
        assert_eq!(popup.first(), 12);
    }

    #[test]
    fn the_wheel_scrolls_without_moving_the_selection_and_clamps() {
        let mut popup = many(20);
        popup.scroll_by(3);
        assert_eq!((popup.first(), popup.selected()), (3, 0));
        popup.scroll_by(99);
        assert_eq!(popup.first(), 12, "20 rows, 8 visible");
        popup.scroll_by(-99);
        assert_eq!(popup.first(), 0);
        let mut short = many(5);
        short.scroll_by(2);
        assert_eq!(short.first(), 0, "nothing to scroll");
    }

    #[test]
    fn selecting_a_row_scrolls_it_into_view_and_ignores_missing_rows() {
        let mut popup = many(20);
        popup.select(15);
        assert_eq!(popup.selected(), 15);
        assert_eq!(popup.first(), 8);
        popup.select(99);
        assert_eq!(popup.selected(), 15);
    }

    #[test]
    fn items_build_with_an_insert_and_a_detail() {
        let item = CompletionItem::new("print", CompletionKind::Function)
            .with_insert("print(")
            .with_detail("fn(any)");
        assert_eq!(item.label, "print");
        assert_eq!(item.insert, "print(");
        assert_eq!(item.detail.as_deref(), Some("fn(any)"));
        let plain = CompletionItem::new("x", CompletionKind::Variable);
        assert_eq!(plain.insert, "x");
        assert_eq!(plain.detail, None);
    }

    #[test]
    fn every_kind_has_a_distinct_marker_letter() {
        let kinds = [
            CompletionKind::Keyword,
            CompletionKind::Function,
            CompletionKind::Variable,
            CompletionKind::Property,
            CompletionKind::Method,
            CompletionKind::Module,
            CompletionKind::Snippet,
            CompletionKind::Other,
        ];
        let mut letters: Vec<char> = kinds.iter().map(|kind| kind.letter()).collect();
        letters.sort_unstable();
        letters.dedup();
        assert_eq!(letters.len(), kinds.len());
    }

    #[test]
    fn closures_and_shared_completers_are_completers() {
        let by_closure = |_: &str, caret: usize| {
            Some(Completion {
                start: caret,
                items: items(&["x"]),
            })
        };
        assert_eq!(by_closure.complete("", 0).expect("some").start, 0);
        let shared: Rc<dyn Completer> = Rc::new(by_closure);
        assert_eq!(shared.complete("ab", 2).expect("some").start, 2);
        let boxed: Rc<Rc<dyn Completer>> = Rc::new(shared);
        assert!(boxed.complete("", 0).is_some());
    }
}
