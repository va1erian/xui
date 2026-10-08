#![forbid(unsafe_code)]

//! The editor's edit operations, independent of xui and of the event loop.
//!
//! Each function takes the buffer, the view and (where it needs it) the
//! clipboard, and returns whether the text changed so the widget can raise
//! `on_change`. Keeping the rules here means typing, auto-indent, indentation,
//! cut/copy/paste and undo are all unit-tested directly.

use crate::buffer::Buffer;
use crate::options::Options;
use crate::platform::Clipboard;
use crate::text::{advance, leading_whitespace};
use crate::view::View;

/// Replaces the selection (or inserts at the caret) with `text`.
pub(crate) fn splice(buffer: &mut Buffer, view: &mut View, text: &str, coalesce: bool) {
    match view.selection() {
        Some((start, end)) => {
            buffer.replace(start..end, text, coalesce);
            view.caret = start + text.chars().count();
        }
        None => {
            buffer.insert(view.caret, text, coalesce);
            view.caret += text.chars().count();
        }
    }
    view.anchor = view.caret;
    view.goal_col = None;
}

/// The closer that matches an auto-indenting opener.
fn closer_for(opener: char) -> Option<char> {
    match opener {
        '{' => Some('}'),
        '(' => Some(')'),
        '[' => Some(']'),
        _ => None,
    }
}

/// Inserts a typed character, coalescing with the previous typing run.
///
/// With [`Options::smart_indent`], typing a closing bracket (`}`, `)` or `]`)
/// on a line that is only whitespace up to the caret first removes one indent
/// level from that line, as one undo step with the bracket.
pub fn type_char(buffer: &mut Buffer, view: &mut View, options: &Options, character: char) {
    if options.smart_indent
        && matches!(character, '}' | ')' | ']')
        && view.selection().is_none()
        && dedent_before_closer(buffer, view, options, character)
    {
        return;
    }
    splice(buffer, view, &character.to_string(), true);
}

/// Dedents the whitespace-only prefix of the caret's line by one level and
/// inserts `closer` after it. Returns `false` (touching nothing) when the line
/// has no such prefix to dedent.
fn dedent_before_closer(
    buffer: &mut Buffer,
    view: &mut View,
    options: &Options,
    closer: char,
) -> bool {
    let line_start = buffer.line_start(buffer.line_of_char(view.caret));
    let prefix: Vec<char> = buffer.slice(line_start..view.caret).chars().collect();
    if prefix.is_empty() || !prefix.iter().all(|c| *c == ' ' || *c == '\t') {
        return false;
    }
    let tab = options.tab_width.max(1);
    let remove = if prefix.last() == Some(&'\t') {
        1
    } else {
        let column = prefix.iter().fold(0, |col, c| advance(col, *c, tab));
        let spaces = prefix.iter().rev().take_while(|c| **c == ' ').count();
        let step = if column % tab == 0 { tab } else { column % tab };
        step.min(spaces)
    };
    buffer.begin_edit();
    buffer.remove(view.caret - remove..view.caret, false);
    buffer.insert(view.caret - remove, &closer.to_string(), false);
    buffer.end_edit();
    view.caret = view.caret - remove + 1;
    view.anchor = view.caret;
    view.goal_col = None;
    true
}

/// Inserts `text` as a run of typed characters.
#[cfg(test)]
pub fn type_text(buffer: &mut Buffer, view: &mut View, text: &str) {
    splice(buffer, view, text, true);
}

