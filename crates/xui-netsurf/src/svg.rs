#![forbid(unsafe_code)]

//! SVG images for NetSurf's image handler (`csrc/nsx_image.c`), drawn with
//! `resvg`: the document's size (its `width`/`height`, else its `viewBox`) is
//! what the page is laid out with, and the pixels are rendered at
//! [`RASTER_SCALE`] times that, so the image stays sharp when the view paints
//! at 2x.
//!
//! An SVG comes from the network, so it is untrusted: it may be at most
//! [`MAX_BYTES`] long, nest [`MAX_DEPTH`] elements deep and cost
//! [`MAX_COST`] once every `<use>` is expanded, its bitmap is held to
//! [`MAX_PIXELS`], and nothing it refers to is ever loaded (no file, no
//! nested image, no font). Text is not drawn: there are no fonts.

use std::collections::HashMap;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, ImageHrefResolver, Options, Tree};

use crate::image::MAX_PIXELS;

/// The largest SVG document read, in bytes. Logos and icons are a few
/// kilobytes; a detailed map or diagram a few megabytes.
pub(crate) const MAX_BYTES: usize = 8 * 1024 * 1024;

/// How deeply elements may nest, counting each `<use>` as a level above
/// what it refers to. The SVG converter recurses per level, so this keeps a
/// hostile document from exhausting the engine thread's stack; drawings
/// from editors nest a few dozen deep.
const MAX_DEPTH: usize = 128;

/// The most elements a document may draw once each `<use>` is replaced by
/// what it refers to: nested `<use>`s can otherwise multiply a small file
/// into billions of shapes.
const MAX_COST: u64 = 1 << 20;

/// How many device pixels each CSS pixel of the image gets.
pub(crate) const RASTER_SCALE: u32 = 2;

/// Whether `data` is an SVG document: an `<svg` element first, after an
/// optional byte order mark, XML declaration, comments, processing
/// instructions and doctype.
pub(crate) fn sniff(data: &[u8]) -> bool {
    let mut rest = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    loop {
        rest = trim_start(rest);
        let skip = if rest.starts_with(b"<?") {
            find(rest, b"?>").map(|at| at + 2)
        } else if rest.starts_with(b"<!--") {
            find(rest, b"-->").map(|at| at + 3)
        } else if rest.starts_with(b"<!") {
            // A doctype; an internal subset holds no `>` outside quotes
            // in documents worth sniffing.
            skip_doctype(rest)
        } else {
            return rest.starts_with(b"<svg")
                && rest
                    .get(4)
                    .is_some_and(|c| c.is_ascii_whitespace() || matches!(c, b'>' | b'/'));
        };
        match skip {
            Some(at) => rest = &rest[at..],
            None => return false,
        }
    }
}

fn trim_start(data: &[u8]) -> &[u8] {
    let start = data
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .unwrap_or(data.len());
    &data[start..]
}

fn find(data: &[u8], needle: &[u8]) -> Option<usize> {
    data.windows(needle.len()).position(|w| w == needle)
}

/// The end of a `<!DOCTYPE ...>`, past a bracketed internal subset.
fn skip_doctype(data: &[u8]) -> Option<usize> {
    let mut depth = 0usize;
    for (at, c) in data.iter().enumerate() {
        match c {
            b'[' => depth += 1,
            b']' => depth = depth.saturating_sub(1),
            b'>' if depth == 0 => return Some(at + 1),
            _ => {}
        }
    }
    None
}

/// Parsing options that load nothing from outside the document.
fn options() -> Options<'static> {
    Options {
        resources_dir: None,
        image_href_resolver: ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Options::default()
    }
}

/// The document parsed, if it is an SVG within the limits.
fn parse(data: &[u8]) -> Option<Tree> {
    if data.len() > MAX_BYTES || !sniff(data) {
        return None;
    }
    let text = std::str::from_utf8(data).ok()?;
    let parsing = usvg::roxmltree::ParsingOptions {
        allow_dtd: true,
        nodes_limit: MAX_COST as u32,
    };
    let doc = usvg::roxmltree::Document::parse_with_options(text, parsing).ok()?;
    if !within_cost(&doc) {
        return None;
    }
    Tree::from_xmltree(&doc, &options()).ok()
}

/// Whether the document nests at most [`MAX_DEPTH`] deep and draws at most
/// [`MAX_COST`] elements with every `<use>` expanded. A `<use>` that refers
/// to itself through others costs as much as the limit.
fn within_cost(doc: &usvg::roxmltree::Document<'_>) -> bool {
    // Document order visits a parent before its children, so each depth is
    // its parent's plus one, without recursing.
    let mut depths: HashMap<usvg::roxmltree::NodeId, usize> = HashMap::new();
    for node in doc.descendants() {
        let depth = node.parent().map_or(0, |p| depths[&p.id()] + 1);
        if depth > MAX_DEPTH {
            return false;
        }
        depths.insert(node.id(), depth);
    }
    let ids: HashMap<&str, usvg::roxmltree::Node<'_, '_>> = doc
        .descendants()
        .filter_map(|n| Some((n.attribute("id")?, n)))
        .collect();
    let mut costs = HashMap::new();
    cost(doc.root(), &ids, &mut costs, 0) <= MAX_COST
}

