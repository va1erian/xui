#![forbid(unsafe_code)]

//! Reading and writing `.lfm` format 2: [`load`], [`Form::to_ron`] and
//! [`format`].
//!
//! Writing is canonical and byte-stable: one fixed [`PrettyConfig`], fields in
//! declaration order and defaults omitted, so formatting a formatted file
//! changes nothing. Comments are not kept.
//!
//! Reading and writing share one limit ([`MAX_DEPTH`]), so a form that
//! [`Form::to_ron`] writes always [`load`]s again, and one that cannot be
//! read back is not written: `to_ron` reports it.

use std::fmt;

use ron::extensions::Extensions;
use ron::ser::PrettyConfig;
use ron::{Options, error::SpannedError};

use crate::model::{Form, MAX_DEPTH, RECURSION_LIMIT, depth_message};
use crate::suggest::closest;

/// The RON options every form is read and written with: newtype variants
/// unwrapped (`Button(text: "OK")`, not `Button((text: "OK"))`), options
/// written without `Some(..)`, and a recursion limit that only [`MAX_DEPTH`]
/// is stricter than (the reader and the writer count RON levels differently,
/// so RON's own default is not the form's limit).
pub(crate) fn options() -> Options {
    Options::default()
        .with_default_extension(Extensions::UNWRAP_VARIANT_NEWTYPES | Extensions::IMPLICIT_SOME)
        .with_recursion_limit(RECURSION_LIMIT)
}

/// The fixed pretty-printing configuration of a saved form: each node on one
/// line, a node's children one per line below it.
fn pretty() -> PrettyConfig {
    PrettyConfig::new()
        .struct_names(true)
        .compact_structs(true)
        .indentor("    ")
        .new_line("\n")
}

/// Parses a form from `.lfm` format-2 text. A byte order mark at the start is
/// ignored, and positions in an error do not count it.
pub fn load(text: &str) -> Result<Form, LoadError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if let Some((line, column)) = format_one(text) {
        return Err(LoadError {
            message: "this is a format-1 (TOML) form; convert it with `xui-form migrate`"
                .to_owned(),
            line,
            column,
            suggestion: None,
        });
    }
    options()
        .from_str(text)
        .map_err(|error| LoadError::from_ron(&error, text))
}

/// Where `text` starts like a format-1 (TOML) form: its first line that is not
/// blank or a `#` comment is `format = 1`, or opens the `[window]` table or a
/// `[[node]]`. A format-2 file starts with `Form(`, a comment or a `#![..]`
/// attribute, none of which a TOML file does.
fn format_one(text: &str) -> Option<(usize, usize)> {
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.starts_with("#!")) {
            continue;
        }
        let key = trimmed
            .strip_prefix("format")
            .is_some_and(|rest| rest.starts_with(|c: char| c == '=' || c.is_whitespace()));
        let table = trimmed.starts_with("[window]") || trimmed.starts_with("[[node]]");
        return (key || table).then(|| (index + 1, line.len() - trimmed.len() + 1));
    }
    None
}

/// Formats `.lfm` text canonically: [`load`] then [`Form::to_ron`].
pub fn format(text: &str) -> Result<String, LoadError> {
    // A form that loaded is within every limit, so it writes; were it not,
    // the failure is reported like any other.
    load(text)?.to_ron().map_err(|error| LoadError {
        message: error.to_string(),
        line: 1,
        column: 1,
        suggestion: None,
    })
}

/// A form that cannot be written as `.lfm` text, because [`load`] would not
/// read the text back.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WriteError {
    /// The tree nests deeper than [`MAX_DEPTH`].
    #[error("the form nests {depth} levels deep; at most {MAX_DEPTH} are allowed")]
    TooDeep {
        /// How deep the tree nests.
        depth: usize,
    },
    /// A value cannot be written as a valid `.lfm` value, such as a length
    /// that is not finite.
    #[error("the form cannot be written: {0}")]
    Unwritable(String),
}

impl Form {
    /// The form as canonical `.lfm` text, ending with a newline.
    ///
    /// A text that is written is a text [`load`] reads back to an equal form.
    /// A form that could not be read back (nested deeper than [`MAX_DEPTH`])
    /// is an error, never a panic and never a file that does not load.
    pub fn to_ron(&self) -> Result<String, WriteError> {
        let depth = self.root.depth();
        if depth > MAX_DEPTH {
            return Err(WriteError::TooDeep { depth });
        }
        let mut text = options()
            .to_string_pretty(self, pretty())
            .map_err(|error| WriteError::Unwritable(error.to_string()))?;
        text.push('\n');
        Ok(text)
    }
}

/// `value` as one line of RON, as the schema shows a field's default.
pub(crate) fn to_ron_compact(value: &impl serde::Serialize) -> String {
    options()
        .to_string(value)
        .expect("a default always serialises")
}

/// A form that could not be read, with where and, for a misspelt name, what
/// was probably meant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadError {
    /// A description of the problem.
    pub message: String,
    /// The one-based line.
    pub line: usize,
    /// The one-based column.
    pub column: usize,
    /// For an unknown field or kind, the closest known one.
    pub suggestion: Option<String>,
}

impl LoadError {
    fn from_ron(error: &SpannedError, text: &str) -> LoadError {
        use ron::error::Error;
        let suggestion = match &error.code {
            Error::NoSuchStructField {
                expected, found, ..
            }
            | Error::NoSuchEnumVariant {
                expected, found, ..
            } => closest(found, expected.iter().copied()).map(str::to_owned),
            _ => None,
        };
        let (mut line, mut column) = (error.span.start.line, error.span.start.col);
        if matches!(&error.code, Error::Message(message) if *message == depth_message()) {
            // The reader stops before the node that is one level too deep.
            (line, column) = crate::locate::next_token(text, line, column);
        } else if matches!(&error.code, Error::IntegerOutOfBounds) {
            // RON reports the digits of `-1`; the number starts at its sign.
            (line, column) = crate::locate::sign_start(text, line, column);
        }
        LoadError {
            message: error.code.to_string(),
            line,
            column,
            suggestion,
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}: {}", self.line, self.column, self.message)?;
        if let Some(suggestion) = &self.suggestion {
            write!(formatter, " (did you mean `{suggestion}`?)")?;
        }
        Ok(())
    }
}

impl std::error::Error for LoadError {}
