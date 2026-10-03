//! A harness for the edit controller tests (included with `#[path]`).
#![allow(dead_code)]

use std::time::Duration;

use xui_rich_text::DocPos;
use xui_rich_text::edit::{Command, EditorState, Effect, LineNav, MemoryClipboard, Motion};
use xui_rich_text::model::{Document, ListItem, ListKind, Selection};

/// Every paragraph is one line; x is the byte offset; a page is 3 lines.
pub struct OneLinePerParagraph(pub Vec<String>);

impl LineNav for OneLinePerParagraph {
    fn line_start(&self, pos: DocPos) -> DocPos {
        DocPos::new(pos.para, 0)
    }

    fn line_end(&self, pos: DocPos) -> DocPos {
        DocPos::new(pos.para, self.0[pos.para].len())
    }

    fn vertical(&self, pos: DocPos, sticky_x: Option<f32>, lines: i32) -> (DocPos, f32) {
        let x = sticky_x.unwrap_or(pos.byte as f32);
        let para = (pos.para as i64 + i64::from(lines)).clamp(0, self.0.len() as i64 - 1) as usize;
        let text = &self.0[para];
        let mut byte = (x as usize).min(text.len());
        while !text.is_char_boundary(byte) {
            byte -= 1;
        }
        (DocPos::new(para, byte), x)
    }

    fn page_lines(&self) -> i32 {
        3
    }
}

/// An editor with a clock and a clipboard.
pub struct Ed {
    pub state: EditorState,
    pub clipboard: MemoryClipboard,
    pub now: Duration,
}

impl Ed {
    pub fn new(text: &str) -> Ed {
        Ed::from_doc(Document::from_plain_text(text))
    }

    pub fn from_doc(doc: Document) -> Ed {
        Ed {
            state: EditorState::new(doc),
            clipboard: MemoryClipboard::default(),
            now: Duration::ZERO,
        }
    }

    /// Runs a command 50 ms after the previous one.
    pub fn run(&mut self, command: Command) -> Effect {
        self.now += Duration::from_millis(50);
        let nav = OneLinePerParagraph(
            self.state
                .doc
                .paragraphs()
                .iter()
                .map(|p| p.text().to_owned())
                .collect(),
        );
        let effect = self.state.exec(command, &nav, self.now, &self.clipboard);
        self.state.doc.check().unwrap();
        assert!(
            self.state.selection_is_valid(),
            "{:?}",
            self.state.selection
        );
        effect
    }

    pub fn type_str(&mut self, text: &str) {
        for ch in text.chars() {
            self.run(Command::InsertText(ch.to_string()));
        }
    }

    pub fn mv(&mut self, motion: Motion) -> Effect {
        self.run(Command::Move {
            motion,
            extend: false,
        })
    }

    pub fn select(&mut self, a: (usize, usize), b: (usize, usize)) {
        self.state.selection = Selection::text(DocPos::new(a.0, a.1), DocPos::new(b.0, b.1));
    }

    pub fn caret_at(&mut self, para: usize, byte: usize) {
        self.state.selection = Selection::caret(DocPos::new(para, byte));
    }

    pub fn text(&self) -> String {
        self.state.doc.to_plain_text()
    }

    pub fn caret(&self) -> DocPos {
        self.state.selection.head().expect("a text selection")
    }
}

/// Undoes the last step, expecting there to be one.
pub fn undo(ed: &mut Ed) {
    assert!(ed.run(Command::Undo).doc_changed());
}

/// The list membership of paragraph `para`.
pub fn list_of(ed: &Ed, para: usize) -> Option<ListItem> {
    let doc = &ed.state.doc;
    doc.styles().para(doc.paragraphs()[para].style()).list
}

/// Puts the caret at the start of `para` and toggles a bullet list.
pub fn bullet(ed: &mut Ed, para: usize) {
    ed.caret_at(para, 0);
    ed.run(Command::ToggleList(ListKind::Bullet));
}
