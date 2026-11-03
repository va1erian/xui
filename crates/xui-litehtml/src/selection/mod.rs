//! Text selection over a [`TextRunTable`]: pure geometry, no layout.
//!
//! A selection is two carets, each "before character `ch` of run `run`".
//! Because runs are in document order, comparing carets compares positions in
//! the document, and a selection is the text between them. Everything here
//! takes points in the table's own space (document points).
//!
//! The character boundaries inside a run are taken from the run's left-to-right
//! `offsets` (see [`TextRun::offsets`](crate::TextRun::offsets)); the
//! Direct2D widget replaces those with DirectWrite's own hit-testing for
//! right-to-left and complex text, where the offsets are not accurate.

use crate::geom::{Point, Rect};
use crate::text_runs::{TextRun, TextRunTable};

/// A caret position: the boundary before character `ch` of run `run`
/// (`ch == char_count` is the boundary after the run's last character).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextPos {
    /// Index into [`TextRunTable::runs`].
    pub run: usize,
    /// Character (not byte) offset within the run.
    pub ch: usize,
}

/// The text between two carets. `anchor` is where the gesture started and
/// `head` where it is now, so `head` may come before `anchor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Where the selection started.
    pub anchor: TextPos,
    /// The end that moves.
    pub head: TextPos,
}

impl Selection {
    /// A selection covering nothing yet, at `pos`.
    pub fn caret(pos: TextPos) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    /// `(first, last)` in document order.
    pub fn ordered(&self) -> (TextPos, TextPos) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

/// How far apart two runs' vertical centres may be, as a fraction of the
/// shorter one's height, and still count as the same line.
const SAME_LINE: f32 = 0.5;

fn is_blank(run: &TextRun) -> bool {
    run.text.trim().is_empty()
}

/// `(vertical, horizontal)` distance from `p` to `rect`, zero inside it.
fn distance(rect: &Rect, p: Point) -> (f32, f32) {
    let dy = (rect.top - p.y).max(p.y - rect.bottom).max(0.0);
    let dx = (rect.left - p.x).max(p.x - rect.right).max(0.0);
    (dy, dx)
}

fn same_line(a: &TextRun, b: &TextRun) -> bool {
    let tolerance = a.rect.height().min(b.rect.height()) * SAME_LINE;
    (a.rect.center().y - b.rect.center().y).abs() < tolerance
}

/// Characters are grouped so a double-click on "Taux," picks "Taux" and a
/// click on the comma picks the punctuation.
fn char_class(c: char) -> u8 {
    if c.is_alphanumeric() || c == '_' {
        0
    } else if c.is_whitespace() {
        1
    } else {
        2
    }
}

impl TextRunTable {
    /// Whether `p` is over a piece of text (for the I-beam cursor).
    pub fn is_text_at(&self, p: Point) -> bool {
        self.runs.iter().any(|r| !is_blank(r) && r.rect.contains(p))
    }

    /// The run nearest `p`, snapping off-text points to the closest readable
    /// run (same line first). The Direct2D widget refines the character
    /// boundary inside this run with DirectWrite.
    pub fn nearest_run(&self, p: Point) -> Option<usize> {
        let mut best: Option<(usize, (f32, f32))> = None;
        for (i, run) in self.runs.iter().enumerate() {
            if is_blank(run) {
                continue;
            }
            let d = distance(&run.rect, p);
            let better = match &best {
                None => true,
                Some((_, b)) => d.0.total_cmp(&b.0).then(d.1.total_cmp(&b.1)).is_lt(),
            };
            if better {
                best = Some((i, d));
                if d == (0.0, 0.0) {
                    break;
                }
            }
        }
        best.map(|(i, _)| i)
    }

    /// The caret position nearest to `p`. Like a browser, a point in the
    /// margin or between lines snaps to the closest text (same line first),
    /// so a drag can wander off the text without losing the selection.
    pub fn pos_at(&self, p: Point) -> Option<TextPos> {
        let i = self.nearest_run(p)?;
        let run = &self.runs[i];
        let x = p.x - run.rect.left;
        let ch = char_at_x(run, x);
        Some(TextPos { run: i, ch })
    }

    /// The word around caret `pos`, for a double-click.
    pub fn word_at_pos(&self, pos: TextPos) -> Option<Selection> {
        let run = self.runs.get(pos.run)?;
        let chars: Vec<char> = run.text.chars().collect();
        // The character the caret sits on, or the last one if it is at the end.
        let i = pos.ch.min(chars.len().saturating_sub(1));
        let class = char_class(chars[i]);
        let mut from = i;
        while from > 0 && char_class(chars[from - 1]) == class {
            from -= 1;
        }
        let mut to = i + 1;
        while to < chars.len() && char_class(chars[to]) == class {
            to += 1;
        }
        Some(Selection {
            anchor: TextPos {
                run: pos.run,
                ch: from,
            },
            head: TextPos {
                run: pos.run,
                ch: to,
            },
        })
    }

    /// The word under `p`, for a double-click.
    pub fn word_at(&self, p: Point) -> Option<Selection> {
        self.pos_at(p).and_then(|pos| self.word_at_pos(pos))
    }

