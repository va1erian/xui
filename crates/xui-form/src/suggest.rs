#![forbid(unsafe_code)]

//! "Did you mean": the closest known name to a misspelt one.

/// The candidate closest to `wanted`, if one is close enough to be what was
/// meant: the same name in another case or `snake_case`/`PascalCase`
/// spelling, or within a third of its length in edits.
pub fn closest<'a>(wanted: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let key = fold(wanted);
    let limit = (wanted.chars().count() / 3).max(1);
    candidates
        .into_iter()
        .map(|candidate| (distance(&key, &fold(candidate)), candidate))
        .filter(|(distance, _)| *distance <= limit)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate)
}

/// `name` lowercased without underscores, so `TabIndex`, `tab_index` and
/// `TABINDEX` compare equal.
fn fold(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// The Levenshtein distance between `a` and `b`.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = previous + usize::from(ca != *cb);
            previous = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(previous + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_misspelling_finds_its_name() {
        let names = ["text", "placeholder", "password"];
        assert_eq!(closest("txt", names), Some("text"));
        assert_eq!(closest("placeholdr", names), Some("placeholder"));
        assert_eq!(closest("Text", names), Some("text"));
        assert_eq!(closest("colour", names), None);
    }

    #[test]
    fn case_and_underscores_do_not_count() {
        assert_eq!(closest("TabIndex", ["tab_index"]), Some("tab_index"));
        assert_eq!(
            closest("multiselect", ["multi_select"]),
            Some("multi_select")
        );
        assert_eq!(closest("Colum", ["Column", "Row"]), Some("Column"));
    }
}
