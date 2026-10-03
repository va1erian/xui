#![forbid(unsafe_code)]

//! Navigation over visual lines: line start and end, and vertical movement
//! that keeps a sticky x across paragraphs, indents and floats.
//!
//! A position at a soft wrap belongs to the later line (the downstream
//! affinity `caret_rect` uses), so Home on it stays on that line.

use super::Layout;
use crate::model::{Affinity, DocPos};

/// How many lines the average-height estimate for a page looks at.
const SAMPLE_LINES: usize = 64;

impl Layout {
    /// The paragraph and line index holding `pos` (downstream), with `pos`
    /// clamped into the document. `None` before the first layout.
    fn locate(&self, pos: DocPos) -> Option<(usize, usize, usize)> {
        let index = pos.para.min(self.paras.len().checked_sub(1)?);
        let para = &self.paras[index];
        if para.lines.is_empty() {
            return None;
        }
        let byte = pos.byte.min(para.text.len());
        Some((index, para.line_of(byte, Affinity::Downstream), byte))
    }

    /// The start of the visual line holding `pos`.
    pub fn line_start(&self, pos: DocPos) -> DocPos {
        let Some((index, li, _)) = self.locate(pos) else {
            return pos;
        };
        DocPos::new(index, self.paras[index].lines[li].range.start)
    }

    /// The end of the visual line holding `pos`: before a forced break, and on
    /// a wrapped line before its hanging spaces, so the caret stays on that
    /// line.
    pub fn line_end(&self, pos: DocPos) -> DocPos {
        let Some((index, li, _)) = self.locate(pos) else {
            return pos;
        };
        let para = &self.paras[index];
        let last = li + 1 == para.lines.len();
        DocPos::new(index, para.lines[li].visible_end(last))
    }

    /// Moves `pos` by `lines` visual lines (negative is up), across
    /// paragraphs, keeping the horizontal position: `sticky_x` is the x to
    /// aim for (layout pixels), or `None` to start from the caret's own x.
    /// Returns the new position and the x to pass next time. At the first or
    /// last line it does not move.
    pub fn vertical(&self, pos: DocPos, sticky_x: Option<f32>, lines: i32) -> (DocPos, f32) {
        let Some((index, li, byte)) = self.locate(pos) else {
            return (pos, sticky_x.unwrap_or(0.0));
        };
        let x = sticky_x.unwrap_or_else(|| {
            let para = &self.paras[index];
            para.lines[li].caret_x(&para.text, byte)
        });
        let (mut p, mut l) = (index, li);
        for _ in 0..lines.unsigned_abs() {
            if lines > 0 {
                if l + 1 < self.paras[p].lines.len() {
                    l += 1;
                } else if p + 1 < self.paras.len() && !self.paras[p + 1].lines.is_empty() {
                    (p, l) = (p + 1, 0);
                } else {
                    break;
                }
            } else if l > 0 {
                l -= 1;
            } else if p > 0 && !self.paras[p - 1].lines.is_empty() {
                (p, l) = (p - 1, self.paras[p - 1].lines.len() - 1);
            } else {
                break;
            }
        }
        if (p, l) == (index, li) {
            return (pos, x);
        }
        let para = &self.paras[p];
        let last = l + 1 == para.lines.len();
        (
            DocPos::new(p, para.lines[l].byte_at_x(&para.text, x, last)),
            x,
        )
    }

    /// How many lines a Page Up or Page Down moves in a view
    /// `viewport_height` pixels tall: the lines that fit, less one so a line
    /// of context stays, and at least one.
    pub fn page_lines(&self, viewport_height: f32) -> i32 {
        let heights: Vec<f32> = self
            .paras
            .iter()
            .flat_map(|p| p.lines.iter().map(|l| l.height))
            .take(SAMPLE_LINES)
            .collect();
        if heights.is_empty() {
            return 1;
        }
        let average = heights.iter().sum::<f32>() / heights.len() as f32;
        ((viewport_height / average).floor() as i32 - 1).max(1)
    }
}
