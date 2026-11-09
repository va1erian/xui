//! Parsing Markdown into the flat document model.

use xui_markdown::{Align, Block, BlockKind, Document, Inline, Marker};

/// Checks every structural invariant the layout relies on.
fn check(doc: &Document) {
    let len = doc.text().len() as u32;
    let mut prev_end = 0;
    for (i, b) in doc.blocks().iter().enumerate() {
        let t = &b.text;
        assert!(
            t.start >= prev_end && t.start <= t.end && t.end <= len,
            "block {i} range"
        );
        assert!(doc.text().is_char_boundary(t.start as usize));
        assert!(doc.text().is_char_boundary(t.end as usize));
        prev_end = t.end;
        let spans = doc.spans(b);
        assert_eq!(spans.is_empty(), t.is_empty(), "block {i} spans vs text");
        let cells = doc.cells(b);
        let mut at = t.start;
        for (k, s) in spans.iter().enumerate() {
            assert_eq!(s.range.start, at, "block {i} spans partition");
            assert!(s.range.start < s.range.end);
            at = s.range.end;
            if let Some(l) = s.link {
                doc.link(l);
            }
            let Some(p) = k.checked_sub(1).map(|p| &spans[p]) else {
                continue;
            };
            let seam = cells.iter().any(|c| c.start == s.range.start);
            assert!(
                seam || (p.style, p.link, p.image) != (s.style, s.link, s.image),
                "block {i} unmerged"
            );
        }
        assert_eq!(at, t.end, "block {i} spans cover text");
        if let BlockKind::TableRow { table, .. } = &b.kind {
            assert_eq!(cells.len(), doc.table(*table).columns.len());
            let mut at = t.start;
            for c in cells {
                assert_eq!(c.start, at, "cells contiguous");
                at = c.end;
                assert!(spans.iter().all(|s| s.range.end <= c.start
                    || s.range.start >= c.end
                    || (s.range.start >= c.start && s.range.end <= c.end)));
            }
            assert_eq!(at, t.end, "cells cover row");
        } else {
            assert!(cells.is_empty());
        }
    }
}

fn parse(src: &str) -> Document {
    let doc = Document::parse(src);
    check(&doc);
    doc
}

fn texts(doc: &Document) -> Vec<&str> {
    doc.blocks().iter().map(|b| doc.block_text(b)).collect()
}

fn kinds(doc: &Document) -> Vec<BlockKind> {
    doc.blocks().iter().map(|b| b.kind.clone()).collect()
}

fn styles(doc: &Document, b: &Block) -> Vec<(String, Inline)> {
    doc.spans(b)
        .iter()
        .map(|s| {
            (
                doc.text()[s.range.start as usize..s.range.end as usize].to_string(),
                s.style,
            )
        })
        .collect()
}

#[test]
fn headings_and_paragraphs() {
    let d = parse("# One\n\nbody *text*\n\n###### Six");
    assert_eq!(
        kinds(&d),
        [
            BlockKind::Heading(1),
            BlockKind::Paragraph,
            BlockKind::Heading(6)
        ]
    );
    assert_eq!(texts(&d), ["One", "body text", "Six"]);
    assert_eq!(parse("").blocks().len(), 0);
}

#[test]
fn inline_styles_nest() {
    let d = parse("a *b **c** d* ~~e~~ `f`");
    let both = Inline::BOLD.with(Inline::ITALIC);
    assert_eq!(
        styles(&d, &d.blocks()[0]),
        [
            ("a ".into(), Inline::NONE),
            ("b ".into(), Inline::ITALIC),
            ("c".into(), both),
            (" d".into(), Inline::ITALIC),
            (" ".into(), Inline::NONE),
            ("e".into(), Inline::STRIKE),
            (" ".into(), Inline::NONE),
            ("f".into(), Inline::CODE),
        ]
    );
}