    /// The whole block (paragraph, table cell, list item) around caret `pos`,
    /// for a triple-click.
    pub fn block_at_pos(&self, pos: TextPos) -> Option<Selection> {
        let run = self.runs.get(pos.run)?;
        let block = run.block;
        let mut first = pos.run;
        while first > 0 && self.runs[first - 1].block == block {
            first -= 1;
        }
        let mut last = pos.run;
        while last + 1 < self.runs.len() && self.runs[last + 1].block == block {
            last += 1;
        }
        Some(Selection {
            anchor: TextPos { run: first, ch: 0 },
            head: TextPos {
                run: last,
                ch: self.runs[last].char_count(),
            },
        })
    }

    /// The whole block under `p`, for a triple-click.
    pub fn block_at(&self, p: Point) -> Option<Selection> {
        self.pos_at(p).and_then(|pos| self.block_at_pos(pos))
    }

    /// Everything, or `None` if there is no text.
    pub fn select_all(&self) -> Option<Selection> {
        let first = self.runs.iter().position(|r| !is_blank(r))?;
        let last = self.runs.iter().rposition(|r| !is_blank(r))?;
        Some(Selection {
            anchor: TextPos { run: first, ch: 0 },
            head: TextPos {
                run: last,
                ch: self.runs[last].char_count(),
            },
        })
    }

    /// Boxes to paint as the highlight, in document points: one per line,
    /// with the spaces between selected words filled in.
    pub fn selection_rects(&self, sel: &Selection) -> Vec<Rect> {
        let (start, end) = sel.ordered();
        let mut out: Vec<Rect> = Vec::new();
        let Some(last_run) = self.runs.len().checked_sub(1) else {
            return out;
        };
        // A selection that no longer fits the table (which the view prevents,
        // but a panic here would abort the app) paints nothing.
        for i in start.run..=end.run.min(last_run) {
            let run = &self.runs[i];
            let from = if i == start.run { start.ch } else { 0 };
            let to = (if i == end.run {
                end.ch
            } else {
                run.char_count()
            })
            .min(run.char_count());
            if from >= to {
                continue;
            }
            let rect = Rect {
                left: run.rect.left + run.offsets[from],
                top: run.rect.top,
                right: run.rect.left + run.offsets[to],
                bottom: run.rect.bottom,
            };
            let extends_last = out.last().is_some_and(|last| {
                (last.center().y - rect.center().y).abs() < rect.height() * SAME_LINE
                    && (rect.left - last.right).abs() < 2.0
            });
            if extends_last {
                let last = out.last_mut().unwrap();
                *last = last.union(rect);
            } else if !is_blank(run) {
                out.push(rect);
            }
            // A blank run (a space) that does not touch the previous
            // highlight is collapsed whitespace litehtml left lying around,
            // not a visible gap: it gets no box.
        }
        out
    }

    /// The selected text, as a copy should read: words joined by spaces,
    /// hard breaks (paragraph, list item, table row, `<br>`) as newlines,
    /// a blank line between separate paragraphs, and wrapped lines joined
    /// back up.
    pub fn selection_text(&self, sel: &Selection) -> String {
        let (start, end) = sel.ordered();
        let mut out = String::new();
        let Some(last_run) = self.runs.len().checked_sub(1) else {
            return out;
        };
        let mut prev: Option<&TextRun> = None;
        let mut saw_space = false;
        for i in start.run..=end.run.min(last_run) {
            let run = &self.runs[i];
            let from = if i == start.run { start.ch } else { 0 };
            let to = if i == end.run {
                end.ch
            } else {
                run.char_count()
            };
            if from >= to {
                continue;
            }
            let piece: String = run.text.chars().skip(from).take(to - from).collect();
            if piece.trim().is_empty() {
                saw_space = true;
                continue;
            }
            if let Some(prev) = prev {
                out.push_str(separator(prev, run, saw_space));
            }
            // Leading/trailing spaces inside a piece are a selection edge
            // falling on whitespace; the separator already decided.
            out.push_str(piece.trim());
            prev = Some(run);
            saw_space = piece.ends_with(' ');
        }
        out
    }
}

/// The character boundary `x` falls on within `run`, from its offsets.
fn char_at_x(run: &TextRun, x: f32) -> usize {
    match run.offsets.iter().position(|&o| o > x) {
        None => run.char_count(),
        Some(0) => 0,
        Some(k) => {
            // `x` falls in slot k-1: nearer to its start or its end.
            let (lo, hi) = (run.offsets[k - 1], run.offsets[k]);
            if x - lo < hi - x { k - 1 } else { k }
        }
    }
}

/// What goes between two consecutive words of a copy.
fn separator(prev: &TextRun, next: &TextRun, saw_space: bool) -> &'static str {
    if same_line(prev, next) {
        // Adjacent words with a space run between them (or a visible gap, as
        // between two table cells) are separate words.
        return if saw_space || next.rect.left - prev.rect.right > 1.0 {
            " "
        } else {
            ""
        };
    }
    if next.block != prev.block {
        // A new paragraph/cell/item. A vertical gap bigger than half a line
        // is the margin between paragraphs.
        let margin = next.rect.top - prev.rect.bottom > next.rect.height() * 0.5;
        return if margin || next.breaks_before >= 2 {
            "\n\n"
        } else {
            "\n"
        };
    }
    // Same block, new line: the text wrapped unless a `<br>` (or a newline in
    // preformatted text) forced the break.
    match next.breaks_before {
        0 => " ",
        1 => "\n",
        _ => "\n\n",
    }
}

#[cfg(test)]
mod tests;
