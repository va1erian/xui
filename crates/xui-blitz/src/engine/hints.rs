//! The legacy presentational attributes Blitz does not map itself, as a
//! user-agent style sheet made for each document.
//!
//! Blitz turns `bgcolor`, `width`, `height`, `align`, `hspace`/`vspace` and
//! the body margins into style, but not body `text`/`link`/`background`,
//! `<font color size face>` or table `cellpadding`/`cellspacing`/`border`,
//! which old sites and most HTML mail lean on (NetSurf's equivalent is
//! `content/handlers/css/hints.c`). After a page is parsed, [`style_sheet`]
//! collects the values it uses and writes one attribute-selector rule per
//! value. As a user-agent sheet it loses to every author rule, as the HTML
//! spec's presentational hints do.
//!
//! Values reach the CSS only after checking: colours and numbers by their
//! characters, names and URLs quoted, so a page cannot inject rules.

use std::collections::BTreeSet;

use blitz_dom::BaseDocument;

/// The sheet for `doc`'s legacy attributes, or `None` when it uses none.
pub(crate) fn style_sheet(doc: &BaseDocument) -> Option<String> {
    let mut rules = BTreeSet::new();
    for (_, node) in doc.tree().iter() {
        let Some(el) = node.element_data() else {
            continue;
        };
        let tag = el.name.local.as_ref();
        for attr in el.attrs() {
            let value = attr.value.trim();
            if let Some(rule) = rule(doc, tag, attr.name.local.as_ref(), value) {
                rules.insert(rule);
            }
        }
    }
    (!rules.is_empty()).then(|| rules.into_iter().collect::<Vec<_>>().join("\n"))
}

/// The rule for one attribute, if it is one Blitz leaves unmapped.
fn rule(doc: &BaseDocument, tag: &str, name: &str, value: &str) -> Option<String> {
    let sel = |t: &str| format!("{t}[{name}=\"{}\" i]", quote(value));
    let cells = |t: &str| {
        let s = sel(t);
        format!("{s} > tr > td, {s} > tr > th, {s} > * > tr > td, {s} > * > tr > th")
    };
    Some(match (tag, name) {
        ("body", "text") => format!("{} {{ color: {} }}", sel("body"), color(value)?),
        ("body", "link") => format!("{} a[href] {{ color: {} }}", sel("body"), color(value)?),
        ("font", "color") => format!("{} {{ color: {} }}", sel("font"), color(value)?),
        ("font", "size") => format!("{} {{ font-size: {} }}", sel("font"), font_size(value)?),
        ("font", "face") => format!("{} {{ font-family: {} }}", sel("font"), families(value)?),
        ("body" | "table" | "td" | "th" | "tr", "background") => {
            let url = doc.url().join(value).ok()?;
            format!(
                "{} {{ background-image: url(\"{}\") }}",
                sel(tag),
                quote(url.as_str())
            )
        }
        ("table", "cellpadding") => {
            format!("{} {{ padding: {}px }}", cells("table"), pixels(value)?)
        }
        ("table", "cellspacing") => {
            format!(
                "{} {{ border-spacing: {}px }}",
                sel("table"),
                pixels(value)?
            )
        }
        ("table", "border") => {
            let width = pixels(value).unwrap_or(1);
            if width == 0 {
                return None;
            }
            format!(
                "{} {{ border: {width}px outset gray }}\n{} {{ border: 1px inset gray }}",
                sel("table"),
                cells("table")
            )
        }
        ("td" | "th", "nowrap") => format!("{tag}[nowrap] {{ white-space: nowrap }}"),
        _ => return None,
    })
}

/// `s` inside a double-quoted CSS string.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            // A newline ends a CSS string; write it as an escape.
            '\n' | '\r' | '\x0c' => out.push_str("\\a "),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// A legacy colour value (`#rgb`, `#rrggbb`, bare hex digits, or a colour
/// name) as CSS.
fn color(value: &str) -> Option<String> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("#{hex}"))
    } else if !value.is_empty() && value.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(value.to_ascii_lowercase())
    } else {
        None
    }
}

/// `<font size>`: 1 to 7, or relative to 3 with a sign.
fn font_size(value: &str) -> Option<&'static str> {
    const SIZES: [&str; 7] = [
        "x-small", "small", "medium", "large", "x-large", "xx-large", "48px",
    ];
    let n: i32 = value.trim_start_matches('+').parse().ok()?;
    let size = if value.starts_with(['+', '-']) {
        3 + n
    } else {
        n
    };
    Some(SIZES[(size.clamp(1, 7) - 1) as usize])
}

/// `<font face>`: a comma-separated list, each name quoted.
fn families(value: &str) -> Option<String> {
    let names: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| format!("\"{}\"", quote(n.trim_matches(['"', '\'']))))
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// A whole number of pixels (`4`, `4px`).
fn pixels(value: &str) -> Option<u32> {
    value.trim_end_matches("px").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_checked() {
        assert_eq!(color("#FFFF00").as_deref(), Some("#FFFF00"));
        assert_eq!(color("ff9").as_deref(), Some("#ff9"));
        assert_eq!(color("Navy").as_deref(), Some("navy"));
        assert_eq!(color("red; } body { x"), None);
    }

    #[test]
    fn font_sizes_are_absolute_or_relative_to_three() {
        assert_eq!(font_size("1"), Some("x-small"));
        assert_eq!(font_size("+1"), Some("large"));
        assert_eq!(font_size("-2"), Some("x-small"));
        assert_eq!(font_size("9"), Some("48px"));
        assert_eq!(font_size("big"), None);
    }

    #[test]
    fn names_and_values_cannot_break_out_of_their_strings() {
        assert_eq!(
            families("Verdana, 'Comic Sans MS'").as_deref(),
            Some("\"Verdana\", \"Comic Sans MS\"")
        );
        assert_eq!(quote("a\"} * {color:red}"), "a\\\"} * {color:red}");
        assert_eq!(pixels("4px"), Some(4));
        assert_eq!(pixels("4;"), None);
    }
}
