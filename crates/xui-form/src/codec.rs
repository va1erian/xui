#![forbid(unsafe_code)]

//! Reading and writing `.lfm` format 2: [`load`], [`Form::to_ron`] and
//! [`format`].
//!
//! Writing is canonical and byte-stable: one fixed [`PrettyConfig`], fields in
//! declaration order and defaults omitted, so formatting a formatted file
//! changes nothing. Comments are not kept.

use std::fmt;

use ron::extensions::Extensions;
use ron::ser::PrettyConfig;
use ron::{Options, error::SpannedError};

use crate::model::Form;
use crate::suggest::closest;

/// The RON options every form is read and written with: newtype variants
/// unwrapped (`Button(text: "OK")`, not `Button((text: "OK"))`) and options
/// written without `Some(..)`.
pub(crate) fn options() -> Options {
    Options::default()
        .with_default_extension(Extensions::UNWRAP_VARIANT_NEWTYPES | Extensions::IMPLICIT_SOME)
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

/// Parses a form from `.lfm` format-2 text.
pub fn load(text: &str) -> Result<Form, LoadError> {
    if text.trim_start().starts_with("format") {
        return Err(LoadError {
            message: "this is a format-1 (TOML) form; convert it with `xui-form migrate`"
                .to_owned(),
            line: 1,
            column: 1,
            suggestion: None,
        });
    }
    options()
        .from_str(text)
        .map_err(|error| LoadError::from_ron(&error))
}

/// Formats `.lfm` text canonically: [`load`] then [`Form::to_ron`].
pub fn format(text: &str) -> Result<String, LoadError> {
    Ok(load(text)?.to_ron())
}

impl Form {
    /// The form as canonical `.lfm` text, ending with a newline.
    pub fn to_ron(&self) -> String {
        let mut text = options()
            .to_string_pretty(self, pretty())
            .expect("a form always serialises");
        text.push('\n');
        text
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
    fn from_ron(error: &SpannedError) -> LoadError {
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
        LoadError {
            message: error.code.to_string(),
            line: error.span.start.line,
            column: error.span.start.col,
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
