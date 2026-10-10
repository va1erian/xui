#![forbid(unsafe_code)]

//! A portable RGBA8 bitmap: PNG and JPEG decoding plus a bilinear resample.
//!
//! The type carries no platform handles, so an image decoded on a worker thread
//! can be handed straight to a backend's [`Canvas::draw_image`].
//!
//! ```
//! # let png_bytes: &[u8] = &[];
//! if let Ok(image) = xui_core::image::Image::decode(png_bytes) {
//!     let thumb = image.resized(64, 64).unwrap();
//!     assert_eq!(thumb.size(), (64, 64));
//! }
//! ```
//!
//! [`Canvas::draw_image`]: crate::backend::Canvas::draw_image

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Issues image identities: process-wide and unique per build of an image, so
/// a backend can key its decoded-image cache by one.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// An RGBA8 bitmap, row-major with the origin at the top-left.
///
/// An image never changes after construction and a clone copies the identity
/// with the pixels, so equal identities mean equal pixels; see [`Image::id`].
#[derive(Clone)]
pub struct Image {
    id: u64,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// Why an image could not be built or decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
    /// The dimensions are empty or the pixel buffer does not match them.
    Size,
    /// The bytes are not a valid image of a supported format.
    Decode(String),
    /// The pixels could not be encoded as a PNG.
    Encode(String),
    /// A file could not be written.
    Io(String),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::Size => f.write_str("pixel buffer does not match the image size"),
            ImageError::Decode(message) => write!(f, "could not decode image: {message}"),
            ImageError::Encode(message) => write!(f, "could not encode image: {message}"),
            ImageError::Io(message) => write!(f, "could not write image: {message}"),
        }
    }
}

impl std::error::Error for ImageError {}

/// A fallible image operation.
pub type Result<T> = std::result::Result<T, ImageError>;

impl Image {
    /// Wraps tightly packed RGBA8 `pixels` (`width * height * 4` bytes,
    /// row-major, top-down), validating the length.
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Result<Image> {
        if width == 0 || height == 0 || pixels.len() != width as usize * height as usize * 4 {
            return Err(ImageError::Size);
        }
        Ok(Image {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            width,
            height,
            pixels,
        })
    }

