#![forbid(unsafe_code)]

//! Converting a format-1 form (flat TOML, absolute rectangles) to format 2.
//!
//! Each container's children become an `Absolute` layout, every node at its
//! old rectangle with its old anchor, or a `Grid` when the rectangles line up
//! in rows and columns and none is anchored. There is no format-1 loader at
//! runtime: forms are converted once, with `xui-form migrate`.

use crate::model::{Form, Length};
use crate::{Catalog, Diagnostic};

mod convert;
mod grid;
mod tree;
mod v1;

use convert::Converter;
pub use tree::NodeProblem;
use v1::V1Form;

/// A converted form, with what the conversion could not carry over.
#[derive(Clone, Debug, PartialEq)]
pub struct Migration {
    /// The format-2 form.
    pub form: Form,
    /// Properties dropped or changed on the way, one line each.
    pub notes: Vec<String>,
}

/// A format-1 form that could not be converted.
#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    /// The text is not valid format-1 TOML.
    #[error("not a format-1 form: {0}")]
    Toml(String),
    /// The form breaks a rule of format 1 (a name used twice, a parent that
    /// does not exist or cannot hold nodes, a parent cycle, a value of the
    /// wrong type), or converts to a form that is not valid format 2 (a name
    /// that is not an identifier or is reserved, a range backwards, a
    /// selection past the items). Each problem names its node; nothing is
    /// converted or written, because converting would drop nodes or write
    /// what `check` rejects.
    #[error("the form is not a valid format-1 form: {}", list(.0))]
    Invalid(Vec<NodeProblem>),
    /// A node could not be expressed in format 2.
    #[error("node `{node}`: {message}")]
    Node {
        /// The node's name.
        node: String,
        /// What went wrong.
        message: String,
    },
    /// The converted form could not be written, or the text written did not
    /// load back to the same form. Nothing is reported as converted then.
    #[error("the converted form cannot be written: {0}")]
    Write(String),
}

/// The problems as one line: `node `a`: …; node `b`: …`.
fn list(problems: &[NodeProblem]) -> String {
    let lines: Vec<String> = problems
        .iter()
        .map(|problem| format!("node `{}`: {}", problem.node, problem.message))
        .collect();
    lines.join("; ")
}

/// The `[window]` keys format 2 carries over; any other is noted as dropped.
const WINDOW_KEYS: [&str; 5] = ["name", "title", "width", "height", "resizable"];

/// What is wrong with the `[window]` table.
fn window_problems(window: &toml::Table) -> Vec<NodeProblem> {
    let mut problems = Vec::new();
    let mut problem = |message: String| {
        problems.push(NodeProblem {
            node: "window".to_owned(),
            message,
        });
    };
    for key in ["name", "title"] {
        if window.get(key).is_some_and(|value| !value.is_str()) {
            problem(format!("`{key}` must be a string"));
        }
    }
    for key in ["width", "height"] {
        match window.get(key) {
            None => {}
            Some(value) => match value.as_integer() {
                Some(size) if size >= 1 => {}
                _ => problem(format!("`{key}` must be an integer of at least 1")),
            },
        }
    }
    if window
        .get("resizable")
        .is_some_and(|value| !value.is_bool())
    {
        problem("`resizable` must be true or false".to_owned());
    }
    problems
}

/// Converts format-1 `text` to a format-2 form.
///
/// A form that breaks a rule of format 1 (see [`MigrateError::Invalid`]) is
/// refused whole, so no node is dropped without being reported: what the
/// conversion leaves out of the form is listed in [`Migration::notes`].
pub fn from_v1(text: &str) -> Result<Migration, MigrateError> {
    let v1: V1Form = toml::from_str(text).map_err(|error| MigrateError::Toml(error.to_string()))?;
    if v1.format != 1 {
        return Err(MigrateError::Toml(format!("format {} is not 1", v1.format)));
    }
    let mut problems = window_problems(&v1.window);
    problems.extend(tree::check(&v1.nodes));
    if !problems.is_empty() {
        return Err(MigrateError::Invalid(problems));
    }
    let int = |key: &str, default: i64| {
        v1.window
            .get(key)
            .and_then(toml::Value::as_integer)
            .unwrap_or(default)
    };
    let text = |key: &str| {
        v1.window
            .get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let size = (int("width", 320), int("height", 200));
    let mut converter = Converter::new(&v1.nodes);
    for key in v1
        .window
        .keys()
        .filter(|key| !WINDOW_KEYS.contains(&key.as_str()))
    {
        converter.notes.push(format!(
            "`window`: dropped `{key}`, which format 2 has no use for"
        ));
    }
    for key in v1.other.keys() {
        converter.notes.push(format!(
            "dropped the top-level `{key}`: format 1 ignored it too"
        ));
    }
    let root = converter.level(None, size, 1, false)?;
    let form = Form {
        name: text("name"),
        title: text("title"),
        size: (Length(size.0 as f32), Length(size.1 as f32)),
        resizable: v1
            .window
            .get("resizable")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true),
        root,
    };
    verify(&form)?;
    let mut notes = converter.notes;
    notes.extend(check_converted(&form)?);
    Ok(Migration { form, notes })
}

/// Validates the converted form like `xui-form check` would. A form that
/// is not valid format 2 (a name that is not an identifier, one reserved for
/// scripts, a range backwards) is refused as the violations of format 1 are,
/// instead of being written for `check` to reject afterwards; its warnings
/// are returned as notes.
fn check_converted(form: &Form) -> Result<Vec<String>, MigrateError> {
    let (errors, warnings): (Vec<Diagnostic>, Vec<Diagnostic>) = form
        .validate(&Catalog::xui())
        .into_iter()
        .partition(Diagnostic::is_error);
    if !errors.is_empty() {
        let problems = errors
            .into_iter()
            .map(|diagnostic| NodeProblem {
                node: diagnostic.node.unwrap_or_else(|| "window".to_owned()),
                message: diagnostic.message,
            })
            .collect();
        return Err(MigrateError::Invalid(problems));
    }
    Ok(warnings
        .into_iter()
        .map(|diagnostic| match diagnostic.node {
            Some(node) => format!("`{node}`: {}", diagnostic.message),
            None => diagnostic.message,
        })
        .collect())
}

/// Writes `form` and reads the text back: a conversion is only reported as
/// done when what it produced is a file that loads, to the same form.
fn verify(form: &Form) -> Result<(), MigrateError> {
    let text = form
        .to_ron()
        .map_err(|error| MigrateError::Write(error.to_string()))?;
    match crate::load(&text) {
        Ok(loaded) if loaded == *form => Ok(()),
        Ok(_) => Err(MigrateError::Write(
            "the text written reads back as a different form".to_owned(),
        )),
        Err(error) => Err(MigrateError::Write(format!(
            "the text written does not load: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests;