/// What `node` draws, in elements, with each `<use>` costing what it refers
/// to. `costs` memoises by node; `None` marks a node being costed, so a
/// reference cycle costs over the limit, as does nesting past [`MAX_DEPTH`]
/// (`level` counts both kinds of step, which bounds the recursion).
fn cost<'a, 'i>(
    node: usvg::roxmltree::Node<'a, 'i>,
    ids: &HashMap<&'a str, usvg::roxmltree::Node<'a, 'i>>,
    costs: &mut HashMap<usvg::roxmltree::NodeId, Option<u64>>,
    level: usize,
) -> u64 {
    match costs.get(&node.id()) {
        Some(Some(known)) => return *known,
        Some(None) => return MAX_COST + 1,
        None => {}
    }
    if level > MAX_DEPTH {
        return MAX_COST + 1;
    }
    costs.insert(node.id(), None);
    let mut total: u64 = 1;
    for child in node.children().filter(|c| c.is_element()) {
        total = total.saturating_add(cost(child, ids, costs, level + 1));
        if total > MAX_COST {
            break;
        }
    }
    if node.tag_name().name() == "use" {
        let href = node
            .attribute((XLINK, "href"))
            .or_else(|| node.attribute("href"));
        if let Some(target) = href
            .and_then(|h| h.strip_prefix('#'))
            .and_then(|id| ids.get(id))
        {
            total = total.saturating_add(cost(*target, ids, costs, level + 1));
        }
    }
    costs.insert(node.id(), Some(total));
    total
}

const XLINK: &str = "http://www.w3.org/1999/xlink";

/// The size an SVG lays out at, in CSS pixels, rounded up; `None` for data
/// that is not an SVG this module will draw.
pub(crate) fn size(data: &[u8]) -> Option<(u32, u32)> {
    let size = parse(data)?.size();
    let width = size.width().ceil();
    let height = size.height().ceil();
    if !(1.0..=f32::from(u16::MAX)).contains(&width)
        || !(1.0..=f32::from(u16::MAX)).contains(&height)
    {
        return None;
    }
    let (width, height) = (width as u32, height as u32);
    (u64::from(width) * u64::from(height) <= MAX_PIXELS).then_some((width, height))
}

/// The bitmap size an SVG of `size` CSS pixels is rendered at:
/// [`RASTER_SCALE`] times larger when that fits [`MAX_PIXELS`].
pub(crate) fn raster_size((width, height): (u32, u32)) -> (u32, u32) {
    let (w, h) = (width * RASTER_SCALE, height * RASTER_SCALE);
    if u64::from(w) * u64::from(h) <= MAX_PIXELS {
        (w, h)
    } else {
        (width, height)
    }
}