/// Replaces the selection (if any) with a newline that copies the indentation
/// of the line the selection starts on.
pub fn enter(buffer: &mut Buffer, view: &mut View, options: &Options) {
    let selection = view.selection();
    let at = selection.map_or(view.caret, |(start, _)| start);
    let after = selection.map_or(view.caret, |(_, end)| end);
    let line = buffer.line_of_char(at);
    let column = at - buffer.line_start(line);
    let before: String = buffer.line_string(line).chars().take(column).collect();
    let indent = leading_whitespace(&before).to_owned();
    let mut extra = String::new();
    let mut split = false;
    if options.smart_indent {
        let opener = before.trim_end().chars().last();
        if let Some(closer) = opener.and_then(closer_for) {
            extra = if indent.contains('\t') && !indent.contains(' ') {
                "\t".to_owned()
            } else {
                options.indent()
            };
            split = buffer.char_at(after) == Some(closer);
        }
    }
    let first = format!("\n{indent}{extra}");
    let inserted = if split {
        format!("{first}\n{indent}")
    } else {
        first.clone()
    };
    // An explicit edit so the newline and the indentation undo together, but
    // separately from the typing run before it.
    buffer.begin_edit();
    if let Some((start, end)) = selection {
        buffer.remove(start..end, false);
    }
    buffer.insert(at, &inserted, false);
    buffer.end_edit();
    view.caret = at + first.chars().count();
    view.anchor = view.caret;
    view.goal_col = None;
}

/// Deletes the selection, or the character before the caret.
///
/// When everything before the caret on its line is spaces, it deletes back to
/// the previous tab stop instead (at most one stop's worth of spaces).
pub fn backspace(buffer: &mut Buffer, view: &mut View, options: &Options) {
    if let Some((start, end)) = view.selection() {
        buffer.replace(start..end, "", false);
        view.caret = start;
    } else if view.caret > 0 {
        let count = spaces_to_previous_stop(buffer, view.caret, options);
        buffer.remove(view.caret - count..view.caret, true);
        view.caret -= count;
    }
    view.anchor = view.caret;
    view.goal_col = None;
}

/// How many chars Backspace removes before `caret` (at least one): back to
/// the previous tab stop when only spaces precede the caret on its line.
fn spaces_to_previous_stop(buffer: &Buffer, caret: usize, options: &Options) -> usize {
    let line_start = buffer.line_start(buffer.line_of_char(caret));
    let column = caret - line_start;
    if column == 0 || !buffer.slice(line_start..caret).chars().all(|c| c == ' ') {
        return 1;
    }
    let tab = options.tab_width.max(1);
    column - (column - 1) / tab * tab
}

/// Deletes the selection, or the character after the caret.
pub fn delete_forward(buffer: &mut Buffer, view: &mut View) {
    if let Some((start, end)) = view.selection() {
        buffer.replace(start..end, "", false);
        view.caret = start;
    } else if view.caret < buffer.len_chars() {
        buffer.remove(view.caret..view.caret + 1, true);
    }
    view.anchor = view.caret;
    view.goal_col = None;
}

/// The inclusive line range a selection touches.
fn selected_lines(buffer: &Buffer, view: &View) -> (usize, usize) {
    let (start, end) = view.selection().unwrap_or((view.caret, view.caret));
    let first = buffer.line_of_char(start);
    let mut last = buffer.line_of_char(end);
    // A selection that ends at the very start of a line does not include it.
    if last > first && buffer.line_start(last) == end {
        last -= 1;
    }
    (first, last)
}

/// Indents the selection or the caret.
///
/// With no selection, or one inside a single line, the selection is replaced by
/// spaces up to the next tab stop (counted in display columns, so a `\t`
/// before it advances correctly). A selection spanning lines indents every
/// line it touches by one level and keeps the selection over them. Returns
/// whether the text changed.
pub fn indent(buffer: &mut Buffer, view: &mut View, options: &Options) -> bool {
    let one_line = view
        .selection()
        .is_none_or(|(start, end)| buffer.line_of_char(start) == buffer.line_of_char(end));
    if one_line {
        let start = view.selection().map_or(view.caret, |(start, _)| start);
        let tab = options.tab_width.max(1);
        let line_start = buffer.line_start(buffer.line_of_char(start));
        let column = buffer
            .slice(line_start..start)
            .chars()
            .fold(0, |col, c| advance(col, c, tab));
        splice(buffer, view, &" ".repeat(tab - column % tab), false);
        return true;
    }
    let (first, last) = selected_lines(buffer, view);
    let unit = options.indent();
    let width = unit.chars().count();
    let mut starts: Vec<usize> = (first..=last).map(|line| buffer.line_start(line)).collect();
    starts.sort_unstable();
    buffer.begin_edit();
    for start in starts.into_iter().rev() {
        buffer.insert(start, &unit, false);
        shift(buffer, view, start, width as i64);
    }
    buffer.end_edit();
    view.goal_col = None;
    !unit.is_empty()
}