    /// An identity for this image's pixels: unique per build, kept by clones.
    ///
    /// Backends cache the decoded (and uploaded) form of an image keyed by
    /// this, so drawing the same image on every repaint — a row icon, a tile's
    /// art — costs one cache lookup instead of a fresh decode. Equal ids mean
    /// equal pixels; distinct ids say nothing (two separately built images with
    /// the same pixels get different ids).
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The size as `(width, height)`.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The tightly packed RGBA8 pixels, row-major and top-down.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Gives the pixel buffer back, for a producer that makes one image per
    /// frame to fill the same allocation again instead of allocating (and
    /// faulting in) a new one each time.
    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }

    /// The RGBA value at `(x, y)`, if in bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = ((y * self.width + x) * 4) as usize;
        self.pixels
            .get(at..at + 4)
            .map(|p| [p[0], p[1], p[2], p[3]])
    }

    /// Encodes the image as an 8-bit RGBA PNG.
    ///
    /// The bytes depend only on the pixels, so equal images encode to equal
    /// bytes.
    pub fn encode_png(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| ImageError::Encode(error.to_string()))?;
        writer
            .write_image_data(&self.pixels)
            .map_err(|error| ImageError::Encode(error.to_string()))?;
        writer
            .finish()
            .map_err(|error| ImageError::Encode(error.to_string()))?;
        Ok(bytes)
    }

    /// Writes the image to `path` as a PNG, creating or replacing the file.
    /// The directory must already exist.
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let bytes = self.encode_png()?;
        std::fs::write(path, bytes).map_err(|error| ImageError::Io(error.to_string()))
    }

    /// Decodes `bytes` as PNG or JPEG, chosen by the format's signature.
    pub fn decode(bytes: &[u8]) -> Result<Image> {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            Image::decode_png(bytes)
        } else if bytes.starts_with(&[0xFF, 0xD8]) {
            Image::decode_jpeg(bytes)
        } else {
            Err(ImageError::Decode("unrecognised image format".into()))
        }
    }

    /// Decodes a PNG into RGBA. Palette, greyscale and 16-bit inputs are
    /// normalised to 8-bit RGBA.
    pub fn decode_png(bytes: &[u8]) -> Result<Image> {
        let mut decoder = png::Decoder::new(bytes);
        decoder.set_transformations(
            png::Transformations::EXPAND
                | png::Transformations::ALPHA
                | png::Transformations::STRIP_16,
        );
        let mut reader = decoder
            .read_info()
            .map_err(|error| ImageError::Decode(error.to_string()))?;
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut buffer)
            .map_err(|error| ImageError::Decode(error.to_string()))?;
        rgba_from_png(
            &buffer[..info.buffer_size()],
            info.color_type,
            info.width,
            info.height,
        )
    }

    /// Decodes a baseline or progressive JPEG into RGBA.
    pub fn decode_jpeg(bytes: &[u8]) -> Result<Image> {
        use zune_jpeg::zune_core::bytestream::ZCursor;
        use zune_jpeg::zune_core::colorspace::ColorSpace;
        use zune_jpeg::zune_core::options::DecoderOptions;

        let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
        let mut decoder = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), options);
        let pixels = decoder
            .decode()
            .map_err(|error| ImageError::Decode(error.to_string()))?;
        let info = decoder
            .info()
            .ok_or_else(|| ImageError::Decode("no image header".into()))?;
        Image::from_rgba(info.width as u32, info.height as u32, pixels)
    }

    /// Resamples the image to `width` x `height` pixels with bilinear
    /// interpolation. The aspect ratio is not preserved.
    pub fn resized(&self, width: u32, height: u32) -> Result<Image> {
        if width == 0 || height == 0 {
            return Err(ImageError::Size);
        }
        if (width, height) == (self.width, self.height) {
            return Ok(self.clone());
        }
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let scale_x = self.width as f32 / width as f32;
        let scale_y = self.height as f32 / height as f32;
        for y in 0..height {
            for x in 0..width {
                let source_x = (x as f32 + 0.5) * scale_x - 0.5;
                let source_y = (y as f32 + 0.5) * scale_y - 0.5;
                let sample = self.bilinear(source_x, source_y);
                let at = ((y * width + x) * 4) as usize;
                pixels[at..at + 4].copy_from_slice(&sample);
            }
        }
        Image::from_rgba(width, height, pixels)
    }

    /// Samples a texel with bilinear interpolation, clamping at the edges.
    fn bilinear(&self, x: f32, y: f32) -> [u8; 4] {
        let x = x.clamp(0.0, (self.width - 1) as f32);
        let y = y.clamp(0.0, (self.height - 1) as f32);
        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = x - x0 as f32;
        let ty = y - y0 as f32;
        let top_left = self.pixel(x0, y0).unwrap_or([0; 4]);
        let top_right = self.pixel(x1, y0).unwrap_or([0; 4]);
        let bottom_left = self.pixel(x0, y1).unwrap_or([0; 4]);
        let bottom_right = self.pixel(x1, y1).unwrap_or([0; 4]);
        let mut out = [0u8; 4];
        for channel in 0..4 {
            let top = mix(top_left[channel], top_right[channel], tx);
            let bottom = mix(bottom_left[channel], bottom_right[channel], tx);
            out[channel] = mix(top, bottom, ty);
        }
        out
    }
}

/// Two images are equal by their pixels; the identity is not part of it, so
/// two separately built images with the same pixels stay equal.
impl PartialEq for Image {
    fn eq(&self, other: &Image) -> bool {
        self.width == other.width && self.height == other.height && self.pixels == other.pixels
    }
}

impl Eq for Image {}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pixels", &self.pixels)
            .finish()
    }
}

