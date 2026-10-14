#![forbid(unsafe_code)]

//! Checking widget names: identifiers, uniqueness and control arrays.
//!
//! A control array claims a range of indices of its name, and two claims
//! clash when the ranges overlap. Ranges are kept, never enumerated, so the
//! cost of a node does not depend on how large its array is.

use std::collections::BTreeMap;

use super::{Diagnostic, RESERVED_NAMES, Severity, is_identifier};
use crate::model::Widget;

/// The names used so far, and how.
#[derive(Default)]
pub(super) struct Names(BTreeMap<String, Used>);

/// How one name has been used.
#[derive(Default)]
struct Used {
    /// Whether it names a widget that is not a control array.
    plain: bool,
    /// The indices taken, as disjoint half-open ranges `start -> end`.
    taken: BTreeMap<u64, u64>,
}

impl Used {
    /// Takes `start..end`, merging it with the ranges it overlaps; whether
    /// none was taken before.
    fn take(&mut self, start: u64, end: u64) -> bool {
        if start >= end {
            return true;
        }
        // The ranges are disjoint and sorted, so those that overlap are the
        // last ones that begin before `end`, back to the first that stops at
        // or before `start`.
        let overlapped: Vec<u64> = self
            .taken
            .range(..end)
            .rev()
            .take_while(|&(_, &stop)| stop > start)
            .map(|(&begin, _)| begin)
            .collect();
        let (mut low, mut high) = (start, end);
        let free = overlapped.is_empty();
        for begin in overlapped {
            if let Some(stop) = self.taken.remove(&begin) {
                low = low.min(begin);
                high = high.max(stop);
            }
        }
        self.taken.insert(low, high);
        free
    }
}

impl Names {
    /// Checks `widget`'s name and its `array` and `index`.
    pub(super) fn check(&mut self, widget: &dyn Widget, out: &mut Vec<Diagnostic>) {
        let name = widget.name();
        let error = |property: &str, message: String| {
            Diagnostic::new(Severity::Error, Some(name), Some(property), message)
        };
        if name.is_empty() {
            if widget.array().is_some() || widget.index().is_some() {
                out.push(error("name", "a control array needs a name".to_owned()));
            }
            return;
        }
        if !is_identifier(name) {
            out.push(error("name", format!("`{name}` is not a valid name")));
            return;
        }
        if RESERVED_NAMES.contains(&name) {
            out.push(error(
                "name",
                format!("`{name}` is reserved: scripts use it for the form itself"),
            ));
        }
        let used = self.0.entry(name.to_owned()).or_default();
        let range = match (widget.array(), widget.index()) {
            (Some(_), Some(_)) => {
                out.push(error(
                    "array",
                    "a node sets either `array` or `index`, not both".to_owned(),
                ));
                None
            }
            (Some(0), None) => {
                out.push(error(
                    "array",
                    "`array` is 0, which builds nothing; omit the node instead".to_owned(),
                ));
                Some((0, 0))
            }
            (Some(count), None) => Some((0, u64::from(count))),
            (None, Some(index)) => Some((u64::from(index), u64::from(index) + 1)),
            (None, None) => None,
        };
        let clash = match range {
            // Not a control array: it clashes with every other use.
            None => {
                let clash = used.plain || !used.taken.is_empty();
                used.plain = true;
                clash
            }
            Some((start, end)) => !used.take(start, end) || used.plain,
        };
        if clash {
            out.push(error("name", format!("`{name}` is used more than once")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_merge_and_clash_without_enumerating() {
        let mut used = Used::default();
        assert!(used.take(0, 3));
        assert!(used.take(3, 4), "adjacent is not overlapping");
        assert!(!used.take(2, 3), "inside the first");
        assert!(used.take(10, 20));
        assert!(!used.take(0, 100), "spans several, merging them");
        assert_eq!(used.taken.len(), 1);
        assert!(used.take(100, u64::from(u32::MAX) + 1));
        assert!(!used.take(u64::from(u32::MAX), u64::from(u32::MAX) + 1));
        assert!(used.take(5, 5), "an empty range takes nothing");
    }
}
