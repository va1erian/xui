#![forbid(unsafe_code)]

//! Finding where, in a text, the next token starts.
//!
//! RON reports an error where its cursor stopped, which for a refused node is
//! just before it: after the `[` or `content:` that precedes it, ahead of the
//! whitespace. [`next_token`] moves such a position to the node itself.

/// The one-based `(line, column)` of the first token at or after `(line,
/// column)` in `text`: whitespace and `//` and (nested) `/* */` comments are
/// skipped. A position outside the text is returned as it is.
pub(crate) fn next_token(text: &str, line: usize, column: usize) -> (usize, usize) {
    match offset_of(text, line, column) {
        Some(offset) => position_of(text, skip_trivia(text, offset)),
        None => (line, column),
    }
}

/// `(line, column)`, moved back over a `-` or `+` just before it: where the
/// number whose digits start there begins. A position elsewhere is returned
/// as it is.
pub(crate) fn sign_start(text: &str, line: usize, column: usize) -> (usize, usize) {
    match offset_of(text, line, column) {
        Some(offset) if text[..offset].ends_with(['-', '+']) => position_of(text, offset - 1),
        _ => (line, column),
    }
}

/// The byte offset of `(line, column)`, columns counted in characters.
fn offset_of(text: &str, line: usize, column: usize) -> Option<usize> {
    let mut start = 0;
    for _ in 1..line {
        start += text[start..].find('\n')? + 1;
    }
    let skipped = column.checked_sub(1)?;
    let rest = &text[start..];
    match rest.char_indices().nth(skipped) {
        Some((index, _)) => Some(start + index),
        None => (rest.chars().count() == skipped).then_some(text.len()),
    }
}

/// `(line, column)` of the byte `offset`, as RON counts them.
fn position_of(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset];
    let line = 1 + before.matches('\n').count();
    let column = 1 + before.chars().rev().take_while(|&c| c != '\n').count();
    (line, column)
}

/// The offset of the first byte at or after `offset` that is not whitespace
/// or inside a comment.
fn skip_trivia(text: &str, mut offset: usize) -> usize {
    loop {
        let rest = &text[offset..];
        let trimmed = rest.trim_start();
        offset += rest.len() - trimmed.len();
        if trimmed.starts_with("//") {
            offset += trimmed.find('\n').map_or(trimmed.len(), |end| end + 1);
        } else if trimmed.starts_with("/*") {
            offset += block_comment_len(trimmed);
        } else {
            return offset;
        }
    }
}

/// The length of the block comment `text` starts with; the whole text when it
/// is not closed. Block comments nest.
fn block_comment_len(text: &str) -> usize {
    let bytes = text.as_bytes();
    let (mut depth, mut index) = (0usize, 0usize);
    while index < bytes.len() {
        if bytes[index..].starts_with(b"/*") {
            depth += 1;
            index += 2;
        } else if bytes[index..].starts_with(b"*/") {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_token_skips_whitespace_and_comments() {
        let text = "Form(\n  // a note\n  /* one /* two */ */ root: X)";
        // Just after `Form(`, at the end of line 1.
        assert_eq!(next_token(text, 1, 6), (3, 23));
        // Already on a token.
        assert_eq!(next_token("Form(a)", 1, 6), (1, 6));
        // Past the end of the text.
        assert_eq!(next_token("ab", 5, 1), (5, 1));
        assert_eq!(next_token("ab  ", 1, 3), (1, 5));
    }

    #[test]
    fn columns_count_characters() {
        // Two two-byte characters, then the cursor.
        assert_eq!(next_token("\"éé\" x", 1, 5), (1, 6));
    }

    #[test]
    fn a_number_starts_at_its_sign() {
        assert_eq!(sign_start("fill: -1", 1, 8), (1, 7));
        assert_eq!(sign_start("fill: +1", 1, 8), (1, 7));
        assert_eq!(sign_start("fill: 1", 1, 7), (1, 7));
        assert_eq!(sign_start("a\nb: -12", 2, 5), (2, 4));
    }

    #[test]
    fn an_unclosed_comment_runs_to_the_end() {
        assert_eq!(next_token("a /* b", 1, 2), (1, 7));
    }
}
