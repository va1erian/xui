#![forbid(unsafe_code)]

//! Ordering for the text a sorted [`ListView`](super::ListView) column shows.

use std::cmp::Ordering;

/// Compares two cell texts the way a person reads a column: runs of digits
/// compare by value (`"9"` before `"10"`, `"3:07"` before `"10:02"`) and other
/// text compares without regard to case. Plain `str::cmp` sorts `"100"` before
/// `"20"`, which looks broken in a numeric column.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (x, y) = (digits(&mut a), digits(&mut b));
                // Leading zeros are ignored, so a longer run is a bigger number.
                let order = x.len().cmp(&y.len()).then_with(|| x.cmp(&y));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// Consumes a run of ASCII digits and returns it without leading zeros.
fn digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while let Some(c) = chars.next_if(char::is_ascii_digit) {
        if !(run.is_empty() && c == '0') {
            run.push(c);
        }
    }
    run
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_runs_compare_by_value() {
        assert_eq!(natural_cmp("9", "10"), Ordering::Less);
        assert_eq!(natural_cmp("100", "20"), Ordering::Greater);
        assert_eq!(natural_cmp("3:07", "10:02"), Ordering::Less);
        assert_eq!(natural_cmp("007", "7"), Ordering::Equal);
        assert_eq!(natural_cmp("Track 2", "Track 10"), Ordering::Less);
    }

    #[test]
    fn text_ignores_case_and_a_prefix_sorts_first() {
        assert_eq!(natural_cmp("abc", "ABD"), Ordering::Less);
        assert_eq!(natural_cmp("Abc", "abc"), Ordering::Equal);
        assert_eq!(natural_cmp("ab", "abc"), Ordering::Less);
        assert_eq!(natural_cmp("abc", "ab"), Ordering::Greater);
        assert_eq!(natural_cmp("1a", "10"), Ordering::Less);
        assert_eq!(natural_cmp("", ""), Ordering::Equal);
    }
}
