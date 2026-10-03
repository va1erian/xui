#![forbid(unsafe_code)]

//! Words wider than an empty line, split at grapheme boundaries.

use unicode_segmentation::UnicodeSegmentation;

use super::{Acc, Breaker, EPS};
use crate::layout::items::ItemKind;
use crate::layout::segment::Brk;

impl Breaker<'_, '_> {
    /// Fits as much of an atom wider than the empty line as possible, splitting
    /// a word at a grapheme. Returns the index of the first item left over.
    pub(super) fn overflow(&mut self, acc: &mut Acc, j: usize, end: usize, width: f32) -> usize {
        for k in j..end {
            let left = width - acc.used;
            if self.items[k].width <= left + EPS {
                let item = self.items[k].clone();
                acc.push(&item);
                continue;
            }
            if acc.content > 0 {
                return k;
            }
            if matches!(self.items[k].kind, ItemKind::Text(_)) {
                self.split_word(k, left);
            }
            let item = self.items[k].clone();
            acc.push(&item);
            return k + 1;
        }
        end
    }

    /// Splits the word at `k` so its head fits `left` pixels (at least one
    /// grapheme), inserting the tail as the next item.
    fn split_word(&mut self, k: usize, left: f32) {
        let item = self.items[k].clone();
        let ItemKind::Text(layout) = &item.kind else {
            return;
        };
        let word = &self.text[item.range.clone()];
        let bounds: Vec<usize> = word
            .grapheme_indices(true)
            .map(|(at, _)| at)
            .skip(1)
            .chain(std::iter::once(word.len()))
            .collect();
        if bounds.len() < 2 {
            return;
        }
        let hit = layout
            .hit_test_point(left.max(0.0), layout.height() / 2.0)
            .byte_index;
        let mut at = bounds.iter().rposition(|&b| b <= hit).unwrap_or(0);
        let start = item.range.start;
        let mut head = self.shape.text_item(
            self.text,
            start..start + bounds[at],
            item.style,
            Brk::Allowed,
        );
        while at > 0 && head.width > left + EPS {
            at -= 1;
            head = self.shape.text_item(
                self.text,
                start..start + bounds[at],
                item.style,
                Brk::Allowed,
            );
        }
        if bounds[at] >= word.len() {
            return;
        }
        let tail = self.shape.text_item(
            self.text,
            start + bounds[at]..item.range.end,
            item.style,
            item.brk,
        );
        self.items[k] = head;
        self.items.insert(k + 1, tail);
    }
}