/// Removes one indent level from every line the selection touches, or from the
/// caret's line. Returns whether the text changed.
pub fn outdent(buffer: &mut Buffer, view: &mut View, options: &Options) -> bool {
    let (first, last) = selected_lines(buffer, view);
    let width = options.tab_width.max(1);
    let mut removals: Vec<(usize, usize)> = Vec::new();
    for line in first..=last {
        let start = buffer.line_start(line);
        let text = buffer.line_string(line);
        let mut count = 0;
        for character in text.chars().take(width) {
            if character == ' ' {
                count += 1;
            } else if character == '\t' {
                count += 1;
                break;
            } else {
                break;
            }
        }
        if count > 0 {
            removals.push((start, count));
        }
    }
    if removals.is_empty() {
        return false;
    }
    buffer.begin_edit();
    for (start, count) in removals.into_iter().rev() {
        buffer.remove(start..start + count, false);
        unshift(buffer, view, start, count);
    }
    buffer.end_edit();
    view.goal_col = None;
    true
}

/// Moves `view`'s caret and anchor by `delta` when they sit after `at`.
fn shift(buffer: &Buffer, view: &mut View, at: usize, delta: i64) {
    let len = buffer.len_chars() as i64;
    let apply = |position: usize| -> usize {
        let position = position as i64;
        if position > at as i64 {
            (position + delta).clamp(0, len) as usize
        } else {
            position as usize
        }
    };
    view.caret = apply(view.caret);
    view.anchor = apply(view.anchor);
}

/// Adjusts `view`'s caret and anchor after `count` chars were removed at `at`:
/// a position after the removed span moves back, one inside it snaps to `at`.
fn unshift(buffer: &Buffer, view: &mut View, at: usize, count: usize) {
    let len = buffer.len_chars();
    let end = at + count;
    let apply = |position: usize| -> usize {
        if position <= at {
            position
        } else if position >= end {
            position - count
        } else {
            at
        }
        .min(len)
    };
    view.caret = apply(view.caret);
    view.anchor = apply(view.anchor);
}

/// Copies the selection, or the caret's whole line when nothing is selected.
/// Returns whether anything was copied.
pub fn copy(buffer: &Buffer, view: &View, clipboard: &dyn Clipboard) -> bool {
    let text = match view.selection() {
        Some((start, end)) => buffer.slice(start..end),
        None => {
            let line = buffer.line_of_char(view.caret);
            let (_, end) = line_with_terminator(buffer, line);
            buffer.slice(buffer.line_start(line)..end)
        }
    };
    if text.is_empty() {
        return false;
    }
    clipboard.set_text(&text);
    true
}

/// Cuts the selection, or the caret's whole line, to the clipboard. Returns
/// whether the text changed.
pub fn cut(buffer: &mut Buffer, view: &mut View, clipboard: &dyn Clipboard) -> bool {
    match view.selection() {
        Some((start, end)) => {
            let text = buffer.slice(start..end);
            clipboard.set_text(&text);
            buffer.replace(start..end, "", false);
            view.caret = start;
        }
        None => {
            let line = buffer.line_of_char(view.caret);
            let start = buffer.line_start(line);
            let (_, end) = line_with_terminator(buffer, line);
            if start == end {
                return false;
            }
            let text = buffer.slice(start..end);
            clipboard.set_text(&text);
            buffer.remove(start..end, false);
            view.caret = start.min(buffer.len_chars());
        }
    }
    view.anchor = view.caret;
    view.goal_col = None;
    true
}