fn mix(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// Converts one decoded PNG frame to RGBA8.
fn rgba_from_png(data: &[u8], color: png::ColorType, width: u32, height: u32) -> Result<Image> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    match color {
        png::ColorType::Rgba => pixels.extend_from_slice(data),
        png::ColorType::Rgb => {
            for chunk in data.as_chunks::<3>().0 {
                pixels.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for chunk in data.as_chunks::<2>().0 {
                pixels.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
        }
        png::ColorType::Grayscale => {
            for value in data {
                pixels.extend_from_slice(&[*value, *value, *value, 255]);
            }
        }
        png::ColorType::Indexed => {
            return Err(ImageError::Decode("indexed PNG was not expanded".into()));
        }
    }
    Image::from_rgba(width, height, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encodes `pixels` as an 8-bit RGBA PNG in memory.
    fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        Image::from_rgba(width, height, pixels.to_vec())
            .unwrap()
            .encode_png()
            .unwrap()
    }

    #[test]
    fn png_decode_and_resize_round_trip() {
        let pixels = [
            255, 0, 0, 255, 0, 255, 0, 255, //
            0, 0, 255, 255, 255, 255, 0, 255,
        ];
        let encoded = encode_png(2, 2, &pixels);

        let decoded = Image::decode_png(&encoded).expect("decode");
        assert_eq!(decoded.size(), (2, 2));
        assert_eq!(decoded.pixel(0, 0), Some([255, 0, 0, 255]));
        assert_eq!(decoded.pixel(1, 1), Some([255, 255, 0, 255]));

        let shrunk = decoded.resized(1, 1).expect("resize");
        assert_eq!(shrunk.size(), (1, 1));
        assert_eq!(shrunk.pixels().len(), 4);

        let grown = decoded.resized(4, 4).expect("resize up");
        assert_eq!(grown.size(), (4, 4));
        assert_eq!(grown.pixels().len(), 4 * 4 * 4);
    }

    #[test]
    fn decode_sniffs_the_png_signature() {
        let encoded = encode_png(1, 1, &[1, 2, 3, 255]);
        assert_eq!(
            Image::decode(&encoded).unwrap().pixel(0, 0),
            Some([1, 2, 3, 255])
        );
    }

    #[test]
    fn a_resized_solid_image_keeps_its_colour() {
        let solid = Image::from_rgba(2, 2, [10, 20, 30, 255].repeat(4)).unwrap();
        let scaled = solid.resized(5, 3).unwrap();
        assert_eq!(scaled.pixel(2, 1), Some([10, 20, 30, 255]));
    }

    #[test]
    fn into_pixels_returns_the_buffer_it_was_built_from() {
        let pixels = vec![1, 2, 3, 255, 4, 5, 6, 255];
        let at = pixels.as_ptr();
        let image = Image::from_rgba(2, 1, pixels).unwrap();
        let back = image.into_pixels();
        assert_eq!(back, [1, 2, 3, 255, 4, 5, 6, 255]);
        assert_eq!(back.as_ptr(), at, "the same allocation, not a copy");
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        assert_eq!(Image::from_rgba(1, 1, vec![0; 3]), Err(ImageError::Size));
        assert!(Image::decode_png(b"not a png").is_err());
        assert!(Image::decode(b"not an image").is_err());
    }

    #[test]
    fn clones_keep_their_identity_and_rebuilds_get_their_own() {
        let image = Image::from_rgba(1, 1, vec![1, 2, 3, 255]).unwrap();
        assert_eq!(image.clone().id(), image.id());

        // The same pixels built again are an equal image with an identity of
        // its own: equality stays by pixels, only the cache key differs.
        let rebuilt = Image::from_rgba(1, 1, vec![1, 2, 3, 255]).unwrap();
        assert_eq!(rebuilt, image);
        assert_ne!(rebuilt.id(), image.id());
    }

    #[test]
    fn encode_round_trips_and_is_deterministic() {
        let image = Image::from_rgba(2, 1, vec![1, 2, 3, 255, 4, 5, 6, 200]).unwrap();
        let bytes = image.encode_png().unwrap();
        assert_eq!(bytes, image.encode_png().unwrap());
        assert_eq!(Image::decode_png(&bytes).unwrap(), image);
    }

    #[test]
    fn save_png_writes_a_file_and_reports_an_unwritable_path() {
        let image = Image::from_rgba(1, 1, vec![9, 8, 7, 255]).unwrap();
        let dir = std::env::temp_dir().join(format!("xui-save-png-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.png");
        image.save_png(&path).unwrap();
        assert_eq!(
            Image::decode_png(&std::fs::read(&path).unwrap()).unwrap(),
            image
        );
        // A path whose parent is a file cannot be created on any platform.
        let bad = path.join("nested.png");
        assert!(matches!(image.save_png(&bad), Err(ImageError::Io(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
