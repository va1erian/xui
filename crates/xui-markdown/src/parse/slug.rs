#![forbid(unsafe_code)]

//! GitHub-style heading anchors.

use std::collections::{HashMap, HashSet};

/// Hands out unique slugs: the first `foo` is `foo`, then `foo-1`, `foo-2`,
/// …, skipping any a heading already took (a heading named `foo-1` before the
/// second `foo` makes that one `foo-2`).
#[derive(Default)]
pub(super) struct Slugs {
    used: HashSet<String>,
    /// The last suffix handed out for each base.
    suffix: HashMap<String, u32>,
}

impl Slugs {
    /// The unique slug for a heading with this text.
    pub(super) fn next(&mut self, heading: &str) -> Box<str> {
        let base = slugify(heading);
        let mut slug = base.clone();
        if self.used.contains(&slug) {
            let n = self.suffix.entry(base.clone()).or_insert(0);
            loop {
                *n += 1;
                slug = format!("{base}-{n}");
                if !self.used.contains(&slug) {
                    break;
                }
            }
        }
        self.used.insert(slug.clone());
        slug.into()
    }
}

/// Lowercases, keeps alphanumerics, `-` and `_`, turns spaces into `-` and
/// drops the rest.
fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            slug.push(c);
        } else if c == ' ' {
            slug.push('-');
        }
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_suffix_never_collides_with_a_heading_of_that_name() {
        let mut slugs = Slugs::default();
        let got: Vec<Box<str>> = ["A", "A", "A-1", "A", "B"]
            .iter()
            .map(|h| slugs.next(h))
            .collect();
        assert_eq!(&*got, ["a", "a-1", "a-1-1", "a-2", "b"].map(Box::from));
    }
}
