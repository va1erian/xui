#![forbid(unsafe_code)]

//! PNG and JPEG decoding for NetSurf's image handlers (`csrc/nsx_image.c`):
//! the header first, for the size NetSurf lays the page out with, then the
//! pixels when the image is first drawn, straight into NetSurf's bitmap.
//!
//! Image data comes from the network, so both steps refuse more than
//! [`MAX_PIXELS`] and treat every decoder error as "no image"; the `sys`
//! layer also stops a panic at the FFI boundary.

use png::{BitDepth, ColorType, Transformations};
use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;

/// The most pixels one image may have: 64 megapixels, a 256 MiB bitmap.
/// Anything larger is a decompression bomb, not a web page image.
pub(crate) const MAX_PIXELS: u64 = 64 * 1024 * 1024;

const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Png,
    Jpeg,
}

/// The format by signature: servers mislabel images often enough that the
/// content type is not trusted.
fn format(data: &[u8]) -> Option<Format> {
    if data.starts_with(PNG_SIGNATURE) {
        Some(Format::Png)
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Format::Jpeg)
    } else {
        None
    }
}

fn within_limit(width: u32, height: u32) -> bool {
    width > 0 && height > 0 && u64::from(width) * u64::from(height) <= MAX_PIXELS
}

fn png_decoder(data: &[u8]) -> png::Decoder<&[u8]> {
    // The decoder's own working memory, on top of the bitmap.
    let limits = png::Limits {
        bytes: (MAX_PIXELS * 8) as usize,
    };
    let mut decoder = png::Decoder::new_with_limits(data, limits);
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    decoder
}

fn jpeg_decoder(data: &[u8]) -> JpegDecoder<ZCursor<&[u8]>> {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    JpegDecoder::new_with_options(ZCursor::new(data), options)
}

/// The image's size from its header, if it is a PNG or JPEG within
/// [`MAX_PIXELS`].
pub(crate) fn size(data: &[u8]) -> Option<(u32, u32)> {
    let (width, height) = match format(data)? {
        Format::Png => {
            let reader = png_decoder(data).read_info().ok()?;
            let info = reader.info();
            (info.width, info.height)
        }
        Format::Jpeg => {
            let mut decoder = jpeg_decoder(data);
            decoder.decode_headers().ok()?;
            let (w, h) = decoder.dimensions()?;
            (u32::try_from(w).ok()?, u32::try_from(h).ok()?)
        }
    };
    within_limit(width, height).then_some((width, height))
}

/// Decodes `data` into `out`: `width * height` RGBA pixels with straight
/// alpha. The size must be what [`size`] said. Returns whether every pixel
/// is opaque, or `None` when the data does not decode.
pub(crate) fn decode(data: &[u8], width: u32, height: u32, out: &mut [u8]) -> Option<bool> {
    let expected = usize::try_from(u64::from(width) * u64::from(height) * 4).ok()?;
    if !within_limit(width, height) || out.len() != expected || size(data)? != (width, height) {
        return None;
    }
    match format(data)? {
        Format::Png => decode_png(data, out),
        Format::Jpeg => {
            jpeg_decoder(data).decode_into(out).ok()?;
            Some(true)
        }
    }
}

