#![forbid(unsafe_code)]

//! Cutting paragraphs into pieces and joining pieces back, the primitives
//! every text edit is built from. Spans stay normalized throughout.

use super::object::OBJECT_CHAR;
use super::paragraph::{Paragraph, Span};
use super::style::{CharStyleId, ParaStyleId};

/// Drops empty spans and merges equal neighbours; an empty result becomes the
/// single empty span of `fallback`.
pub(crate) fn normalize(spans: impl IntoIterator<Item = Span>, fallback: CharStyleId) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for span in spans {
        if span.len == 0 {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.style == span.style => last.len += span.len,
            _ => out.push(span),
        }
    }
    if out.is_empty() {
        out.push(Span {
            len: 0,
            style: fallback,
        });
    }
    out
}

/// The part of `spans` inside `lo..hi`, mapped through `f`, with the parts
/// outside kept as they are.
pub(crate) fn map_range(
    spans: &[Span],
    lo: usize,
    hi: usize,
    mut f: impl FnMut(CharStyleId) -> CharStyleId,
) -> Vec<Span> {
    let mut out = Vec::with_capacity(spans.len() + 2);
    let mut start = 0;
    for span in spans {
        let end = start + span.len;
        let parts = [
            (start, end.min(lo), false),
            (start.max(lo), end.min(hi), true),
            (start.max(hi), end, false),
        ];
        for (from, to, inside) in parts {
            if from < to {
                let style = if inside { f(span.style) } else { span.style };
                out.push(Span {
                    len: to - from,
                    style,
                });
            }
        }
        start = end;
    }
    out
}

impl Paragraph {
    /// The style a character typed at `byte` gets: that of the character
    /// before it, or of the first one at the start.
    pub(crate) fn typing_style(&self, byte: usize) -> CharStyleId {
        self.style_at(byte.saturating_sub(1))
    }

    /// The number of objects anchored before `byte`.
    pub(crate) fn anchors_before(&self, byte: usize) -> usize {
        self.text[..byte].matches(OBJECT_CHAR).count()
    }

    /// The piece of the paragraph in `lo..hi`, in the same paragraph style. An
    /// empty piece carries the typing style at `lo`.
    pub(crate) fn slice(&self, lo: usize, hi: usize) -> Paragraph {
        let a = self.anchors_before(lo);
        let b = a + self.text[lo..hi].matches(OBJECT_CHAR).count();
        let spans = self.spans.iter().copied().scan(0, |start, span| {
            let from = (*start).max(lo);
            *start += span.len;
            let to = (*start).min(hi);
            Some(Span {
                len: to.saturating_sub(from),
                style: span.style,
            })
        });
        Paragraph {
            text: self.text[lo..hi].to_owned(),
            spans: normalize(spans, self.typing_style(lo)),
            anchors: self.anchors[a..b].to_vec(),
            style: self.style,
        }
    }

    /// The pieces joined into one paragraph in `style`; if the result is empty
    /// it keeps `fallback` as its typing style.
    pub(crate) fn concat(
        parts: &[&Paragraph],
        fallback: CharStyleId,
        style: ParaStyleId,
    ) -> Paragraph {
        let mut text = String::new();
        let mut anchors = Vec::new();
        for part in parts {
            text.push_str(&part.text);
            anchors.extend_from_slice(&part.anchors);
        }
        let spans = normalize(parts.iter().flat_map(|p| p.spans.iter().copied()), fallback);
        Paragraph {
            text,
            spans,
            anchors,
            style,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(n: u32) -> CharStyleId {
        CharStyleId(n)
    }

    #[test]
    fn normalize_merges_and_keeps_an_empty_span() {
        let spans = [
            Span {
                len: 2,
                style: style(1),
            },
            Span {
                len: 0,
                style: style(2),
            },
            Span {
                len: 3,
                style: style(1),
            },
        ];
        let out = normalize(spans, style(9));
        assert_eq!(
            out,
            vec![Span {
                len: 5,
                style: style(1)
            }]
        );
        assert_eq!(
            normalize([], style(9)),
            vec![Span {
                len: 0,
                style: style(9)
            }]
        );
    }

    #[test]
    fn slice_and_concat_are_inverse() {
        let mut p = Paragraph::new("héllo wörld", ParaStyleId::DEFAULT, style(0));
        p.spans = vec![
            Span {
                len: 3,
                style: style(1),
            },
            Span {
                len: p.text.len() - 3,
                style: style(0),
            },
        ];
        let (head, tail) = (p.slice(0, 7), p.slice(7, p.text.len()));
        let joined = Paragraph::concat(&[&head, &tail], style(0), p.style);
        assert_eq!(joined, p);
        assert!(p.slice(3, 3).text.is_empty());
        assert_eq!(p.slice(3, 3).spans[0].style, style(1));
    }
}
