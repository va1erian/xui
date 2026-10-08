//! The completion popup driven through the event mapper, the way a window
//! would: key presses, typed characters and clicks against a live offscreen
//! `Ui`.

use super::*;
use crate::completion::{Completer, Completion, CompletionItem, CompletionKind};
use crate::popup::Layout;
use crate::text::is_word_char;
use xui_core::message::Key;

/// A completer that offers a fixed word list for the identifier before the
/// caret, counting chars the way the editor does.
fn words(list: &'static [&'static str]) -> impl Completer {
    move |text: &str, caret: usize| {
        let chars: Vec<char> = text.chars().collect();
        let mut start = caret;
        while start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        }
        let items = list
            .iter()
            .map(|word| CompletionItem::new(*word, CompletionKind::Function).with_detail("fn"))
            .collect();
        Some(Completion { start, items })
    }
}

const WORDS: &[&str] = &["print", "println", "parse_int", "Point", "let", "lerp"];

fn completing(text: &str) -> EditorState {
    let mut state = state(text);
    state.completer = Some(Rc::new(words(WORDS)));
    state
}

/// Focuses `state` and puts the caret at the end of the text.
fn focus_end(state: &mut EditorState, ui: &Ui<()>, id: xui_core::backend::WidgetId) {
    handle(state, ui, id, &Event::SetFocus);
    handle(state, ui, id, &ctrl(Key::END));
}

fn type_str(state: &mut EditorState, ui: &Ui<()>, id: xui_core::backend::WidgetId, text: &str) {
    for character in text.chars() {
        handle(state, ui, id, &Event::Char(character));
    }
}

fn labels(state: &EditorState) -> Vec<String> {
    let popup = state.completion.as_ref().expect("a popup is open");
    (0..popup.len())
        .map(|row| popup.item(row).expect("row").label.clone())
        .collect()
}

fn shift(key: Key) -> Event {
    Event::KeyDown {
        key,
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::NONE
        },
        repeat: 1,
        system: false,
    }
}

#[test]
fn ctrl_space_opens_the_popup() {
    with_ui(|ui, id| {
        let mut state = completing("pr");
        focus_end(&mut state, ui, id);
        assert!(state.completion.is_none());
        let outcome = handle(&mut state, ui, id, &ctrl(Key::SPACE)).expect("handled");
        assert!(!outcome.changed);
        assert_eq!(state.buffer.text(), "pr", "no space was typed");
        assert_eq!(labels(&state), ["print", "println", "parse_int"]);
    });
}

#[test]
fn ctrl_space_without_a_completer_is_not_handled() {
    with_ui(|ui, id| {
        let mut state = state("pr");
        focus_end(&mut state, ui, id);
        assert!(handle(&mut state, ui, id, &ctrl(Key::SPACE)).is_none());
        assert!(state.completion.is_none());
    });
}

#[test]
fn a_completer_with_nothing_to_offer_opens_nothing() {
    with_ui(|ui, id| {
        let mut state = state("pr");
        state.completer = Some(Rc::new(|_: &str, _: usize| None::<Completion>));
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE)).expect("handled");
        assert!(state.completion.is_none());
        type_str(&mut state, ui, id, "o");
        assert!(state.completion.is_none(), "auto-trigger asks too");
    });
}

#[test]
fn ctrl_space_with_nothing_matching_the_word_opens_nothing() {
    with_ui(|ui, id| {
        let mut state = completing("zzz");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE)).expect("handled");
        assert!(state.completion.is_none());
    });
}

#[test]
fn ctrl_space_after_a_non_word_char_offers_everything() {
    with_ui(|ui, id| {
        let mut state = completing("x = ");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE)).expect("handled");
        assert_eq!(labels(&state).len(), WORDS.len());
    });
}

#[test]
fn typing_opens_the_popup_from_the_second_identifier_char() {
    with_ui(|ui, id| {
        let mut state = completing("");
        focus_end(&mut state, ui, id);
        type_str(&mut state, ui, id, "p");
        assert!(state.completion.is_none(), "one char is not enough");
        type_str(&mut state, ui, id, "r");
        assert_eq!(labels(&state), ["print", "println", "parse_int"]);
    });
}

#[test]
fn the_word_length_counts_chars_before_the_caret_not_the_line() {
    with_ui(|ui, id| {
        let mut state = completing("p ");
        focus_end(&mut state, ui, id);
        type_str(&mut state, ui, id, "r");
        assert!(state.completion.is_none(), "the word is `r` alone");
        type_str(&mut state, ui, id, "i");
        assert!(state.completion.is_some(), "`ri` is two chars");
    });
}