#[test]
fn links_autolinks_and_images() {
    let d = parse("[x **y**](http://a/b?c=1 \"t\") <http://auto.link> ![alt *t*](pic.png)");
    let b = &d.blocks()[0];
    assert_eq!(d.block_text(b), "x y http://auto.link alt t");
    let spans = d.spans(b);
    assert_eq!(d.link(spans[0].link.unwrap()), "http://a/b?c=1");
    assert_eq!(spans[1].link, spans[0].link);
    assert_eq!(d.link(spans[3].link.unwrap()), "http://auto.link");
    let img = spans.last().unwrap();
    assert!(img.style.contains(Inline::IMAGE) && img.link.is_none());
}

#[test]
fn breaks_and_html_are_literal() {
    let d = parse("one\ntwo  \nthree\\\nfour <b>x</b>");
    assert_eq!(texts(&d), ["one two\nthree\nfour <b>x</b>"]);
    let d = parse("<script>alert(1)</script>\n\nafter");
    assert_eq!(texts(&d), ["<script>alert(1)</script>", "after"]);
    assert_eq!(d.blocks()[0].kind, BlockKind::Paragraph);
}

#[test]
fn lists_markers_and_depth() {
    let d = parse("- a\n- b\n  - c\n\n3. x\n4. y\n\n- [ ] todo\n- [x] done");
    use BlockKind::ListItem as L;
    assert_eq!(
        kinds(&d),
        [
            L(Marker::Bullet),
            L(Marker::Bullet),
            L(Marker::Bullet),
            L(Marker::Number(3)),
            L(Marker::Number(4)),
            L(Marker::Task(false)),
            L(Marker::Task(true)),
        ]
    );
    assert_eq!(texts(&d), ["a", "b", "c", "x", "y", "todo", "done"]);
    let depth: Vec<u8> = d.blocks().iter().map(|b| b.list).collect();
    assert_eq!(depth, [1, 1, 2, 1, 1, 1, 1]);
}

#[test]
fn loose_and_odd_items() {
    let d = parse(
        "1. first\n\n   second\n\n2. - nested\n3. ```\n   code\n   ```\n4.\n- [ ] loose\n\n  more",
    );
    assert_eq!(
        kinds(&d)[..4],
        [
            BlockKind::ListItem(Marker::Number(1)),
            BlockKind::Paragraph,
            BlockKind::ListItem(Marker::Number(2)),
            BlockKind::ListItem(Marker::Bullet),
        ]
    );
    let rest = &d.blocks()[4..];
    assert_eq!(rest[0].kind, BlockKind::ListItem(Marker::Number(3)));
    assert!(rest[0].text.is_empty());
    assert_eq!(d.block_text(&rest[1]), "code");
    assert_eq!(rest[1].list, 1);
    assert_eq!(rest[2].kind, BlockKind::ListItem(Marker::Number(4)));
    assert!(rest[2].text.is_empty());
    assert!(
        d.blocks()
            .iter()
            .any(|b| b.kind == BlockKind::ListItem(Marker::Task(false)))
    );
}

#[test]
fn code_blocks() {
    let d =
        parse("```rust ignore\nfn a() {}\n\nfn b() {}\n```\n\n    indented\n    two\n\n```\n```");
    assert_eq!(
        kinds(&d)[0],
        BlockKind::Code {
            lang: "rust".into()
        }
    );
    assert_eq!(texts(&d), ["fn a() {}\n\nfn b() {}", "indented\ntwo", ""]);
    assert_eq!(kinds(&d)[1], BlockKind::Code { lang: "".into() });
}

#[test]
fn quotes_and_rules() {
    let d = parse("> a\n>\n> > b\n\n---\n\nc");
    let q: Vec<u8> = d.blocks().iter().map(|b| b.quote).collect();
    assert_eq!(q, [1, 2, 0, 0]);
    assert_eq!(d.blocks()[2].kind, BlockKind::Rule);
    assert!(d.blocks()[2].text.is_empty());
}

