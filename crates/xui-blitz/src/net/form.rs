//! A submitted form's entries as a request body, in the encoding its
//! `enctype` names.

use blitz_traits::net::{EntryValue, FormData};

/// The body and `Content-Type` for `form` sent as `enctype`.
pub(crate) fn encode(form: &FormData, enctype: Option<&str>) -> (Vec<u8>, String) {
    match enctype.map(str::trim) {
        Some(t) if t.eq_ignore_ascii_case("multipart/form-data") => multipart(form),
        Some(t) if t.eq_ignore_ascii_case("text/plain") => {
            let mut body = String::new();
            for entry in form.iter() {
                body.push_str(&entry.name);
                body.push('=');
                body.push_str(entry.value.as_ref());
                body.push_str("\r\n");
            }
            (body.into_bytes(), "text/plain;charset=UTF-8".to_string())
        }
        _ => {
            let mut body = url::form_urlencoded::Serializer::new(String::new());
            for entry in form.iter() {
                body.append_pair(&entry.name, entry.value.as_ref());
            }
            (
                body.finish().into_bytes(),
                "application/x-www-form-urlencoded".to_string(),
            )
        }
    }
}

/// `multipart/form-data`; a file entry carries the file's bytes.
fn multipart(form: &FormData) -> (Vec<u8>, String) {
    let boundary = format!("----xui-blitz-{:016x}", boundary_seed(form));
    let mut body = Vec::new();
    for entry in form.iter() {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        let name = quote(&entry.name);
        match &entry.value {
            EntryValue::String(value) => {
                body.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
                );
                body.extend_from_slice(value.as_bytes());
            }
            EntryValue::File(path) => {
                let filename = path
                    .file_name()
                    .map(|f| quote(&f.to_string_lossy()))
                    .unwrap_or_default();
                body.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n\
                         Content-Type: application/octet-stream\r\n\r\n"
                    )
                    .as_bytes(),
                );
                match std::fs::read(path) {
                    Ok(bytes) => body.extend_from_slice(&bytes),
                    Err(e) => log::warn!("xui-blitz: form file {}: {e}", path.display()),
                }
            }
            EntryValue::EmptyFile => body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{name}\"; filename=\"\"\r\n\
                     Content-Type: application/octet-stream\r\n\r\n"
                )
                .as_bytes(),
            ),
        }
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

/// Escapes a name for a quoted `Content-Disposition` parameter.
fn quote(s: &str) -> String {
    s.replace('"', "%22")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Varies the boundary with the form and the time, so a value is unlikely to
/// contain it.
fn boundary_seed(form: &FormData) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for entry in form.iter() {
        entry.name.hash(&mut h);
        entry.value.as_ref().hash(&mut h);
    }
    std::time::SystemTime::now().hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use blitz_traits::net::Entry;

    fn form() -> FormData {
        FormData(vec![
            Entry {
                name: "q".into(),
                value: "a b&c".into(),
            },
            Entry {
                name: "lang".into(),
                value: "en".into(),
            },
        ])
    }

    #[test]
    fn url_encoded_is_the_default() {
        let (body, ty) = encode(&form(), None);
        assert_eq!(body, b"q=a+b%26c&lang=en");
        assert_eq!(ty, "application/x-www-form-urlencoded");
    }

    #[test]
    fn multipart_names_its_boundary() {
        let (body, ty) = encode(&form(), Some("multipart/form-data"));
        let boundary = ty.split("boundary=").nth(1).unwrap();
        let body = String::from_utf8(body).unwrap();
        assert!(body.starts_with(&format!("--{boundary}\r\n")));
        assert!(body.contains("name=\"q\"\r\n\r\na b&c\r\n"));
        assert!(body.ends_with(&format!("--{boundary}--\r\n")));
    }
}
