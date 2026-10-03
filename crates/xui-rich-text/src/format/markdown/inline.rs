#![forbid(unsafe_code)]

//! The inline half of the Markdown writer: a paragraph's characters become
//! [`Unit`]s, emphasis is trimmed and then emitted with correctly nested
//! markers, and links wrap runs of units.

use super::ImageExport;
use crate::format::base64;
use crate::model::{Document, OBJECT_CHAR, Paragraph};

/// The markers of the three emphasis kinds, in the order of [`Unit::marks`]:
/// bold, italic, strike-through.
const DELIMITERS: [&str; 3] = ["**", "*", "~~"];
/// The same, for a marker that would otherwise touch a `*`.
const UNDERSCORES: [&str; 3] = ["__", "_", "~~"];

/// A bold weight, as far as Markdown is concerned.
const BOLD_WEIGHT: u16 = 600;

/// One character (or image, or line break) with the formatting Markdown keeps.
pub(super) struct Unit<'a> {
    /// The Markdown for this unit, already escaped.
    text: String,
    /// Whether it is whitespace, which emphasis markers must not touch.
    blank: bool,
    /// Bold, italic and strike-through.
    marks: [bool; 3],
    link: Option<&'a str>,
}

/// Characters that always need a backslash.
const ESCAPED: &str = "\\`*_[]<>~|&";

/// Tracks where a line starts, where `#`, `-` and `1.` mean something.
#[derive(Default)]
struct Line {
    /// Characters emitted since the line started.
    chars: usize,
    /// How many of them are leading digits.
    digits: usize,
}

impl Line {
    fn escape(&mut self, c: char) -> String {
        let escaped = ESCAPED.contains(c)
            || (self.chars == 0 && matches!(c, '#' | '+' | '-' | '='))
            || (self.digits > 0 && self.digits == self.chars && matches!(c, '.' | ')'));
        if c.is_ascii_digit() && self.digits == self.chars {
            self.digits += 1;
        }
        self.chars += 1;
        if escaped {
            format!("\\{c}")
        } else {
            c.to_string()
        }
    }

    /// Notes a non-text unit (an image) on the line.
    fn skip(&mut self) {
        self.chars += 1;
    }

    fn reset(&mut self) {
        *self = Line::default();
    }
}

/// Escapes `text` for use inside a link label or image alt text.
fn escape_label(text: &str) -> String {
    text.chars()
        .map(|c| {
            if ESCAPED.contains(c) {
                format!("\\{c}")
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// A link or image target in Markdown syntax.
fn destination(target: &str) -> String {
    let plain = !target.is_empty()
        && !target
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '(' | ')' | '<' | '>'));
    let encoded: String = target
        .chars()
        .map(|c| match c {
            '\\' => "%5C".to_owned(),
            '<' => "%3C".to_owned(),
            '>' => "%3E".to_owned(),
            '\n' | '\r' => String::new(),
            c => c.to_string(),
        })
        .collect();
    if plain {
        encoded
    } else {
        format!("<{encoded}>")
    }
}

fn image_markdown(doc: &Document, id: crate::model::ObjectId, images: &ImageExport) -> String {
    let Some(object) = doc.objects().get(id) else {
        return String::new();
    };
    let target = match images {
        ImageExport::DataUri => object
            .image
            .encode_png()
            .map(|png| format!("data:image/png;base64,{}", base64::encode(&png))),
        ImageExport::Callback(callback) => Ok(callback(&object.image, &object.alt)),
    };
    match target {
        Ok(target) => format!("![{}]({})", escape_label(&object.alt), destination(&target)),
        Err(_) => escape_label(&object.alt),
    }
}

/// The units of `para`. With `flat`, line breaks become spaces (headings).
pub(super) fn units<'a>(
    doc: &'a Document,
    para: &'a Paragraph,
    images: &ImageExport,
    flat: bool,
) -> Vec<Unit<'a>> {
    let mut out = Vec::new();
    let mut line = Line::default();
    let mut anchors = para.anchors().iter();
    for (range, id) in para.runs() {
        let style = doc.styles().char(id);
        let marks = [
            style.weight.value() >= BOLD_WEIGHT,
            style.italic,
            style.strike,
        ];
        let link = style.link.as_deref();
        for c in para.text()[range].chars() {
            let (text, blank) = match c {
                OBJECT_CHAR => {
                    line.skip();
                    let text = anchors
                        .next()
                        .map(|&id| image_markdown(doc, id, images))
                        .unwrap_or_default();
                    (text, false)
                }
                '\u{2028}' | '\n' if flat => {
                    line.skip();
                    (" ".to_owned(), true)
                }
                '\u{2028}' | '\n' => {
                    line.reset();
                    ("\\\n".to_owned(), true)
                }
                c => (line.escape(c), c.is_whitespace()),
            };
            out.push(Unit {
                text,
                blank,
                marks,
                link,
            });
        }
    }
    out
}