#[test]
fn tables() {
    let d = parse("| a | *b* | c |\n|:--|:-:|--:|\n| 1 | 2 |\n| 3 | 4 | 5 | 6 |\n");
    let BlockKind::TableRow { table, header, .. } = d.blocks()[0].kind else {
        panic!()
    };
    assert!(header);
    assert_eq!(
        d.table(table).columns,
        [Align::Left, Align::Center, Align::Right]
    );
    assert_eq!(d.blocks().len(), 3);
    for b in d.blocks() {
        assert_eq!(d.cells(b).len(), 3);
    }
    let cell = |b: usize, c: usize| {
        let r = d.cells(&d.blocks()[b])[c].clone();
        &d.text()[r.start as usize..r.end as usize]
    };
    assert_eq!((cell(0, 0), cell(0, 1), cell(0, 2)), ("a", "b", "c"));
    assert_eq!((cell(1, 1), cell(1, 2)), ("2", ""));
    assert_eq!((cell(2, 2), d.block_text(&d.blocks()[2])), ("5", "345"));
}

#[test]
fn anchors() {
    let d = parse(
        "# Hello, World!\n## Hello, World!\n# Hello World\n# `a_b` & c-d\n# !!\n# !!\n# Ünï Çode",
    );
    assert_eq!(d.anchor("hello-world"), Some(0));
    assert_eq!(d.anchor("hello-world-1"), Some(1));
    assert_eq!(d.anchor("hello-world-2"), Some(2));
    assert_eq!(d.anchor("a_b--c-d"), Some(3));
    assert_eq!(d.anchor(""), Some(4));
    assert_eq!(d.anchor("-1"), Some(5));
    assert_eq!(d.anchor("ünï-çode"), Some(6));
    assert_eq!(d.anchor("nope"), None);
}

#[test]
fn limited_cuts_at_block_boundaries() {
    let src = "aaa\n\nbbb\n\nccc";
    let (d, cut) = Document::parse_limited(src, 100);
    assert!(!cut && d.blocks().len() == 3);
    let (d, cut) = Document::parse_limited(src, 10);
    assert!(cut);
    assert_eq!(texts(&d), ["aaa", "bbb"]);
    let (d, cut) = Document::parse_limited("l1\nl2\nl3", 5);
    assert!(cut);
    assert_eq!(texts(&d), ["l1"]);
    let (d, cut) = Document::parse_limited("abcdef", 3);
    assert!(cut);
    assert_eq!(texts(&d), ["abc"]);
    // Every limit, including ones inside multi-byte characters, is safe.
    let src = "é\u{1F600}日本 a\n\nè\u{1F600}\n x";
    for max in 0..=src.len() {
        let (d, _) = Document::parse_limited(src, max);
        check(&d);
    }
}

#[test]
fn never_panics_on_odd_input() {
    for src in [
        "|",
        "|a|\n|-|\n|b|c|",
        "- ",
        "1.",
        "> ",
        "```",
        "[",
        "![",
        "<",
        "* * *",
        "\0",
        "\r\n\r\n",
        "- [ ]",
        "# ",
        "~~",
        "    ",
        "a  \n",
        "> - > 1. - ```\n",
        "| a |\n|---|\n| `x |",
    ] {
        parse(src);
    }
}

#[test]
fn large_document_is_consistent() {
    let mut src = String::new();
    for i in 0..2000 {
        src += &format!(
            "## Section {i}\n\nText *em* **strong** `code` [link](http://x/{i}) ~~gone~~\nsoft\n\n\
             - item {i}\n  - [x] nested\n1. one\n\n> quote\n\n```rs\nlet a = {i};\n```\n\n\
             | a | b |\n|---|--:|\n| 1 | **2** |\n\n---\n\n"
        );
    }
    let d = parse(&src);
    assert_eq!(d.blocks().len(), 2000 * 10);
    assert_eq!(
        d.anchor("section-1999")
            .map(|i| d.block_text(&d.blocks()[i])),
        Some("Section 1999")
    );
}