#[test]
fn a_dot_or_a_double_colon_opens_the_popup_at_once() {
    with_ui(|ui, id| {
        let mut state = completing("obj");
        focus_end(&mut state, ui, id);
        type_str(&mut state, ui, id, ".");
        assert_eq!(labels(&state).len(), WORDS.len());
        assert_eq!(state.completion.as_ref().expect("open").start(), 4);

        let mut state = completing("mod");
        focus_end(&mut state, ui, id);
        type_str(&mut state, ui, id, ":");
        assert!(state.completion.is_none(), "one colon is not a path");
        type_str(&mut state, ui, id, ":");
        assert_eq!(labels(&state).len(), WORDS.len());
    });
}

#[test]
fn a_non_identifier_char_closes_the_popup() {
    with_ui(|ui, id| {
        let mut state = completing("pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        assert!(state.completion.is_some());
        type_str(&mut state, ui, id, " ");
        assert!(state.completion.is_none());
        assert_eq!(state.buffer.text(), "pr ");
    });
}

#[test]
fn typing_narrows_the_list_and_no_match_closes_it() {
    with_ui(|ui, id| {
        let mut state = completing("pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        type_str(&mut state, ui, id, "int");
        assert_eq!(
            labels(&state),
            ["println", "parse_int"],
            "`print` is complete, so it is left out"
        );
        type_str(&mut state, ui, id, "l");
        assert_eq!(labels(&state), ["println"]);
        type_str(&mut state, ui, id, "x");
        assert!(state.completion.is_none(), "nothing matches `printlx`");
    });
}

#[test]
fn enter_accepts_and_replaces_exactly_the_word_in_one_undo_step() {
    with_ui(|ui, id| {
        let mut state = completing("x = pri");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        let outcome = handle(&mut state, ui, id, &key_down(Key::RETURN)).expect("handled");
        assert!(outcome.changed);
        assert_eq!(
            state.buffer.text(),
            "x = print",
            "no newline, word replaced"
        );
        assert_eq!(state.view.caret, 9);
        assert!(state.completion.is_none());
        handle(&mut state, ui, id, &ctrl(Key::Z));
        assert_eq!(state.buffer.text(), "x = pri", "one undo restores the word");
    });
}

#[test]
fn tab_accepts_instead_of_indenting() {
    with_ui(|ui, id| {
        let mut state = completing("");
        focus_end(&mut state, ui, id);
        type_str(&mut state, ui, id, "pri");
        assert!(state.completion.is_some());
        let outcome = handle(&mut state, ui, id, &key_down(Key::TAB)).expect("handled");
        assert!(outcome.changed);
        assert_eq!(state.buffer.text(), "print");
    });
}

#[test]
fn accepting_uses_the_insert_text_not_the_label() {
    with_ui(|ui, id| {
        let mut state = state("pr");
        state.completer = Some(Rc::new(|_: &str, caret: usize| {
            Some(Completion {
                start: caret - 2,
                items: vec![
                    CompletionItem::new("print", CompletionKind::Function).with_insert("print()"),
                ],
            })
        }));
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        handle(&mut state, ui, id, &key_down(Key::RETURN));
        assert_eq!(state.buffer.text(), "print()");
        assert_eq!(state.view.caret, 7);
    });
}

#[test]
fn arrow_keys_move_the_selection_and_do_not_move_the_caret() {
    with_ui(|ui, id| {
        let mut state = completing("pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        let selected = |state: &EditorState| state.completion.as_ref().expect("open").selected();
        handle(&mut state, ui, id, &key_down(Key::DOWN));
        assert_eq!(selected(&state), 1);
        assert_eq!(state.view.caret, 2, "the caret stayed");
        handle(&mut state, ui, id, &key_down(Key::UP));
        handle(&mut state, ui, id, &key_down(Key::UP));
        assert_eq!(selected(&state), 2, "Up from the first wraps to the last");
        handle(&mut state, ui, id, &key_down(Key::PAGE_UP));
        assert_eq!(selected(&state), 0, "pages clamp");
        handle(&mut state, ui, id, &key_down(Key::PAGE_DOWN));
        assert_eq!(selected(&state), 2);
        handle(&mut state, ui, id, &key_down(Key::DOWN));
        assert_eq!(selected(&state), 0, "Down from the last wraps");
        handle(&mut state, ui, id, &key_down(Key::DOWN));
        handle(&mut state, ui, id, &key_down(Key::RETURN));
        assert_eq!(
            state.buffer.text(),
            "println",
            "the moved selection is accepted"
        );
    });
}

#[test]
fn escape_closes_and_tab_indents_again() {
    with_ui(|ui, id| {
        let mut state = completing("pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        let outcome = handle(&mut state, ui, id, &key_down(Key::ESCAPE)).expect("handled");
        assert!(!outcome.changed);
        assert!(state.completion.is_none());
        let outcome = handle(&mut state, ui, id, &key_down(Key::TAB)).expect("handled");
        assert!(outcome.changed);
        assert_eq!(state.buffer.text(), "pr  ", "up to the next tab stop");
    });
}

#[test]
fn enter_inserts_a_newline_when_the_popup_is_closed() {
    with_ui(|ui, id| {
        let mut state = completing("pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        handle(&mut state, ui, id, &key_down(Key::ESCAPE));
        handle(&mut state, ui, id, &key_down(Key::RETURN));
        assert_eq!(state.buffer.text(), "pr\n");
    });
}

#[test]
fn shift_tab_closes_the_popup_and_outdents() {
    with_ui(|ui, id| {
        let mut state = completing("    pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        assert!(state.completion.is_some());
        let outcome = handle(&mut state, ui, id, &shift(Key::TAB)).expect("handled");
        assert!(outcome.changed);
        assert!(state.completion.is_none());
        assert_eq!(state.buffer.text(), "pr");
    });
}

#[test]
fn moving_the_caret_out_of_the_word_closes_the_popup() {
    with_ui(|ui, id| {
        let mut state = completing("x pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        handle(&mut state, ui, id, &key_down(Key::LEFT));
        assert!(state.completion.is_some(), "still inside the word");
        handle(&mut state, ui, id, &key_down(Key::LEFT));
        assert!(state.completion.is_some(), "at the word's start");
        handle(&mut state, ui, id, &key_down(Key::LEFT));
        assert!(state.completion.is_none(), "past the start");

        handle(&mut state, ui, id, &key_down(Key::END));
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        assert!(state.completion.is_some());
        handle(&mut state, ui, id, &key_down(Key::HOME));
        assert!(state.completion.is_none(), "Home left the word");

        let mut state = completing("pr x");
        handle(&mut state, ui, id, &Event::SetFocus);
        state.view.caret = 2;
        state.view.anchor = 2;
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        assert!(state.completion.is_some());
        handle(&mut state, ui, id, &key_down(Key::RIGHT));
        assert!(state.completion.is_none(), "Right past the word's end");
    });
}

#[test]
fn selecting_text_closes_the_popup() {
    with_ui(|ui, id| {
        let mut state = completing("pri");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        handle(&mut state, ui, id, &shift(Key::LEFT));
        assert!(state.completion.is_none());
    });
}

#[test]
fn an_edit_that_is_not_typing_or_backspace_closes_the_popup() {
    with_ui(|ui, id| {
        let mut state = completing("prx");
        handle(&mut state, ui, id, &Event::SetFocus);
        state.view.caret = 2;
        state.view.anchor = 2;
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        assert!(state.completion.is_some());
        let outcome = handle(&mut state, ui, id, &key_down(Key::DELETE)).expect("handled");
        assert!(outcome.changed);
        assert!(state.completion.is_none(), "Delete is not a typing edit");

        let mut state = completing("pri");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        handle(&mut state, ui, id, &key_down(Key::BACK));
        assert_eq!(state.buffer.text(), "pr");
        assert!(state.completion.is_some(), "Backspace stays in the word");
        assert_eq!(labels(&state), ["print", "println", "parse_int"]);
        handle(&mut state, ui, id, &ctrl(Key::Z));
        assert!(state.completion.is_none(), "Undo closes it");
    });
}

#[test]
fn backspacing_before_the_words_start_closes_the_popup() {
    with_ui(|ui, id| {
        let mut state = completing("x pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        handle(&mut state, ui, id, &key_down(Key::BACK));
        handle(&mut state, ui, id, &key_down(Key::BACK));
        assert!(state.completion.is_some(), "empty word, caret at its start");
        handle(&mut state, ui, id, &key_down(Key::BACK));
        assert!(state.completion.is_none());
    });
}

#[test]
fn losing_focus_closes_the_popup() {
    with_ui(|ui, id| {
        let mut state = completing("pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        handle(&mut state, ui, id, &Event::KillFocus);
        assert!(state.completion.is_none());
    });
}

fn mouse_down(x: i32, y: i32) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn layout(state: &EditorState, ui: &Ui<()>, id: xui_core::backend::WidgetId) -> Layout {
    let popup = state.completion.as_ref().expect("open");
    Layout::compute(state, popup, &viewport(ui, id, state), ui.dpi()).expect("on screen")
}

#[test]
fn a_click_outside_the_popup_closes_it_and_moves_the_caret() {
    with_ui(|ui, id| {
        let mut state = completing("pr\n\n\n\nend");
        handle(&mut state, ui, id, &Event::SetFocus);
        state.view.caret = 2;
        state.view.anchor = 2;
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        let popup = layout(&state, ui, id).rect;
        handle(&mut state, ui, id, &mouse_down(280, popup.bottom + 40));
        assert!(state.completion.is_none());
        assert_ne!(state.view.caret, 2, "the click went on to the editor");
    });
}

#[test]
fn a_click_on_a_candidate_accepts_it() {
    with_ui(|ui, id| {
        let mut state = completing("pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        let layout = layout(&state, ui, id);
        let x = layout.rect.left + 20;
        let y = layout.rect.top + 1 + layout.row_height + layout.row_height / 2;
        let outcome = handle(&mut state, ui, id, &mouse_down(x, y)).expect("handled");
        assert!(outcome.changed);
        assert_eq!(state.buffer.text(), "println", "the second row");
        assert!(state.completion.is_none());
        assert_eq!(state.view.caret, 7);
    });
}

#[test]
fn a_click_on_the_popups_border_is_swallowed() {
    with_ui(|ui, id| {
        let mut state = completing("pr");
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        let layout = layout(&state, ui, id);
        let outcome = handle(
            &mut state,
            ui,
            id,
            &mouse_down(layout.rect.left + 20, layout.rect.top),
        )
        .expect("handled");
        assert!(!outcome.changed);
        assert!(state.completion.is_some(), "still open");
    });
}

#[test]
fn the_wheel_over_the_popup_scrolls_its_list() {
    with_ui(|ui, id| {
        static MANY: &[&str] = &[
            "item00", "item01", "item02", "item03", "item04", "item05", "item06", "item07",
            "item08", "item09", "item10", "item11",
        ];
        let mut state = state("item");
        state.completer = Some(Rc::new(words(MANY)));
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        let layout = layout(&state, ui, id);
        let wheel = |delta| Event::MouseWheel {
            delta,
            horizontal: false,
            x: layout.rect.left + 20,
            y: layout.rect.top + 10,
            modifiers: Modifiers::NONE,
        };
        let first = |state: &EditorState| state.completion.as_ref().expect("open").first();
        handle(&mut state, ui, id, &wheel(-120)).expect("handled");
        assert_eq!(first(&state), 3, "a notch down scrolls three rows");
        handle(&mut state, ui, id, &wheel(-120));
        assert_eq!(first(&state), 4, "clamped to 12 rows, 8 visible");
        handle(&mut state, ui, id, &wheel(240));
        assert_eq!(first(&state), 0);
        assert_eq!(state.view.first_line, 0, "the editor did not scroll");
        let selected = state.completion.as_ref().expect("open").selected();
        assert_eq!(selected, 0, "the selection did not move");
    });
}

#[test]
fn multibyte_words_complete_by_char_offsets() {
    with_ui(|ui, id| {
        static ACCENTED: &[&str] = &["éclair", "écran", "apple"];
        let mut state = state("日本 éc");
        state.completer = Some(Rc::new(words(ACCENTED)));
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        assert_eq!(state.completion.as_ref().expect("open").start(), 3);
        assert_eq!(labels(&state), ["éclair", "écran"]);
        handle(&mut state, ui, id, &key_down(Key::DOWN));
        handle(&mut state, ui, id, &key_down(Key::RETURN));
        assert_eq!(state.buffer.text(), "日本 écran");
        assert_eq!(state.view.caret, 8);
        handle(&mut state, ui, id, &ctrl(Key::Z));
        assert_eq!(state.buffer.text(), "日本 éc");
    });
}

#[test]
fn a_start_past_the_caret_is_clamped() {
    with_ui(|ui, id| {
        let mut state = state("pr");
        state.completer = Some(Rc::new(|_: &str, _: usize| {
            Some(Completion {
                start: 99,
                items: vec![CompletionItem::new("print", CompletionKind::Function)],
            })
        }));
        focus_end(&mut state, ui, id);
        handle(&mut state, ui, id, &ctrl(Key::SPACE));
        assert_eq!(state.completion.as_ref().expect("open").start(), 2);
        handle(&mut state, ui, id, &key_down(Key::RETURN));
        assert_eq!(state.buffer.text(), "prprint", "an empty word at the caret");
    });
}