/// Clears an emphasis mark on the blank units at the edges of each of its
/// runs, so the marker never touches whitespace (`** a**` is not bold).
fn trim_marks(units: &mut [Unit<'_>]) {
    for mark in 0..DELIMITERS.len() {
        let mut i = 0;
        while i < units.len() {
            if !units[i].marks[mark] {
                i += 1;
                continue;
            }
            let end = (i..units.len())
                .find(|&j| !units[j].marks[mark])
                .unwrap_or(units.len());
            let (mut lo, mut hi) = (i, end);
            while lo < hi && units[lo].blank {
                units[lo].marks[mark] = false;
                lo += 1;
            }
            while hi > lo && units[hi - 1].blank {
                units[hi - 1].marks[mark] = false;
                hi -= 1;
            }
            i = end;
        }
    }
}

/// Emits `units` (one link group) with emphasis markers properly nested: runs
/// that last longest open outermost, and a marker that must close while an
/// inner one stays open closes it first and reopens it.
fn emit_emphasis(units: &[Unit<'_>], out: &mut String) {
    // run_end[i][m]: where the run of mark m containing unit i ends.
    let mut run_end = vec![[0usize; 3]; units.len() + 1];
    for i in (0..units.len()).rev() {
        let next = run_end[i + 1];
        run_end[i] = std::array::from_fn(|m| match (units[i].marks[m], next[m]) {
            (false, _) => 0,
            (true, 0) => i + 1,
            (true, end) => end,
        });
    }
    let mut open: Vec<(usize, &str)> = Vec::new();
    for (i, unit) in units.iter().enumerate() {
        if let Some(level) = open.iter().position(|&(m, _)| !unit.marks[m]) {
            while open.len() > level {
                let (_, delimiter) = open.pop().expect("level is within the stack");
                out.push_str(delimiter);
            }
        }
        let mut fresh: Vec<usize> = (0..3)
            .filter(|&m| unit.marks[m] && !open.iter().any(|&(o, _)| o == m))
            .collect();
        fresh.sort_by_key(|&m| std::cmp::Reverse(run_end[i][m]));
        // A marker right after `*` would fuse with it into a run the parser
        // cannot split, so it switches to the underscore form.
        let after_star = out.ends_with('*');
        for m in fresh {
            let delimiter = if after_star {
                UNDERSCORES[m]
            } else {
                DELIMITERS[m]
            };
            out.push_str(delimiter);
            open.push((m, delimiter));
        }
        out.push_str(&unit.text);
    }
    while let Some((_, delimiter)) = open.pop() {
        out.push_str(delimiter);
    }
}

/// The Markdown for a paragraph's units.
pub(super) fn render(mut units: Vec<Unit<'_>>) -> String {
    let mut out = String::new();
    let mut start = 0;
    while start < units.len() {
        let link = units[start].link;
        let end = (start..units.len())
            .find(|&j| units[j].link != link)
            .unwrap_or(units.len());
        let group = &mut units[start..end];
        trim_marks(group);
        match link {
            Some(url) => {
                out.push('[');
                emit_emphasis(group, &mut out);
                out.push_str("](");
                out.push_str(&destination(url));
                out.push(')');
            }
            None => emit_emphasis(group, &mut out),
        }
        start = end;
    }
    out
}
