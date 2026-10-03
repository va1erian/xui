#![forbid(unsafe_code)]

//! Standard base64 (RFC 4648, with padding): enough for embedded images.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes `bytes` as padded base64.
pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn sextet(c: u8) -> Option<u32> {
    ALPHABET.iter().position(|&a| a == c).map(|i| i as u32)
}

/// Decodes padded base64; `None` for any character outside the alphabet,
/// misplaced padding or a length that is not a multiple of four.
pub(crate) fn decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let chunks = bytes.len() / 4;
    for (index, chunk) in bytes.chunks(4).enumerate() {
        let padding = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if padding > 2 || (padding > 0 && index + 1 != chunks) {
            return None;
        }
        let mut n = 0u32;
        for &c in &chunk[..4 - padding] {
            n = n << 6 | sextet(c)?;
        }
        n <<= 6 * padding as u32;
        for i in 0..3 - padding {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_rfc_vectors() {
        for (plain, coded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode(plain.as_bytes()), coded);
            assert_eq!(decode(coded).unwrap(), plain.as_bytes());
        }
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in ["Zg=", "Z", "Zg==Zm9v", "Zm9!", "=Zg=", "Z===", "Zm 9"] {
            assert_eq!(decode(bad), None, "{bad}");
        }
    }

    #[test]
    fn round_trips_every_byte() {
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(decode(&encode(&bytes)).unwrap(), bytes);
    }
}
