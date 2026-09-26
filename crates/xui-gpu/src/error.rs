#![forbid(unsafe_code)]

//! [`GlError`]: why an OpenGL context or frame failed.

/// Why an OpenGL context or frame failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlError(String);

impl GlError {
    /// The error as text.
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl From<String> for GlError {
    fn from(message: String) -> GlError {
        GlError(message)
    }
}

impl std::fmt::Display for GlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GlError {}
