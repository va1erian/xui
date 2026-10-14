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
    /// Takes `start..end`, merging it with the ranges it overlaps. When some
    /// of it was taken before, the error is the first indices (as `start..end`)
    /// that were.
    fn take(&mut self, start: u64, end: u64) -> Result<(), (u64, u64)> {
        if start >= end {
            return Ok(());
        }
        // The ranges are disjoint and sorted, so those that overlap are the
        // last ones that begin before `end`, back to the first that stops at
        // or before `start`.
        let overlapped: Vec<(u64, u64)> = self
            .taken
            .range(..end)
            .rev()
            .take_while(|&(_, &stop)| stop > start)
            .map(|(&begin, &stop)| (begin, stop))
            .collect();
        // The last of them is the lowest.
        let taken_before = overlapped
            .last()
            .map(|&(begin, stop)| (start.max(begin), end.min(stop)));
        let (mut low, mut high) = (start, end);
        for (begin, stop) in overlapped {
            self.taken.remove(&begin);
            low = low.min(begin);
            high = high.max(stop);
        }
        self.taken.insert(low, high);
        taken_before.map_or(Ok(()), Err)
    }
}

/// What a clash of `name`'s elements `first..end` says: which of them.
fn clash_message(name: &str, first: u64, end: u64) -> String {
    if end - first == 1 {
        format!("`{name}[{first}]` is used more than once")
    } else {
        format!(
            "`{name}[{first}]` to `{name}[{}]` are used more than once",
            end - 1
        )
    }
}

impl Names {
    /// Checks `widget`'s name and its `array` and `index`. `repeatable` is
    /// whether the widget may be a control array at all: a container may not,
    /// which the limits report, so its `array` and `index` are not read here.
    pub(super) fn check(
        &mut self,
        widget: &dyn Widget,
        repeatable: bool,
        out: &mut Vec<Diagnostic>,
    ) {
        let (array, index) = if repeatable {
            (widget.array(), widget.index())
        } else {
            (None, None)
        };
        let name = widget.name();
        let error = |property: &str, message: String| {
            Diagnostic::new(Severity::Error, Some(name), Some(property), message)
        };
        if name.is_empty() {
            if array.is_some() || index.is_some() {
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
        let range = match (array, index) {
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
        let more_than_once = format!("`{name}` is used more than once");
        let clash = match range {
            // Not a control array: it clashes with every other use.
            None => {
                let clash = used.plain || !used.taken.is_empty();
                used.plain = true;
                clash.then_some(more_than_once)
            }
            Some((start, end)) => match used.take(start, end) {
                Err((first, end)) => Some(clash_message(name, first, end)),
                Ok(()) => used.plain.then_some(more_than_once),
            },
        };
        if let Some(message) = clash {
            out.push(error("name", message));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_merge_and_clash_without_enumerating() {
        let mut used = Used::default();
        assert_eq!(used.take(0, 3), Ok(()));
        assert_eq!(used.take(3, 4), Ok(()), "adjacent is not overlapping");
        assert_eq!(used.take(2, 3), Err((2, 3)), "inside the first");
        assert_eq!(used.take(10, 20), Ok(()));
        assert_eq!(used.take(0, 100), Err((0, 3)), "the lowest taken first");
        assert_eq!(used.taken.len(), 1, "merged");
        assert_eq!(used.take(100, u64::from(u32::MAX) + 1), Ok(()));
        let top = u64::from(u32::MAX);
        assert_eq!(used.take(top, top + 1), Err((top, top + 1)));
        assert_eq!(used.take(5, 5), Ok(()), "an empty range takes nothing");
    }
}