fn decode_png(data: &[u8], out: &mut [u8]) -> Option<bool> {
    let mut reader = png_decoder(data).read_info().ok()?;
    let (color, depth) = reader.output_color_type();
    if depth != BitDepth::Eight {
        return None;
    }
    let channels = match color {
        ColorType::Grayscale => 1,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
        ColorType::Indexed => return None,
    };
    let mut buf = vec![0; reader.output_buffer_size()];
    // A truncated or damaged stream still shows the rows that decoded, as
    // browsers do; the rest stays transparent.
    if let Err(e) = reader.next_frame(&mut buf) {
        log::debug!("xui-netsurf: partial PNG: {e}");
    }
    let pixels = out.len() / 4;
    if buf.len() < pixels * channels {
        return None;
    }
    let mut opaque = true;
    for (src, dst) in buf.chunks_exact(channels).zip(out.chunks_exact_mut(4)) {
        let rgba = match channels {
            1 => [src[0], src[0], src[0], 255],
            2 => [src[0], src[0], src[0], src[1]],
            3 => [src[0], src[1], src[2], 255],
            _ => [src[0], src[1], src[2], src[3]],
        };
        opaque &= rgba[3] == 255;
        dst.copy_from_slice(&rgba);
    }
    Some(opaque)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A `width` x `height` PNG of one colour.
    pub(crate) fn png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        let pixels: Vec<u8> = rgba
            .iter()
            .copied()
            .cycle()
            .take(width as usize * height as usize * 4)
            .collect();
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        bytes
    }

    #[test]
    fn decodes_a_png() {
        let data = png(3, 2, [10, 20, 30, 128]);
        assert_eq!(size(&data), Some((3, 2)));
        let mut out = vec![0; 3 * 2 * 4];
        assert_eq!(decode(&data, 3, 2, &mut out), Some(false));
        assert_eq!(&out[..4], &[10, 20, 30, 128]);
    }

    #[test]
    fn an_opaque_png_says_so() {
        let data = png(2, 2, [1, 2, 3, 255]);
        let mut out = vec![0; 16];
        assert_eq!(decode(&data, 2, 2, &mut out), Some(true));
    }

    #[test]
    fn refuses_a_png_bomb() {
        // A header claiming 100000 x 100000 pixels: the size alone is refused.
        let mut data = png(1, 1, [0; 4]);
        data[16..20].copy_from_slice(&100_000u32.to_be_bytes());
        data[20..24].copy_from_slice(&100_000u32.to_be_bytes());
        // Fix the IHDR CRC so the decoder reaches the size check.
        let crc = crc32(&data[12..29]);
        data[29..33].copy_from_slice(&crc.to_be_bytes());
        assert_eq!(size(&data), None);
    }

    fn jpeg(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        let pixels = ::image::RgbImage::from_pixel(width, height, ::image::Rgb(rgb));
        let mut bytes = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgb8(pixels)
            .write_to(&mut bytes, ::image::ImageFormat::Jpeg)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn decodes_a_jpeg() {
        let data = jpeg(5, 3, [0, 200, 0]);
        assert_eq!(size(&data), Some((5, 3)));
        let mut out = vec![0; 5 * 3 * 4];
        assert_eq!(decode(&data, 5, 3, &mut out), Some(true));
        assert!(
            out[1] > 180 && out[0] < 40 && out[3] == 255,
            "{:?}",
            &out[..4]
        );
    }

    #[test]
    fn damaged_data_never_panics() {
        // A seeded walk of byte flips and truncations over both formats.
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for original in [png(9, 7, [9, 8, 7, 6]), jpeg(9, 7, [200, 100, 50])] {
            for _ in 0..300 {
                let mut data = original.clone();
                for _ in 0..1 + next() % 4 {
                    let at = next() as usize % data.len();
                    data[at] ^= next() as u8 | 1;
                }
                data.truncate(1 + next() as usize % data.len());
                // A flipped JPEG size is legal up to MAX_PIXELS; keep the
                // test's memory small.
                if let Some((w, h)) = size(&data).filter(|(w, h)| w * h <= 1 << 16) {
                    let mut out = vec![0; w as usize * h as usize * 4];
                    let _ = decode(&data, w, h, &mut out);
                }
            }
        }
    }

    #[test]
    fn refuses_garbage_and_mismatched_sizes() {
        assert_eq!(size(b""), None);
        assert_eq!(size(b"GIF89a"), None);
        assert_eq!(size(&[0xFF, 0xD8, 0xFF, 0x00, 0x01]), None);
        assert_eq!(size(PNG_SIGNATURE), None);
        let data = png(2, 2, [0; 4]);
        let mut out = vec![0; 16];
        assert_eq!(decode(&data, 4, 1, &mut out), None);
        assert_eq!(decode(&data[..40], 2, 2, &mut out), None);
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
}
