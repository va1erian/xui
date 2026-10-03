#![forbid(unsafe_code)]

//! Saving and exporting documents: the native JSON format and Markdown export.

mod base64;
#[cfg(feature = "serde")]
pub mod json;
pub mod markdown;

use std::fmt;

#[cfg(feature = "serde")]
pub use json::{from_json, to_json};
pub use markdown::{ImageExport, to_markdown};

/// Why a saved document could not be loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    /// The text is not JSON of the expected shape.
    Syntax(String),
    /// The file's `version` is not one this build reads.
    Version(u32),
    /// An embedded image is not valid base64 or not a decodable image.
    Image(String),
    /// The structure breaks a document invariant (spans not covering the
    /// text, an unknown style or object, mismatched anchors, ...).
    Invalid(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Syntax(e) => write!(f, "not a rich-text file: {e}"),
            FormatError::Version(v) => write!(f, "unsupported file version {v}"),
            FormatError::Image(e) => write!(f, "bad embedded image: {e}"),
            FormatError::Invalid(e) => write!(f, "invalid document: {e}"),
        }
    }
}

impl std::error::Error for FormatError {}
