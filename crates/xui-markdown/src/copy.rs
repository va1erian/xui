#![forbid(unsafe_code)]

//! A selection as plain text, for the clipboard.
//!
//! The text keeps what reads well when pasted anywhere: a blank line between
//! paragraphs, one line per list item with its marker, code as written, and a
//! table as tab-separated rows (which a spreadsheet splits into cells).

use std::ops::Range;

use crate::model::{Block, BlockKind, Document, Marker, OBJECT, range};

impl Document {
    /// The text in `selection` (byte offsets into [`Document::text`]) as
    /// plain text. A list item's marker is included when its start is and
    /// the selection reaches past that one item; text from inside a single
    /// item (a double- or triple-clicked word or item) copies without it.
    pub fn plain_text(&self, selection: Range<u32>) -> String {
        let mut out = String::new();
        if selection.is_empty() {
            return out;
        }
        let first = self
            .blocks
            .partition_point(|b| b.text.end < selection.start);
        let mut previous: Option<&Block> = None;
        for block in &self.blocks[first..] {
            if block.text.start > selection.end {
                break;
            }
            let from = selection.start.max(block.text.start);
            let to = selection.end.min(block.text.end);
            // A block that only touches an end of the selection is not in it,
            // unless it has no text (a rule, an empty item) and lies inside.
            let inside = block.text.start > selection.start && block.text.start < selection.end;
            if from >= to && !(block.text.is_empty() && inside) {
                continue;
            }
            if let Some(previous) = previous {
                out.push_str(separator(previous, block));
            }
            let spans_blocks = selection.start < block.text.start || selection.end > block.text.end;
            if from == block.text.start && spans_blocks {
                marker(&mut out, block);
            }
            match &block.kind {
                BlockKind::TableRow { .. } => self.row(&mut out, block, from..to),
                _ => push_text(&mut out, &self.text[range(&(from..to))]),
            }
            previous = Some(block);
        }
        out
    }

    /// The cells of a table row within `within`, separated by tabs. Only
    /// cells in the selection count, so text from one cell has no tabs.
    fn row(&self, out: &mut String, block: &Block, within: Range<u32>) {
        let mut first = true;
        // A whole row keeps its empty cells, so columns stay aligned; in a
        // partly selected row an empty cell counts only strictly inside.
        let whole = within.start <= block.text.start && within.end >= block.text.end;
        for cell in self.cells(block) {
            let from = cell.start.max(within.start);
            let to = cell.end.min(within.end);
            let empty_inside = cell.is_empty()
                && (whole || (within.start < cell.start && cell.start < within.end));
            if from >= to && !empty_inside {
                continue;
            }
            if !first {
                out.push('\t');
            }
            first = false;
            push_text(out, &self.text[range(&(from..to))]);
        }
    }
}

/// What goes between two blocks: a line break inside a list or a table, a
/// blank line elsewhere.
fn separator(previous: &Block, block: &Block) -> &'static str {
    let same_table = matches!(
        (&previous.kind, &block.kind),
        (BlockKind::TableRow { table: a, .. }, BlockKind::TableRow { table: b, .. }) if a == b
    );
    let in_list = previous.list > 0 && matches!(block.kind, BlockKind::ListItem(_));
    if same_table || in_list { "\n" } else { "\n\n" }
}

/// Appends `text` without the [`OBJECT`]s standing for images with no alt
/// text.
fn push_text(out: &mut String, text: &str) {
    if text.contains(OBJECT) {
        out.extend(text.chars().filter(|&c| c != OBJECT));
    } else {
        out.push_str(text);
    }
}

/// A list item's marker, indented by its depth, as Markdown writes it.
fn marker(out: &mut String, block: &Block) {
    let BlockKind::ListItem(marker) = block.kind else {
        return;
    };
    for _ in 1..block.list {
        out.push_str("  ");
    }
    match marker {
        Marker::Bullet => out.push_str("- "),
        Marker::Number(n) => {
            out.push_str(&n.to_string());
            out.push_str(". ");
        }
        Marker::Task(true) => out.push_str("- [x] "),
        Marker::Task(false) => out.push_str("- [ ] "),
    }
}

#[cfg(test)]
mod tests {
    use crate::Document;

    fn all(markdown: &str) -> String {
        let doc = Document::parse(markdown);
        doc.plain_text(0..doc.text().len() as u32)
    }

    #[test]
    fn paragraphs_are_a_blank_line_apart_and_styles_drop_their_markers() {
        assert_eq!(
            all("# Title\n\nSome **bold** text.\n\nNext."),
            "Title\n\nSome bold text.\n\nNext."
        );
    }

    #[test]
    fn list_items_keep_their_markers_one_per_line() {
        assert_eq!(
            all("- one\n- two\n  1. nested\n- [x] done\n\nAfter"),
            "- one\n- two\n  1. nested\n- [x] done\n\nAfter"
        );
    }

    #[test]
    fn code_is_verbatim_and_tables_are_tab_separated() {
        assert_eq!(all("```\nfn a() {\n\tb();\n}\n```"), "fn a() {\n\tb();\n}");
        assert_eq!(all("| a | b |\n|---|---|\n| 1 | 2 |"), "a\tb\n1\t2");
    }

    #[test]
    fn a_partial_selection_takes_only_its_text_and_no_marker() {
        let doc = Document::parse("- alpha beta\n- gamma");
        let text = doc.text();
        let from = text.find("beta").unwrap() as u32;
        let to = text.find("gam").unwrap() as u32 + 3;
        assert_eq!(doc.plain_text(from..to), "beta\n- gam");
        assert_eq!(doc.plain_text(from..from), "");
    }

    #[test]
    fn text_from_inside_one_item_has_no_marker() {
        let doc = Document::parse(
            "- one
- two words",
        );
        let item = doc.blocks()[1].text.clone();
        assert_eq!(doc.plain_text(item.clone()), "two words");
        assert_eq!(doc.plain_text(item.start..item.start + 3), "two");
        assert_eq!(
            doc.plain_text(0..item.end),
            "- one
- two words"
        );
    }

    #[test]
    fn an_empty_cell_beside_the_selection_adds_no_tab() {
        let doc = Document::parse(
            "| a |  | b |
|---|---|---|
| 1 | 2 | 3 |",
        );
        let a = doc.text().find('a').unwrap() as u32;
        assert_eq!(doc.plain_text(a..a + 1), "a");
        let b = doc.text().find('b').unwrap() as u32;
        assert_eq!(doc.plain_text(a..b + 1), "a		b");
    }

    #[test]
    fn a_whole_table_keeps_its_empty_first_and_last_cells() {
        assert_eq!(
            all("| h1 | h2 | h3 |
|---|---|---|
|  | b | c |
| d | e |  |"),
            "h1	h2	h3
	b	c
d	e	"
        );
    }

    #[test]
    fn a_block_only_touching_the_selection_adds_no_separator() {
        let doc = Document::parse(
            "First.

Second.",
        );
        let second = doc.blocks()[1].text.clone();
        assert_eq!(doc.plain_text(second.start..second.end), "Second.");
        let first = doc.blocks()[0].text.clone();
        assert_eq!(doc.plain_text(first.start..first.end), "First.");
    }

    #[test]
    fn a_selection_inside_one_cell_has_no_tabs() {
        let doc = Document::parse("| left | right |\n|---|---|\n| x | y |");
        let at = doc.text().find("right").unwrap() as u32;
        assert_eq!(doc.plain_text(at..at + 3), "rig");
    }
}
