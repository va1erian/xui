#![forbid(unsafe_code)]

//! Pure wrapped-run layout: greedy word wrap across runs plus an end-ellipsis
//! when the wrapped block is taller than the box it is painted into.
//!
//! It measures through a caller-supplied closure, so the arithmetic is
//! testable without a backend and the widget can feed it
//! [`Ui::measure_text`](crate::app::Ui::measure_text).

use crate::backend::{TextMetrics, TextStyle};
use crate::color::Color;
use crate::geometry::Rect;
use crate::units::Dip;

/// The end-of-text ellipsis.
const ELLIPSIS: &str = "\u{2026}";
/// The size a placeholder style uses when a line has no pieces to measure the
/// ellipsis with.
const FALLBACK_SIZE: Dip = Dip(14.0);

/// One run as the layout sees it: its text and its resolved style.
pub(crate) struct Span<'a> {
    /// The index of the run this span came from.
    pub(crate) run: usize,
    /// The run's text.
    pub(crate) text: &'a str,
    /// The run's resolved style.
    pub(crate) style: TextStyle,
}

/// One placed piece of a run, in block-local coordinates.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Fragment {
    /// The run this fragment belongs to, or `None` for the ellipsis.
    pub(crate) run: Option<usize>,
    /// The text painted in the fragment's box.
    pub(crate) text: String,
    /// The fragment's box, relative to the block's origin.
    pub(crate) rect: Rect,
}

/// A laid-out block of runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Layout {
    /// The placed pieces, in paint order.
    pub(crate) fragments: Vec<Fragment>,
    /// The width of the widest line.
    pub(crate) width: i32,
    /// The total height.
    pub(crate) height: i32,
}

impl Layout {
    /// The fragment whose box contains `(x, y)`, topmost first.
    pub(crate) fn fragment_at(&self, x: i32, y: i32) -> Option<&Fragment> {
        self.fragments.iter().find(|fragment| {
            let rect = fragment.rect;
            x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
        })
    }
}

/// One token of a run's text: a word, a run of whitespace collapsed to one
/// space, or a hard line break.
enum Token<'a> {
    Word(&'a str),
    Space,
    Newline,
}

/// Splits `text` into words, collapsed whitespace and line breaks.
fn tokens(text: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some(&(start, ch)) = chars.peek() {
        if ch == '\r' {
            chars.next();
        } else if ch == '\n' {
            out.push(Token::Newline);
            chars.next();
        } else if ch.is_whitespace() {
            while let Some(&(_, c)) = chars.peek() {
                if c == '\n' || c == '\r' || !c.is_whitespace() {
                    break;
                }
                chars.next();
            }
            out.push(Token::Space);
        } else {
            let mut end = start + ch.len_utf8();
            chars.next();
            while let Some(&(index, c)) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                end = index + c.len_utf8();
                chars.next();
            }
            out.push(Token::Word(&text[start..end]));
        }
    }
    out
}

/// A piece before it is given a final position.
struct Piece {
    run: Option<usize>,
    text: String,
    width: i32,
    height: i32,
    style: TextStyle,
}

/// A line under construction or finished.
struct Line {
    pieces: Vec<Piece>,
    width: i32,
    height: i32,
}

/// Accumulates tokens into wrapped lines.
struct Builder<'m> {
    max_width: i32,
    measure: &'m mut dyn FnMut(&str, &TextStyle) -> TextMetrics,
    lines: Vec<Line>,
    pieces: Vec<Piece>,
    width: i32,
    height: i32,
    pending: Option<(usize, TextStyle)>,
}

impl Builder<'_> {
    /// Places `text` on the current line, wrapping first when it would not fit
    /// and emitting a pending space when there is one.
    fn place(&mut self, run: usize, text: &str, style: &TextStyle) {
        let metrics = (self.measure)(text, style);
        let mut space = 0;
        if !self.pieces.is_empty()
            && let Some((_, space_style)) = &self.pending
        {
            space = (self.measure)(" ", space_style).width;
        }
        if !self.pieces.is_empty() && self.width + space + metrics.width > self.max_width {
            self.flush();
            space = 0;
        }
        if space > 0
            && let Some((space_run, space_style)) = self.pending.take()
        {
            let metrics = (self.measure)(" ", &space_style);
            self.push(space_run, " ", metrics, space_style);
        }
        self.push(run, text, metrics, style.clone());
        self.pending = None;
    }

    fn push(&mut self, run: usize, text: &str, metrics: TextMetrics, style: TextStyle) {
        self.pieces.push(Piece {
            run: Some(run),
            text: text.to_string(),
            width: metrics.width,
            height: metrics.height,
            style,
        });
        self.width += metrics.width;
        self.height = self.height.max(metrics.height);
    }

    fn flush(&mut self) {
        if self.pieces.is_empty() {
            self.pending = None;
            return;
        }
        let pieces = std::mem::take(&mut self.pieces);
        self.lines.push(Line {
            pieces,
            width: self.width,
            height: self.height.max(1),
        });
        self.width = 0;
        self.height = 0;
        self.pending = None;
    }
}

