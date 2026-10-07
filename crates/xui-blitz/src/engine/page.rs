//! A response as the HTML Blitz parses: pages as they are, text and images
//! wrapped in a page of their own, and the error page.

use crate::net::Loaded;

/// Whether the view shows a response of MIME type `mime` (lower case, without
/// parameters) rather than downloading it. An absent type is taken as HTML.
pub(crate) fn is_displayable(mime: &str) -> bool {
    mime.is_empty()
        || is_html(mime)
        || is_text(mime)
        || (mime.starts_with("image/") && !mime.contains("icon"))
}

fn is_html(mime: &str) -> bool {
    matches!(mime, "text/html" | "application/xhtml+xml")
}

fn is_text(mime: &str) -> bool {
    mime.starts_with("text/") || matches!(mime, "application/json" | "application/xml")
}

/// The HTML to show for `loaded`.
pub(crate) fn html_for(loaded: &Loaded) -> String {
    let content_type = loaded.header("Content-Type").unwrap_or("");
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime == "application/xhtml+xml" {
        decode(&loaded.body, content_type)
    } else if mime.is_empty() || is_html(&mime) {
        as_html(decode(&loaded.body, content_type))
    } else if is_text(&mime) {
        format!(
            "<!DOCTYPE html><html><body><pre style=\"white-space: pre-wrap; word-wrap: break-word\">{}</pre></body></html>",
            escape(&decode(&loaded.body, content_type))
        )
    } else if mime.starts_with("image/") {
        format!(
            "<!DOCTYPE html><html><body style=\"margin: 0\"><img src=\"{}\" alt=\"\"></body></html>",
            escape(&loaded.url)
        )
    } else {
        error_page(
            &loaded.url,
            &format!("This is {mime}, which cannot be shown."),
        )
    }
}

/// `text` as Blitz should parse it when it is `text/html`: as HTML, whatever
/// its doctype says.
///
/// Blitz switches to its XML parser when the text starts with `<?xml`, an
/// XHTML `<!DOCTYPE` or `<html xmlns="http://www.w3.org/1999/xhtml">` (as
/// most HTML mail does), and the XML parser leaves elements without the HTML
/// namespace unstyled. HTML ignores a comment before the doctype, and the
/// comment stops the sniffing.
pub(crate) fn as_html(text: String) -> String {
    let start = text.trim_start_matches('\u{feff}').trim_start();
    if start.starts_with("<?xml") || start.starts_with("<!DOCTYPE") || start.starts_with("<html") {
        format!("<!-- xui-blitz: text/html -->{text}")
    } else {
        text
    }
}

/// A page saying `url` could not be opened, and why.
pub(crate) fn error_page(url: &str, message: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><title>Problem loading page</title></head>\
         <body style=\"font-family: sans-serif; margin: 2em\">\
         <h1 style=\"font-size: 1.4em\">The page could not be opened</h1>\
         <p style=\"word-wrap: break-word\"><code>{}</code></p><p>{}</p></body></html>",
        escape(url),
        escape(message)
    )
}

/// Text as HTML character data.
pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// The body as text: Latin-1 when the header or an early `<meta>` says so
/// (`windows-1252` read as Latin-1 too), else UTF-8.
fn decode(body: &[u8], content_type: &str) -> String {
    let head = String::from_utf8_lossy(&body[..body.len().min(1024)]).to_ascii_lowercase();
    let declared = charset(&content_type.to_ascii_lowercase())
        .or_else(|| charset(&head))
        .unwrap_or_default();
    match declared.as_str() {
        "iso-8859-1" | "latin1" | "windows-1252" | "us-ascii" | "ascii" => {
            body.iter().map(|&b| b as char).collect()
        }
        _ => String::from_utf8_lossy(body).into_owned(),
    }
}

/// The value after the first `charset=` in `s`.
fn charset(s: &str) -> Option<String> {
    let at = s.find("charset=")? + "charset=".len();
    let value: String = s[at..]
        .trim_start_matches(['"', '\''])
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded(ty: &str, body: &[u8]) -> Loaded {
        Loaded {
            url: "http://example.com/x".into(),
            status: 200,
            headers: vec![("Content-Type".into(), ty.into())],
            body: body.to_vec(),
        }
    }

    #[test]
    fn html_is_shown_as_it_is() {
        assert_eq!(html_for(&loaded("text/html", b"<p>hi")), "<p>hi");
    }

    #[test]
    fn xhtml_doctypes_served_as_html_are_parsed_as_html() {
        let page = "<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\">\n<p>x";
        assert!(html_for(&loaded("text/html", page.as_bytes())).starts_with("<!--"));
        assert_eq!(
            html_for(&loaded("application/xhtml+xml", page.as_bytes())),
            page
        );
        assert_eq!(as_html("<!doctype html><p>".into()), "<!doctype html><p>");
    }

    #[test]
    fn plain_text_is_escaped_into_a_pre() {
        let html = html_for(&loaded("text/plain", b"a < b"));
        assert!(html.contains("<pre"));
        assert!(html.contains("a &lt; b"));
    }

    #[test]
    fn latin1_is_decoded_from_the_header_or_a_meta() {
        assert_eq!(
            html_for(&loaded("text/html; charset=ISO-8859-1", b"caf\xe9")),
            "café"
        );
        let body = b"<meta charset=\"windows-1252\"><p>na\xefve";
        assert!(html_for(&loaded("text/html", body)).ends_with("naïve"));
    }

    #[test]
    fn images_are_wrapped_and_archives_are_not_shown() {
        assert!(html_for(&loaded("image/png", b"")).contains("<img src=\"http://example.com/x\""));
        assert!(is_displayable("image/png"));
        assert!(!is_displayable("application/zip"));
        assert!(is_displayable(""));
    }
}