/// Draws `data` stretched over `width` x `height` RGBA pixels with straight
/// alpha into `out`. Returns whether every pixel is opaque, or `None` when
/// the data is not an SVG this module draws.
pub(crate) fn decode(data: &[u8], width: u32, height: u32, out: &mut [u8]) -> Option<bool> {
    if u64::from(width) * u64::from(height) > MAX_PIXELS
        || out.len() != width as usize * height as usize * 4
    {
        return None;
    }
    let tree = parse(data)?;
    let mut pixmap = Pixmap::new(width, height)?;
    let size = tree.size();
    let transform =
        Transform::from_scale(width as f32 / size.width(), height as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut opaque = true;
    for (px, dst) in pixmap.pixels().iter().zip(out.chunks_exact_mut(4)) {
        let c = px.demultiply();
        opaque &= c.alpha() == u8::MAX;
        dst.copy_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Some(opaque)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARES: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<!-- two squares -->
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="0 0 2 1">
<rect width="1" height="1" fill="#f00"/><rect x="1" width="1" height="1" fill="#00f" fill-opacity="0.5"/>
</svg>"##;

    #[test]
    fn sniffs_svg_after_a_prolog() {
        assert!(sniff(SQUARES.as_bytes()));
        assert!(sniff(b"\xEF\xBB\xBF <svg/>"));
        assert!(sniff(
            b"<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x\" [<!ENTITY a \"b\">]><svg>"
        ));
        assert!(!sniff(b"<svgx>"));
        assert!(!sniff(b"<html><svg>"));
        assert!(!sniff(b"<!-- never closed <svg>"));
        assert!(!sniff(b"\x89PNG"));
    }

    #[test]
    fn sizes_from_width_and_height_else_the_view_box() {
        assert_eq!(size(SQUARES.as_bytes()), Some((20, 10)));
        assert_eq!(
            size(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30.5 12"/>"#),
            Some((31, 12))
        );
        assert_eq!(raster_size((20, 10)), (40, 20));
        assert_eq!(raster_size((4000, 4000)), (4000, 4000));
    }

    #[test]
    fn renders_scaled_with_straight_alpha() {
        let (w, h) = raster_size(size(SQUARES.as_bytes()).unwrap());
        let mut out = vec![0; w as usize * h as usize * 4];
        assert_eq!(decode(SQUARES.as_bytes(), w, h, &mut out), Some(false));
        let at = |x: usize, y: usize| &out[(y * w as usize + x) * 4..][..4];
        assert_eq!(at(5, 5), &[255, 0, 0, 255]);
        let blue = at(30, 10);
        assert!(
            blue[2] == 255 && blue[0] == 0 && (126..=129).contains(&blue[3]),
            "{blue:?}"
        );
    }

    #[test]
    fn loads_nothing_from_outside() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="4" height="4">
<image width="4" height="4" xlink:href="/etc/passwd"/>
<image width="4" height="4" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'%3E%3Crect width='9' height='9'/%3E%3C/svg%3E"/>
</svg>"#;
        let mut out = vec![0; 4 * 4 * 4];
        assert_eq!(decode(svg, 4, 4, &mut out), Some(false));
        assert!(out.iter().all(|&b| b == 0), "nothing drawn");
    }

    #[test]
    fn refuses_use_bombs_and_deep_nesting() {
        let mut bomb = String::from(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="4" height="4"><defs><rect id="l0" width="1" height="1"/>"#,
        );
        for level in 1..12 {
            bomb.push_str(&format!("<g id=\"l{level}\">"));
            for _ in 0..10 {
                bomb.push_str(&format!("<use xlink:href=\"#l{}\"/>", level - 1));
            }
            bomb.push_str("</g>");
        }
        bomb.push_str("</defs><use xlink:href=\"#l11\"/></svg>");
        assert_eq!(size(bomb.as_bytes()), None);

        let cycle = br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g></svg>"##;
        assert_eq!(size(cycle), None);

        let deep = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">{}{}</svg>"#,
            "<g>".repeat(MAX_DEPTH + 1),
            "</g>".repeat(MAX_DEPTH + 1)
        );
        assert_eq!(size(deep.as_bytes()), None);
        let shallow = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">{}{}</svg>"#,
            "<g>".repeat(60),
            "</g>".repeat(60)
        );
        assert_eq!(size(shallow.as_bytes()), Some((4, 4)));
    }

    #[test]
    fn refuses_oversized_documents_and_bitmaps() {
        let big = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><!--{}--></svg>",
            " ".repeat(MAX_BYTES)
        );
        assert_eq!(size(big.as_bytes()), None);
        let huge = br#"<svg xmlns="http://www.w3.org/2000/svg" width="60000" height="60000"/>"#;
        assert_eq!(size(huge), None);
        let mut out = vec![0; 16];
        assert_eq!(decode(SQUARES.as_bytes(), 1, 1, &mut out), None);
    }

    #[test]
    fn damaged_data_never_panics() {
        // A seeded walk of byte flips, insertions of markup and truncations.
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        const SNIPPETS: &[&[u8]] = &[
            b"<use href=\"#a\"/>",
            b"<g id=\"a\">",
            b"</g>",
            b" viewBox=\"0 0 0 0\"",
            b" width=\"-1\"",
            b"<!DOCTYPE svg [<!ENTITY x \"&x;\">]>",
            b"&x;",
            b"<linearGradient id=\"g\" href=\"#g\"/>",
            b" fill=\"url(#g)\"",
            b"<filter id=\"f\"><feGaussianBlur stdDeviation=\"1e9\"/></filter>",
            b" filter=\"url(#f)\"",
        ];
        for _ in 0..600 {
            let mut data = SQUARES.as_bytes().to_vec();
            for _ in 0..1 + next() % 4 {
                let at = next() as usize % data.len();
                if next() % 2 == 0 {
                    data[at] ^= next() as u8 | 1;
                } else {
                    let snippet = SNIPPETS[next() as usize % SNIPPETS.len()];
                    data.splice(at..at, snippet.iter().copied());
                }
            }
            data.truncate(1 + next() as usize % data.len());
            if let Some(px) = size(&data) {
                let (w, h) = raster_size(px);
                if u64::from(w) * u64::from(h) <= 1 << 16 {
                    let mut out = vec![0; w as usize * h as usize * 4];
                    let _ = decode(&data, w, h, &mut out);
                }
            }
        }
    }
}
