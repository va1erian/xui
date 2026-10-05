//! `docs/cookbook.md` shows the recipes of `examples/cookbook.rs` verbatim:
//! every `// [name]` region of the example appears in the doc, so a recipe
//! that changes (and still compiles) cannot leave a stale copy behind.

use std::path::Path;

fn read(relative: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("{relative}: {error}"))
        .replace("\r\n", "\n")
}

#[test]
fn every_recipe_in_the_example_is_in_the_doc() {
    let example = read("crates/xui/examples/cookbook.rs");
    let doc = read("docs/cookbook.md");
    let mut regions = 0;
    for start in example.match_indices("// [").map(|(at, _)| at) {
        let name_end = example[start..].find(']').expect("a closed marker") + start;
        let name = &example[start + 4..name_end];
        if name.starts_with('/') {
            continue;
        }
        let body_start = example[name_end..].find('\n').expect("a body") + name_end + 1;
        let end = example
            .find(&format!("// [/{name}]"))
            .unwrap_or_else(|| panic!("an end marker for {name}"));
        let body = example[body_start..end].trim_end();
        assert!(
            doc.contains(body),
            "docs/cookbook.md is missing recipe `{name}`"
        );
        regions += 1;
    }
    assert!(regions >= 6, "found {regions} recipes");
}