/// Pastes the clipboard at the caret, replacing the selection. Returns whether
/// the text changed.
pub fn paste(buffer: &mut Buffer, view: &mut View, clipboard: &dyn Clipboard) -> bool {
    let Some(text) = clipboard.text() else {
        return false;
    };
    if text.is_empty() {
        return false;
    }
    splice(buffer, view, &text, false);
    true
}

/// Undoes the last action. Returns whether the text changed.
pub fn undo(buffer: &mut Buffer, view: &mut View) -> bool {
    let Some(caret) = buffer.undo() else {
        return false;
    };
    view.caret = caret.min(buffer.len_chars());
    view.anchor = view.caret;
    view.goal_col = None;
    true
}

/// Redoes the last undone action. Returns whether the text changed.
pub fn redo(buffer: &mut Buffer, view: &mut View) -> bool {
    let Some(caret) = buffer.redo() else {
        return false;
    };
    view.caret = caret.min(buffer.len_chars());
    view.anchor = view.caret;
    view.goal_col = None;
    true
}

/// The end of `line` including its terminator, when it has one.
fn line_with_terminator(buffer: &Buffer, line: usize) -> (usize, usize) {
    let start = buffer.line_start(line);
    let count = buffer.line_count();
    let end = if line + 1 < count {
        buffer.line_start(line + 1)
    } else {
        buffer.len_chars()
    };
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::InProcessClipboard;

    fn editor(text: &str) -> (Buffer, View, Options) {
        let buffer = Buffer::new(text);
        let mut view = View::new();
        view.caret = buffer.len_chars();
        view.anchor = view.caret;
        (buffer, view, Options::default())
    }

    fn place(view: &mut View, caret: usize) {
        view.caret = caret;
        view.anchor = caret;
    }

    #[test]
    fn typing_replaces_the_selection() {
        let (mut buffer, mut view, _) = editor("hello world");
        view.anchor = 0;
        view.caret = 5;
        type_text(&mut buffer, &mut view, "bye");
        assert_eq!(buffer.text(), "bye world");
        assert_eq!(view.caret, 3);
        assert_eq!(view.selection(), None);
    }

    #[test]
    fn enter_copies_the_current_indent() {
        let (mut buffer, mut view, _) = editor("    let x = 1;");
        place(&mut view, 4);
        enter(&mut buffer, &mut view, &Options::default());
        assert_eq!(buffer.text(), "    \n    let x = 1;");
        assert_eq!(view.caret, 9);
    }

    #[test]
    fn enter_replaces_the_selection() {
        let (mut buffer, mut view, _) = editor("xabcx");
        view.anchor = 1;
        view.caret = 4;
        enter(&mut buffer, &mut view, &Options::default());
        assert_eq!(
            buffer.text(),
            "x
x"
        );
        assert_eq!(view.caret, 2);
        assert_eq!(view.selection(), None);
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "xabcx", "one undo step");
    }

    #[test]
    fn indent_and_outdent_report_whether_the_text_changed() {
        let (mut buffer, mut view, options) = editor("a");
        place(&mut view, 0);
        assert!(indent(&mut buffer, &mut view, &options));
        assert!(outdent(&mut buffer, &mut view, &options));
        assert!(
            !outdent(&mut buffer, &mut view, &options),
            "nothing to remove"
        );
    }

    #[test]
    fn backspace_deletes_the_selection_or_one_char() {
        let (mut buffer, mut view, _) = editor("abc");
        view.anchor = 0;
        view.caret = 2;
        backspace(&mut buffer, &mut view, &Options::default());
        assert_eq!(buffer.text(), "c");
        assert_eq!(view.caret, 0);

        let (mut buffer, mut view, _) = editor("abc");
        place(&mut view, 3);
        backspace(&mut buffer, &mut view, &Options::default());
        assert_eq!(buffer.text(), "ab");
        assert_eq!(view.caret, 2);
    }

    #[test]
    fn delete_forward_removes_the_char_after_the_caret() {
        let (mut buffer, mut view, _) = editor("abc");
        place(&mut view, 1);
        delete_forward(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "ac");
        assert_eq!(view.caret, 1);
    }

    #[test]
    fn indent_moves_every_selected_line_and_undoes_once() {
        let (mut buffer, mut view, options) = editor("a\nb\nc");
        view.anchor = 0;
        view.caret = buffer.len_chars();
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    a\n    b\n    c");
        assert_eq!(view.anchor, 0);
        assert_eq!(view.caret, buffer.len_chars());
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "a\nb\nc");
    }

    #[test]
    fn indent_without_a_selection_inserts_at_the_caret() {
        let (mut buffer, mut view, options) = editor("ab");
        place(&mut view, 1);
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "a   b");
        assert_eq!(view.caret, 4);
    }

    #[test]
    fn outdent_removes_a_level_from_each_selected_line() {
        let (mut buffer, mut view, options) = editor("    a\n\tb\nc");
        view.anchor = 0;
        view.caret = buffer.len_chars();
        outdent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "a\nb\nc");
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "    a\n\tb\nc");
    }

    #[test]
    fn outdent_without_a_selection_uses_the_caret_line() {
        let (mut buffer, mut view, options) = editor("one\n    two");
        place(&mut view, buffer.len_chars());
        outdent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "one\ntwo");
    }

    #[test]
    fn copy_takes_the_selection_or_the_whole_line() {
        let clipboard = InProcessClipboard;
        let (buffer, mut view, _) = editor("one\ntwo");
        view.anchor = 0;
        view.caret = 3;
        assert!(copy(&buffer, &view, &clipboard));
        assert_eq!(clipboard.text().as_deref(), Some("one"));

        view.anchor = 4;
        view.caret = 4;
        assert!(copy(&buffer, &view, &clipboard));
        assert_eq!(clipboard.text().as_deref(), Some("two"));
    }

    #[test]
    fn cut_removes_the_selection_or_the_whole_line() {
        let clipboard = InProcessClipboard;
        let (mut buffer, mut view, _) = editor("one\ntwo");
        place(&mut view, 4);
        assert!(cut(&mut buffer, &mut view, &clipboard));
        assert_eq!(buffer.text(), "one\n");
        assert_eq!(clipboard.text().as_deref(), Some("two"));
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "one\ntwo");
    }

    #[test]
    fn paste_replaces_the_selection() {
        let clipboard = InProcessClipboard;
        clipboard.set_text("X");
        let (mut buffer, mut view, _) = editor("one");
        place(&mut view, 1);
        assert!(paste(&mut buffer, &mut view, &clipboard));
        assert_eq!(buffer.text(), "oXne");
        assert_eq!(view.caret, 2);
    }

    #[test]
    fn undo_and_redo_drive_the_view_caret() {
        let (mut buffer, mut view, _) = editor("");
        type_text(&mut buffer, &mut view, "ab");
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "");
        assert_eq!(view.caret, 0);
        redo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "ab");
        assert_eq!(view.caret, 2);
    }

    /// `text` with the caret at the `|` (removed), and the options defaults.
    fn at_bar(text: &str) -> (Buffer, View, Options) {
        let caret = text
            .chars()
            .position(|c| c == '|')
            .expect("a | marks the caret");
        let (buffer, mut view, options) = editor(&text.replacen('|', "", 1));
        place(&mut view, caret);
        (buffer, view, options)
    }

    fn type_one(buffer: &mut Buffer, view: &mut View, options: &Options, character: char) {
        type_char(buffer, view, options, character);
    }

    #[test]
    fn tab_inserts_up_to_the_next_tab_stop() {
        let (mut buffer, mut view, options) = at_bar("ab|c");
        assert!(indent(&mut buffer, &mut view, &options));
        assert_eq!(buffer.text(), "ab  c");
        assert_eq!(view.caret, 4);

        let (mut buffer, mut view, options) = at_bar("|abc");
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    abc", "a full level at a stop");

        let (mut buffer, mut view, options) = at_bar("abcd|");
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "abcd    ");
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "abcd", "one undo step");
    }

    #[test]
    fn tab_stops_count_a_tab_char_by_display_column() {
        let (mut buffer, mut view, options) = at_bar("\t|x");
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "\t    x", "the tab already reached column 4");

        let (mut buffer, mut view, options) = at_bar("a\t|x");
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "a\t    x");

        let (mut buffer, mut view, options) = at_bar("  \t |x");
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "  \t    x", "column 5 needs 3 more");
    }

    #[test]
    fn tab_stops_count_chars_not_bytes() {
        let (mut buffer, mut view, options) = at_bar("é€|x");
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "é€  x");
        assert_eq!(view.caret, 4);
    }

    #[test]
    fn tab_in_an_empty_buffer_and_with_another_width() {
        let (mut buffer, mut view, options) = editor("");
        assert!(indent(&mut buffer, &mut view, &options));
        assert_eq!(buffer.text(), "    ");

        let options = Options {
            tab_width: 2,
            ..Options::default()
        };
        let (mut buffer, mut view, _) = at_bar("a|");
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "a ");
    }

    #[test]
    fn tab_replaces_a_selection_inside_one_line() {
        let (mut buffer, mut view, options) = editor("ab cd ef");
        view.anchor = 3;
        view.caret = 5;
        assert!(indent(&mut buffer, &mut view, &options));
        assert_eq!(
            buffer.text(),
            "ab   ef",
            "replaced by the stop from column 3"
        );
        assert_eq!(view.selection(), None);
        assert_eq!(view.caret, 4);
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "ab cd ef", "one undo step");

        // A reversed selection behaves the same.
        let (mut buffer, mut view, options) = editor("ab cd ef");
        view.anchor = 5;
        view.caret = 3;
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "ab   ef");
    }

    #[test]
    fn tab_on_a_whole_line_selection_without_its_newline_replaces_it() {
        let (mut buffer, mut view, options) = editor("one\ntwo");
        view.anchor = 0;
        view.caret = 3;
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    \ntwo");
    }

    #[test]
    fn tab_on_a_selection_spanning_lines_indents_each_line_and_keeps_it() {
        let (mut buffer, mut view, options) = editor("ab\ncd\nef");
        view.anchor = 1;
        view.caret = 7;
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    ab\n    cd\n    ef");
        assert_eq!(view.anchor, 5, "anchor followed its char");
        assert_eq!(view.caret, 19, "caret followed its char");

        let (mut buffer, mut view, options) = editor("ab\ncd\nef");
        view.anchor = 7;
        view.caret = 1;
        indent(&mut buffer, &mut view, &options);
        assert_eq!(
            (view.anchor, view.caret),
            (19, 5),
            "reversed keeps direction"
        );
    }

    #[test]
    fn tab_on_a_selection_ending_at_a_line_start_skips_that_line() {
        let (mut buffer, mut view, options) = editor("ab\ncd\nef");
        view.anchor = 0;
        view.caret = 6;
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    ab\n    cd\nef");
        assert_eq!(view.anchor, 0);
        assert_eq!(view.caret, 14, "still ends at the start of `ef`");

        // Starting mid-line and ending at the next line start indents only the first.
        let (mut buffer, mut view, options) = editor("abc\ndef");
        view.anchor = 1;
        view.caret = 4;
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    abc\ndef");
    }

    #[test]
    fn tab_on_a_crlf_selection_indents_by_lines() {
        let (mut buffer, mut view, options) = editor("a\r\nb\r\nc");
        view.anchor = 0;
        view.caret = buffer.len_chars();
        indent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    a\r\n    b\r\n    c");
    }

    #[test]
    fn shift_tab_keeps_the_caret_at_the_same_place_in_the_text() {
        let (mut buffer, mut view, options) = at_bar("        ab|c");
        outdent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    abc");
        assert_eq!(view.caret, 6, "still after `ab`");

        // A caret inside the removed whitespace lands at the line start.
        let (mut buffer, mut view, options) = at_bar("  |  x");
        outdent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "x");
        assert_eq!(view.caret, 0);

        // Mixed leading whitespace, tab first.
        let (mut buffer, mut view, options) = at_bar("\t  y|");
        outdent(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "  y");
        assert_eq!(view.caret, 3);
    }

    #[test]
    fn shift_tab_on_an_empty_buffer_or_unindented_line_changes_nothing() {
        let (mut buffer, mut view, options) = editor("");
        assert!(!outdent(&mut buffer, &mut view, &options));
        let (mut buffer, mut view, options) = at_bar("|x");
        assert!(!outdent(&mut buffer, &mut view, &options));
        assert!(!buffer.can_undo());
    }

    #[test]
    fn backspace_in_leading_spaces_goes_back_to_the_previous_stop() {
        let options = Options::default();
        let (mut buffer, mut view, _) = at_bar("      |x");
        backspace(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    x", "col 6 -> col 4");
        assert_eq!(view.caret, 4);
        backspace(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "x", "col 4 -> col 0");
        assert_eq!(view.caret, 0);
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "      x", "the run undoes together");
    }

    #[test]
    fn backspace_deletes_one_space_when_one_is_all_that_is_left_to_the_stop() {
        let (mut buffer, mut view, options) = at_bar("     |x");
        backspace(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    x");
        assert!(view.caret == 4);
    }

    #[test]
    fn backspace_after_text_or_a_tab_char_deletes_one_char() {
        let (mut buffer, mut view, options) = at_bar("    a  |");
        backspace(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "    a ");

        let (mut buffer, mut view, options) = at_bar("\t  |x");
        backspace(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "	 x", "mixed whitespace: one char");

        let (mut buffer, mut view, options) = at_bar("\t|x");
        backspace(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "x");
    }

    #[test]
    fn backspace_at_a_line_start_joins_lines_and_at_the_buffer_start_does_nothing() {
        let (mut buffer, mut view, options) = at_bar("ab\n|    cd");
        backspace(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "ab    cd");
        assert_eq!(view.caret, 2);

        let (mut buffer, mut view, options) = at_bar("|  x");
        backspace(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "  x");
        assert!(!buffer.can_undo());

        let (mut buffer, mut view, options) = editor("");
        backspace(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "");
    }

    #[test]
    fn backspace_stop_is_per_line_and_counts_chars_not_bytes() {
        let (mut buffer, mut view, options) = at_bar("é€\n      |z");
        backspace(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "é€\n    z");
        assert_eq!(view.caret, 7);
    }

    #[test]
    fn enter_adds_a_level_after_an_opening_bracket() {
        for (text, expected) in [
            ("fn f() {|", "fn f() {\n    "),
            ("  call(|", "  call(\n      "),
            ("  v = [  |", "  v = [  \n      "),
        ] {
            let (mut buffer, mut view, options) = at_bar(text);
            enter(&mut buffer, &mut view, &options);
            assert_eq!(buffer.text(), expected, "{text}");
            assert_eq!(view.caret, buffer.len_chars());
        }
    }

    #[test]
    fn enter_between_a_bracket_pair_puts_the_closer_on_its_own_line() {
        let (mut buffer, mut view, options) = at_bar("  if x {|}");
        enter(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "  if x {\n      \n  }");
        assert_eq!(view.caret, 15, "on the indented middle line");
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "  if x {}", "one undo step");

        let (mut buffer, mut view, options) = at_bar("(|)");
        enter(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "(\n    \n)");

        let (mut buffer, mut view, options) = at_bar("a[|]");
        enter(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "a[\n    \n]");
    }

    #[test]
    fn enter_does_not_split_a_mismatched_pair() {
        let (mut buffer, mut view, options) = at_bar("{|)");
        enter(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "{\n    )");
    }

    #[test]
    fn enter_after_a_tab_indented_opener_indents_with_a_tab() {
        let (mut buffer, mut view, options) = at_bar("\tf {|}");
        enter(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "\tf {\n\t\t\n\t}");
        assert_eq!(view.caret, 7);
    }

    #[test]
    fn enter_with_multibyte_text_before_the_caret_uses_char_offsets() {
        let (mut buffer, mut view, options) = at_bar("  é€ {|}");
        enter(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "  é€ {\n      \n  }");
        assert_eq!(view.caret, 13);
    }

    #[test]
    fn enter_with_a_selection_looks_at_its_start_and_end() {
        let (mut buffer, mut view, options) = editor("f{abc}");
        view.anchor = 2;
        view.caret = 5;
        enter(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "f{\n    \n}");
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "f{abc}");
    }

    #[test]
    fn enter_does_not_add_a_level_when_smart_indent_is_off() {
        let options = Options {
            smart_indent: false,
            ..Options::default()
        };
        let (mut buffer, mut view, _) = at_bar("  f {|}");
        enter(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "  f {\n  }");
        assert_eq!(view.caret, 8);
    }

    #[test]
    fn enter_in_an_empty_buffer_inserts_a_bare_newline() {
        let (mut buffer, mut view, options) = editor("");
        enter(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "\n");
        assert_eq!(view.caret, 1);
    }

    #[test]
    fn enter_in_a_crlf_buffer_keeps_the_indent_and_adds_a_level() {
        let (mut buffer, mut view, options) = at_bar("a\r\n  b {|\r\nc");
        enter(&mut buffer, &mut view, &options);
        assert_eq!(buffer.text(), "a\r\n  b {\n      \r\nc");
    }

    #[test]
    fn a_closer_on_a_whitespace_only_line_dedents_it_first() {
        let (mut buffer, mut view, options) = at_bar("{\n    |");
        type_one(&mut buffer, &mut view, &options, '}');
        assert_eq!(buffer.text(), "{\n}");
        assert_eq!(view.caret, 3);
        undo(&mut buffer, &mut view);
        assert_eq!(buffer.text(), "{\n    ", "dedent and closer undo together");

        let (mut buffer, mut view, options) = at_bar("f(\n        |");
        type_one(&mut buffer, &mut view, &options, ')');
        assert_eq!(buffer.text(), "f(\n    )");

        let (mut buffer, mut view, options) = at_bar("[\n\t|");
        type_one(&mut buffer, &mut view, &options, ']');
        assert_eq!(buffer.text(), "[\n]");
    }

    #[test]
    fn a_closer_dedents_to_the_stop_when_off_a_stop() {
        let (mut buffer, mut view, options) = at_bar("      |");
        type_one(&mut buffer, &mut view, &options, '}');
        assert_eq!(buffer.text(), "    }");
    }

    #[test]
    fn a_closer_elsewhere_is_typed_plainly() {
        let (mut buffer, mut view, options) = at_bar("|");
        type_one(&mut buffer, &mut view, &options, '}');
        assert_eq!(buffer.text(), "}", "nothing to dedent at column 0");

        let (mut buffer, mut view, options) = at_bar("  x|");
        type_one(&mut buffer, &mut view, &options, ')');
        assert_eq!(buffer.text(), "  x)");

        let (mut buffer, mut view, _) = at_bar("    |");
        let options = Options {
            smart_indent: false,
            ..Options::default()
        };
        type_one(&mut buffer, &mut view, &options, '}');
        assert_eq!(buffer.text(), "    }");

        let (mut buffer, mut view, options) = editor("    ab");
        view.anchor = 0;
        view.caret = 4;
        type_one(&mut buffer, &mut view, &options, '}');
        assert_eq!(buffer.text(), "}ab", "a selection is just replaced");
    }
}
