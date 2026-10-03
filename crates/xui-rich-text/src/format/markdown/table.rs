#![forbid(unsafe_code)]

//! Tables as GFM pipe tables. GFM needs a header row, so the first row is
//! always written as one. A cell's paragraphs and line breaks become `<br>`,
//! and list items keep their marker as text.

use super::{ImageExport, inline};
use crate::model::{Document, ListKind, TableSpan};

/// The pipe table for `span`, each line ending in a newline.
pub(super) fn table(doc: &Document, span: &TableSpan, images: &ImageExport) -> String {
    let mut out = String::new();
    for (r, cells) in span.rows.iter().enumerate() {
        out.push('|');
        for cell in cells {
            out.push(' ');
            out.push_str(&cell_text(doc, cell.clone(), images));
            out.push_str(" |");
        }
        out.push('\n');
        if r == 0 {
            out.push('|');
            out.push_str(&" --- |".repeat(cells.len()));
            out.push('\n');
        }
    }
    out
}

/// The Markdown of one cell's paragraphs, on one line.
fn cell_text(doc: &Document, paras: std::ops::Range<usize>, images: &ImageExport) -> String {
    let mut parts = Vec::new();
    let mut number = 0;
    for para in &doc.paragraphs()[paras] {
        let body = inline::render(inline::units(doc, para, images, false));
        let body = body.replace("\\\n", "<br>").replace('\n', "<br>");
        let marker = match doc.styles().para(para.style()).list {
            Some(item) if item.kind == ListKind::Numbered => {
                number += 1;
                format!("{number}. ")
            }
            Some(_) => "\u{2022} ".to_owned(),
            None => {
                number = 0;
                String::new()
            }
        };
        parts.push(format!("{marker}{body}"));
    }
    while parts.last().is_some_and(String::is_empty) && parts.len() > 1 {
        parts.pop();
    }
    parts.join("<br>")
}
