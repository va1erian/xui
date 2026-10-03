#![forbid(unsafe_code)]

//! Character and paragraph formatting, lists, indentation and images.

use std::ops::RangeInclusive;

use xui_core::Dip;

use super::controller::{EditorState, Effect};
use crate::model::{
    CharStylePatch, DocPos, EditOp, InlineImage, ListItem, ListKind, ParaStylePatch, Selection, Tri,
};

/// How far one Indent moves a paragraph that is not in a list.
const INDENT_STEP: Dip = Dip(24.0);
/// The deepest list level Indent reaches.
const MAX_LEVEL: u8 = 8;

/// A boolean character attribute a toggle command flips.
#[derive(Clone, Copy)]
pub(super) enum Attr {
    Bold,
    Italic,
    Underline,
    Strike,
}

impl Attr {
    fn patch(self, on: bool) -> CharStylePatch {
        match self {
            Attr::Bold => CharStylePatch::bold(on),
            Attr::Italic => CharStylePatch::italic(on),
            Attr::Underline => CharStylePatch::underline(on),
            Attr::Strike => CharStylePatch::strike(on),
        }
    }

    fn pending(self, patch: &CharStylePatch) -> Option<bool> {
        match self {
            Attr::Bold => patch.bold,
            Attr::Italic => patch.italic,
            Attr::Underline => patch.underline,
            Attr::Strike => patch.strike,
        }
    }
}

/// Lays `other`'s set attributes over `into`.
fn overlay(into: &mut CharStylePatch, other: CharStylePatch) {
    macro_rules! take {
        ($($field:ident),*) => {$(
            if other.$field.is_some() {
                into.$field = other.$field;
            }
        )*};
    }
    take!(
        bold, italic, underline, strike, color, highlight, link, size, family, baseline
    );
}

impl EditorState {
    /// The paragraphs the selection touches; a range ending at the very start
    /// of a paragraph does not touch it.
    fn selected_paras(&self) -> RangeInclusive<usize> {
        let Some(range) = self.selection_range() else {
            return 0..=0;
        };
        let last = if range.end.byte == 0 && range.end.para > range.start.para {
            range.end.para - 1
        } else {
            range.end.para
        };
        range.start.para..=last
    }

    fn pending_effect(&mut self, changed: bool) -> Effect {
        Effect {
            selection: changed,
            ..Effect::NONE
        }
    }

    /// Bold, italic, underline and strike-through toggles.
    pub(super) fn toggle(&mut self, attr: Attr) -> Effect {
        let summary = self.doc.style_summary(&self.selection);
        let current = match attr {
            Attr::Bold => summary.bold,
            Attr::Italic => summary.italic,
            Attr::Underline => summary.underline,
            Attr::Strike => summary.strike,
        };
        let on = matches!(current, Tri::Uniform(true));
        if self.selection.is_collapsed() {
            let pending = self.pending.get_or_insert_with(CharStylePatch::default);
            let now_on = attr.pending(pending).unwrap_or(on);
            overlay(pending, attr.patch(!now_on));
            return self.pending_effect(true);
        }
        self.set_char_style(attr.patch(!on))
    }

    /// Restyles the selection, or the next typed text at a caret.
    pub(super) fn set_char_style(&mut self, patch: CharStylePatch) -> Effect {
        if self.selection.is_collapsed() {
            overlay(
                self.pending.get_or_insert_with(CharStylePatch::default),
                patch,
            );
            return self.pending_effect(true);
        }
        let Some(range) = self.selection_range() else {
            return Effect::NONE;
        };
        self.run(|cx| {
            cx.apply(EditOp::SetCharStyle { range, patch })?;
            Ok(None)
        })
    }

    /// Applies a paragraph patch to every selected paragraph.
    pub(super) fn set_para_style(&mut self, patch: ParaStylePatch) -> Effect {
        let paras = self.selected_paras();
        self.run(|cx| {
            cx.apply(EditOp::SetParaStyle {
                paras: *paras.start()..*paras.end() + 1,
                patch,
            })?;
            Ok(None)
        })
    }

    /// Makes the selected paragraphs list items, or takes them out of the list.
    pub(super) fn toggle_list(&mut self, kind: ListKind) -> Effect {
        let paras = self.selected_paras();
        let levels: Vec<Option<ListItem>> = paras
            .clone()
            .map(|p| {
                self.doc
                    .styles()
                    .para(self.doc.paragraphs()[p].style())
                    .list
            })
            .collect();
        let all = levels.iter().all(|l| l.is_some_and(|l| l.kind == kind));
        self.run(|cx| {
            for (para, old) in paras.zip(levels) {
                let list = (!all).then(|| ListItem {
                    kind,
                    level: old.map_or(0, |l| l.level),
                });
                cx.apply(EditOp::SetParaStyle {
                    paras: para..para + 1,
                    patch: ParaStylePatch::list(list),
                })?;
            }
            Ok(None)
        })
    }

    /// Indent and Outdent: list level inside lists, left indent elsewhere.
    pub(super) fn shift_indent(&mut self, deeper: bool) -> Effect {
        let paras = self.selected_paras();
        let current: Vec<_> = paras
            .clone()
            .map(|p| {
                self.doc
                    .styles()
                    .para(self.doc.paragraphs()[p].style())
                    .clone()
            })
            .collect();
        self.run(|cx| {
            for (para, style) in paras.zip(current) {
                let patch = match style.list {
                    Some(item) => {
                        let level = if deeper {
                            (item.level + 1).min(MAX_LEVEL)
                        } else {
                            item.level.saturating_sub(1)
                        };
                        ParaStylePatch::list(Some(ListItem { level, ..item }))
                    }
                    None => {
                        let step = if deeper {
                            INDENT_STEP.0
                        } else {
                            -INDENT_STEP.0
                        };
                        ParaStylePatch {
                            indent_left: Some(Dip((style.indent_left.0 + step).max(0.0))),
                            ..ParaStylePatch::default()
                        }
                    }
                };
                cx.apply(EditOp::SetParaStyle {
                    paras: para..para + 1,
                    patch,
                })?;
            }
            Ok(None)
        })
    }

    /// Replaces the selection with an image and selects it.
    pub(super) fn insert_image(&mut self, object: InlineImage) -> Effect {
        let Some(range) = self.selection_range() else {
            return Effect::NONE;
        };
        let at = range.start;
        self.run(|cx| {
            if !range.is_empty() {
                cx.apply(EditOp::Delete { range })?;
            }
            cx.apply(EditOp::InsertObject { at, object })?;
            let id = cx.doc().paragraphs()[at.para]
                .objects()
                .find(|&(byte, _)| byte == at.byte)
                .map(|(_, id)| id);
            Ok(Some(id.map_or(
                Selection::caret(DocPos::new(at.para, at.byte)),
                Selection::Object,
            )))
        })
    }
}
