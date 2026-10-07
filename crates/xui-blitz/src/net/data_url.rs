//! `data:` URLs (RFC 2397): `data:[<mime>][;base64],<data>`.

/// The MIME type and bytes of a `data:` URL, or `None` when it is malformed.
/// `url` is the whole URL, scheme included.
pub(crate) fn decode(url: &str) -> Option<(String, Vec<u8>)> {
    let rest = url.get(..5)?.eq_ignore_ascii_case("data:").then(|| &url[5..])?;
    let (meta, data) = rest.split_once(',')?;
    // A fragment is not part of the data.
    let data = data.split_once('#').map_or(data, |(d, _)| d);
    let (mime, base64) = match meta.rsplit_once(';') {
        Some((mime, b)) if b.trim().eq_ignore_ascii_case("base64") => (mime, true),
        _ => (meta, false),
    };
    let mime = match mime.trim() {
        "" => "text/plain;charset=US-ASCII".to_string(),
        m if m.starts_with(';') => format!("text/plain{m}"),
        m => m.to_string(),
    };
    let bytes = percent_decode(data);
    let bytes = if base64 { base64_decode(&bytes)? } else { bytes };
    Some((mime, bytes))
}

fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        match (b[i], b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(hi), Some(lo)) => {
                out.push((hi * 16 + lo) as u8);
                i += 3;
            }
            (c, ..) => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Standard or URL-safe base64, padding optional, whitespace ignored.
fn base64_decode(input: &[u8]) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in input {
        if c == b'=' {
            break;
        }
        if c.is_ascii_whitespace() {
            continue;
        }
        acc = (acc << 6) | value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn plain_text_is_percent_decoded() {
        assert_eq!(
            decode("data:,Hello%2C%20World!"),
            Some(("text/plain;charset=US-ASCII".into(), b"Hello, World!".to_vec()))
        );
    }

    #[test]
    fn base64_with_a_type() {
        assert_eq!(
            decode("data:text/html;base64,PHA+aGk8L3A+"),
            Some(("text/html".into(), b"<p>hi</p>".to_vec()))
        );
    }

    #[test]
    fn parameters_stay_with_the_type() {
        let (mime, body) = decode("data:text/html;charset=utf-8,%3Cb%3E").unwrap();
        assert_eq!(mime, "text/html;charset=utf-8");
        assert_eq!(body, b"<b>");
    }

    #[test]
    fn malformed_urls_are_refused() {
        assert_eq!(decode("data:text/plain"), None);
        assert_eq!(decode("http://x/,y"), None);
        assert_eq!(decode("data:;base64,!!!!"), None);
    }
}
