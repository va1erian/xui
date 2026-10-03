#![forbid(unsafe_code)]

//! Folding two inverse ops into one, which is how typing and backspace runs
//! become a single undo step.

use super::EditOp;
use crate::model::paragraph::Paragraph;

impl EditOp {
    /// Folds `newer`, the inverse of an edit made right after the one this is
    /// the inverse of, into `self`. Returns whether it could.
    ///
    /// Adjacent removals of typed text merge into one removal; adjacent
    /// reinsertions of deleted text (backspace, forward delete) merge into one
    /// reinsertion.
    pub(crate) fn merge_inverse(&mut self, newer: &EditOp) -> bool {
        match (&mut *self, newer) {
            (EditOp::Remove { range, .. }, EditOp::Remove { range: next, .. })
                if range.start.para == range.end.para
                    && next.start == range.end
                    && next.end.para == next.start.para =>
            {
                range.end = next.end;
                true
            }
            (
                EditOp::Reinsert { at, content },
                EditOp::Reinsert {
                    at: next_at,
                    content: next,
                },
            ) if content.paras.len() == 1 && next.paras.len() == 1 && at.para == next_at.para => {
                let newer_len = next.paras[0].text.len();
                let (first, second, start) = if next_at.byte + newer_len == at.byte {
                    (&next.paras[0], &content.paras[0], *next_at)
                } else if next_at.byte == at.byte {
                    (&content.paras[0], &next.paras[0], *at)
                } else {
                    return false;
                };
                let joined = Paragraph::concat(&[first, second], first.spans[0].style, first.style);
                let mut objects = content.objects.clone();
                objects.extend(next.objects.iter().cloned());
                *at = start;
                content.paras[0] = joined;
                content.objects = objects;
                true
            }
            _ => false,
        }
    }
}
