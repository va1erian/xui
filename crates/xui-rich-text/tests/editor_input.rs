//! Keyboard input, clipboard, undo and the messages the app hears.

mod common;
mod rig;

use common::{DocBuilder, Run};
use rig::*;
use xui_core::backend::Event;
use xui_core::message::{Key, Modifiers, MouseButton};
use xui_rich_text::edit::Command;
use xui_rich_text::model::{CharStyleId, ListItem, ListKind, ParaStyle, ParaStyleId};
use xui_rich_text::{DocPos, Document};

#[test]
fn typing_enter_and_backspace() {
    run(Document::new(), |stage, rig| {
        typing(stage, "ab");
        key(stage, Key::RETURN);
        typing(stage, "cd");
        assert_eq!(rig.text(), "ab\ncd");
        key(stage, Key::BACK);
        assert_eq!(rig.text(), "ab\nc");
        key(stage, Key::BACK);
        key(stage, Key::BACK);
        assert_eq!(rig.text(), "ab", "backspace at a paragraph start merges");
    });
}

#[test]
fn control_characters_are_not_typed() {
    run(Document::new(), |stage, rig| {
        stage.inject(Event::Char('\u{2}'));
        stage.inject(Event::Char('\r'));
        typing(stage, "x");
        assert_eq!(rig.text(), "x");
    });
}

#[test]
fn ctrl_b_then_typing_makes_a_bold_run() {
    run(plain("plain "), |stage, rig| {
        key(stage, Key::END);
        ctrl(stage, Key::B);
        typing(stage, "bold");
        let (bold, text) = rig.editor.with_document(|d| {
            let p = &d.paragraphs()[0];
            let at = |byte| d.styles().char(p.style_at(byte)).weight.value();
            (at(8), p.text().to_owned())
        });
        assert_eq!(text, "plain bold");
        assert_eq!(bold, 700);
        let first = rig.editor.with_document(|d| {
            d.styles()
                .char(d.paragraphs()[0].style_at(1))
                .weight
                .value()
        });
        assert_eq!(first, 400, "the earlier text stays regular");
    });
}

#[test]
fn undo_and_redo() {
    run(Document::new(), |stage, rig| {
        typing(stage, "abc");
        ctrl(stage, Key::Z);
        assert_eq!(rig.text(), "", "typing a word is one undo step");
        ctrl(stage, Key::Y);
        assert_eq!(rig.text(), "abc");
        ctrl(stage, Key::Z);
        key_with(
            stage,
            Key::Z,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        );
        assert_eq!(rig.text(), "abc", "Ctrl+Shift+Z redoes");
    });
}

#[test]
fn shift_arrows_select_and_typing_replaces() {
    run(plain("hello"), |stage, rig| {
        key(stage, Key::HOME);
        shift(stage, Key::RIGHT);
        shift(stage, Key::RIGHT);
        let range = rig.range();
        assert_eq!((range.start.byte, range.end.byte), (0, 2));
        typing(stage, "J");
        assert_eq!(rig.text(), "Jllo");
    });
}

#[test]
fn select_all_then_delete() {
    run(plain("one"), |stage, rig| {
        ctrl(stage, Key::A);
        key(stage, Key::DELETE);
        assert_eq!(rig.text(), "");
    });
}

#[test]
fn cut_and_paste_through_the_clipboard() {
    run(plain("hello"), |stage, rig| {
        ctrl(stage, Key::A);
        ctrl(stage, Key::X);
        assert_eq!(rig.text(), "");
        ctrl(stage, Key::V);
        ctrl(stage, Key::V);
        assert_eq!(rig.text(), "hellohello");
    });
}

#[test]
fn tab_indents_in_a_list_and_types_a_tab_elsewhere() {
    let mut b = DocBuilder::new();
    let item = b.para_style(ParaStyle {
        list: Some(ListItem {
            kind: ListKind::Bullet,
            level: 0,
        }),
        ..ParaStyle::default()
    });
    b.paragraph(item, &[Run::Text("item", CharStyleId::DEFAULT)]);
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[Run::Text("plain", CharStyleId::DEFAULT)],
    );
    run(b.finish(), |stage, rig| {
        key(stage, Key::TAB);
        let level = rig.editor.with_document(|d| {
            d.styles()
                .para(d.paragraphs()[0].style())
                .list
                .map(|l| l.level)
        });
        assert_eq!(level, Some(1));
        key(stage, Key::DOWN);
        key(stage, Key::TAB);
        assert!(rig.text().contains("\tplain") || rig.text().contains("plain\t"));
    });
}

#[test]
fn the_app_hears_about_changes_and_formatting() {
    run(Document::new(), |stage, rig| {
        typing(stage, "a");
        {
            let log = rig.log.borrow();
            assert!(log.contains(&Msg::Changed("a".into())), "{log:?}");
        }
        rig.log.borrow_mut().clear();
        ctrl(stage, Key::B);
        assert_eq!(*rig.log.borrow(), [Msg::Bold(true)]);
        rig.log.borrow_mut().clear();
        rig.editor.exec(Command::SelectAll);
        assert!(
            !rig.log
                .borrow()
                .iter()
                .any(|m| matches!(m, Msg::Changed(_)))
        );
    });
}

#[test]
fn a_ctrl_click_on_a_link_reports_it() {
    let mut b = DocBuilder::new();
    let link = b.char_style(xui_rich_text::model::CharStyle {
        link: Some("https://example.com".into()),
        ..Default::default()
    });
    b.paragraph(
        ParaStyleId::DEFAULT,
        &[
            Run::Text("see ", CharStyleId::DEFAULT),
            Run::Text("here", link),
        ],
    );
    run(b.finish(), |stage, rig| {
        let r = rig.editor.caret_rect(DocPos::new(0, 6), Default::default());
        let y = (r.top + r.bottom) / 2;
        mouse(stage, "down", r.left, y);
        mouse(stage, "up", r.left, y);
        assert!(!rig.log.borrow().iter().any(|m| matches!(m, Msg::Link(_))));
        stage.inject(Event::MouseDown {
            x: r.left,
            y,
            button: MouseButton::Left,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        });
        assert!(
            rig.log
                .borrow()
                .contains(&Msg::Link("https://example.com".into()))
        );
    });
}
