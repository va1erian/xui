#![forbid(unsafe_code)]

//! What a toolbar shows for a selection: each attribute as one value or mixed.

use xui_core::Dip;

use super::selection::{DocRange, Selection};
use super::style::{
    Align, BlockKind, CharStyle, CharStyleId, ListItem, ParaStyle, ParaStyleId, TextColor,
};
use super::{DocPos, Document};

/// An attribute over a selection: the same everywhere, or varying.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tri<T> {
    /// Every selected character or paragraph has this value.
    Uniform(T),
    /// The values differ.
    Mixed,
}

impl<T: PartialEq> Tri<T> {
    fn of<I: Iterator<Item = T>>(mut values: I) -> Option<Tri<T>> {
        let first = values.next()?;
        Some(if values.all(|v| v == first) {
            Tri::Uniform(first)
        } else {
            Tri::Mixed
        })
    }
}

/// The formatting of a selection, as a toolbar needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleSummary {
    /// Bold (weight 600 or more).
    pub bold: Tri<bool>,
    /// Italic.
    pub italic: Tri<bool>,
    /// Underline.
    pub underline: Tri<bool>,
    /// Strike-through.
    pub strike: Tri<bool>,
    /// Font size.
    pub size: Tri<Dip>,
    /// Font family; `None` is the default font.
    pub family: Tri<Option<String>>,
    /// Text colour.
    pub color: Tri<TextColor>,
    /// Paragraph alignment.
    pub align: Tri<Align>,
    /// List membership.
    pub list: Tri<Option<ListItem>>,
    /// Structural role.
    pub kind: Tri<BlockKind>,
}

impl StyleSummary {
    fn new(chars: &[&CharStyle], paras: &[&ParaStyle]) -> StyleSummary {
        let c = |f: fn(&CharStyle) -> bool| Tri::of(chars.iter().map(|s| f(s))).unwrap();
        StyleSummary {
            bold: c(|s| s.weight.value() >= 600),
            italic: c(|s| s.italic),
            underline: c(|s| s.underline),
            strike: c(|s| s.strike),
            size: Tri::of(chars.iter().map(|s| s.size)).unwrap(),
            family: Tri::of(chars.iter().map(|s| s.family.clone())).unwrap(),
            color: Tri::of(chars.iter().map(|s| s.color)).unwrap(),
            align: Tri::of(paras.iter().map(|s| s.align)).unwrap(),
            list: Tri::of(paras.iter().map(|s| s.list)).unwrap(),
            kind: Tri::of(paras.iter().map(|s| s.kind)).unwrap(),
        }
    }
}

impl Document {
    /// The formatting of `sel`. For a caret it is the style a typed character
    /// would get; for an image, the style of its anchor and paragraph.
    pub fn style_summary(&self, sel: &Selection) -> StyleSummary {
        let valid = self
            .selection_range(sel)
            .filter(|&range| self.check_range(range).is_ok());
        let (chars, paras) = match (sel, valid) {
            (Selection::Object(_), Some(range)) => (
                vec![self.paragraphs[range.start.para].style_at(range.start.byte)],
                vec![self.paragraphs[range.start.para].style],
            ),
            (_, Some(range)) => self.styles_in(range),
            (_, None) => self.styles_in(DocRange::new(DocPos::default(), DocPos::default())),
        };
        let chars: Vec<&CharStyle> = chars.iter().map(|&id| self.styles.char(id)).collect();
        let paras: Vec<&ParaStyle> = paras.iter().map(|&id| self.styles.para(id)).collect();
        StyleSummary::new(&chars, &paras)
    }

    /// The distinct character and paragraph styles a text range touches.
    fn styles_in(&self, range: DocRange) -> (Vec<CharStyleId>, Vec<ParaStyleId>) {
        let mut chars = Vec::new();
        let mut paras = Vec::new();
        let note = |chars: &mut Vec<CharStyleId>, id: CharStyleId| {
            if !chars.contains(&id) {
                chars.push(id);
            }
        };
        if range.is_empty() {
            note(&mut chars, self.typing_style(range.start));
            paras.push(self.paragraphs[range.start.para].style);
            return (chars, paras);
        }
        for (index, lo, hi) in self.covered(range) {
            let para = &self.paragraphs[index];
            for (run, id) in para.runs() {
                if (run.start < hi && run.end > lo) || para.text.is_empty() {
                    note(&mut chars, id);
                }
            }
        }
        if chars.is_empty() {
            note(&mut chars, self.typing_style(range.start));
        }
        let last = if range.end.byte == 0 && range.end.para > range.start.para {
            range.end.para - 1
        } else {
            range.end.para
        };
        for para in &self.paragraphs[range.start.para..=last] {
            if !paras.contains(&para.style) {
                paras.push(para.style);
            }
        }
        (chars, paras)
    }
}