/// Lays `spans` out as one wrapped block, ellipsizing the last fitting line
/// when `max_height` is positive and the block is taller.
pub(crate) fn layout(
    spans: &[Span<'_>],
    max_width: i32,
    max_height: i32,
    measure: &mut dyn FnMut(&str, &TextStyle) -> TextMetrics,
) -> Layout {
    let max_width = max_width.max(1);
    let mut builder = Builder {
        max_width,
        measure,
        lines: Vec::new(),
        pieces: Vec::new(),
        width: 0,
        height: 0,
        pending: None,
    };
    for span in spans {
        for token in tokens(span.text) {
            match token {
                Token::Newline => builder.flush(),
                Token::Space => builder.pending = Some((span.run, span.style.clone())),
                Token::Word(word) => builder.place(span.run, word, &span.style),
            }
        }
    }
    builder.flush();

    let mut lines = builder.lines;
    let fits = |lines: &[Line]| {
        let total: i32 = lines.iter().map(|line| line.height).sum();
        max_height <= 0 || total <= max_height
    };
    if !fits(&lines) {
        ellipsize(&mut lines, max_width, max_height, measure);
    }
    assemble(&lines)
}

/// Truncates `lines` to the ones that fit `max_height` and appends an ellipsis
/// to the last kept line, dropping trailing pieces until it fits `max_width`.
fn ellipsize(
    lines: &mut Vec<Line>,
    max_width: i32,
    max_height: i32,
    measure: &mut dyn FnMut(&str, &TextStyle) -> TextMetrics,
) {
    let mut kept = 0;
    let mut top = 0;
    for line in lines.iter() {
        if kept > 0 && top + line.height > max_height {
            break;
        }
        top += line.height;
        kept += 1;
    }
    lines.truncate(kept.max(1));
    let Some(last) = lines.last_mut() else {
        return;
    };
    let style = last
        .pieces
        .last()
        .map(|piece| piece.style.clone())
        .unwrap_or_else(|| TextStyle::new(Color::rgb(0, 0, 0), FALLBACK_SIZE));
    let metrics = measure(ELLIPSIS, &style);
    while let Some(piece) = last.pieces.last() {
        if last.width + metrics.width <= max_width {
            break;
        }
        last.width -= piece.width;
        last.pieces.pop();
    }
    last.pieces.push(Piece {
        run: None,
        text: ELLIPSIS.to_string(),
        width: metrics.width,
        height: metrics.height.max(1),
        style,
    });
    last.width += metrics.width;
}

/// Turns finished lines into positioned [`Fragment`]s.
fn assemble(lines: &[Line]) -> Layout {
    let mut layout = Layout::default();
    let mut top = 0;
    for line in lines {
        let mut left = 0;
        for piece in &line.pieces {
            layout.fragments.push(Fragment {
                run: piece.run,
                text: piece.text.clone(),
                rect: Rect::new(left, top, left + piece.width, top + piece.height),
            });
            left += piece.width;
        }
        layout.width = layout.width.max(line.width);
        top += line.height;
    }
    layout.height = top;
    layout
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A measurer with a fixed 10px advance per character and 20px height.
    fn fixed(text: &str, _style: &TextStyle) -> TextMetrics {
        TextMetrics {
            width: text.chars().count() as i32 * 10,
            height: 20,
            ascent: 15,
            descent: 5,
        }
    }

    fn span<'a>(run: usize, text: &'a str) -> Span<'a> {
        Span {
            run,
            text,
            style: TextStyle::new(Color::rgb(0, 0, 0), Dip(14.0)),
        }
    }

    fn fragments(spans: &[Span], max_width: i32, max_height: i32) -> Layout {
        layout(spans, max_width, max_height, &mut fixed)
    }

    #[test]
    fn words_wrap_at_the_width() {
        let out = fragments(&[span(0, "aa bb cc")], 70, i32::MAX);
        assert_eq!(out.width, 50);
        assert_eq!(out.height, 40);
        let texts: Vec<&str> = out.fragments.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(texts, ["aa", " ", "bb", "cc"]);
        assert_eq!(
            out.fragments[3].rect.top, 20,
            "cc starts on the second line"
        );
    }

    #[test]
    fn runs_wrap_across_their_boundary() {
        let spans = [span(0, "hello "), span(1, "world")];
        let out = fragments(&spans, 70, i32::MAX);
        assert_eq!(out.height, 40, "the second run wrapped to a new line");
        assert_eq!(out.fragments.last().unwrap().run, Some(1));
        assert_eq!(out.fragments.last().unwrap().rect.left, 0);
    }

    #[test]
    fn a_hard_newline_breaks_the_line() {
        let out = fragments(&[span(0, "a\nb")], 1000, i32::MAX);
        assert_eq!(out.height, 40);
        assert_eq!(out.fragments[1].rect.top, 20);
    }

    #[test]
    fn an_ellipsis_marks_a_block_that_is_too_tall() {
        let out = fragments(&[span(0, "aa bb cc")], 70, 20);
        assert_eq!(out.height, 20, "one line was kept");
        assert_eq!(out.fragments.last().unwrap().text, ELLIPSIS);
        assert_eq!(out.fragments.last().unwrap().run, None);
    }

    #[test]
    fn an_ellipsis_drops_pieces_that_no_longer_fit() {
        let out = fragments(&[span(0, "aaaa bb")], 25, 20);
        assert_eq!(out.fragments.len(), 1);
        assert_eq!(out.fragments[0].text, ELLIPSIS);
        assert_eq!(out.fragments[0].rect.left, 0);
    }

    #[test]
    fn a_separator_keeps_its_space_and_run() {
        let spans = [span(0, "one"), span(1, " "), span(2, "two")];
        let out = fragments(&spans, 1000, i32::MAX);
        let texts: Vec<&str> = out.fragments.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(texts, ["one", " ", "two"]);
        assert_eq!(out.fragments[1].run, Some(1));
    }
}
