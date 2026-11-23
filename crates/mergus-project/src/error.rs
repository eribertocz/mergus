//! The errors of reading a project.

use std::fmt;

use mergus_ports::PortError;

/// The kind of a name checked by MG-REQ-0305.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NameKind {
    /// A model: the stem of a `.sql` or `.py` file under `models/`.
    Model,
    /// A folder: a directory under `models/`, other than a schema.
    Folder,
    /// A schema: a directory directly under `models/gold/`.
    Schema,
    /// A source: the stem of a `sources/*.toml` file.
    Source,
    /// A seed: the stem of a `seeds/*.csv` file.
    Seed,
}

impl fmt::Display for NameKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Model => "model",
            Self::Folder => "folder",
            Self::Schema => "schema",
            Self::Source => "source",
            Self::Seed => "seed",
        })
    }
}

/// A problem of the project. Paths are relative to the project root, separated by `/`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProjectError {
    /// Two model files have the same stem (MG-REQ-0303).
    #[error("model `{name}` is defined twice: `{first}` and `{second}`")]
    DuplicateModel {
        /// The model name.
        name: String,
        /// The path of the first file, in walking order.
        first: String,
        /// The path of the later file.
        second: String,
    },
    /// A name is not a lowercase identifier (MG-REQ-0305).
    #[error("{kind} name `{name}` of `{path}` is not a lowercase identifier; use `{suggestion}`")]
    InvalidName {
        /// What the name names.
        kind: NameKind,
        /// The name given.
        name: String,
        /// The path of the file or directory that carries the name.
        path: String,
        /// The valid name of MG-REQ-0360.
        suggestion: String,
    },
    /// A configuration file is not TOML 1.0 (MG-REQ-0316).
    #[error("`{path}` line {line} is not TOML 1.0: {message}")]
    InvalidToml {
        /// The path of the file.
        path: String,
        /// The line of the problem, from 1.
        line: usize,
        /// What the parser found.
        message: String,
    },
    /// A key is not in the schema of its file (MG-REQ-0316, MG-REQ-0364).
    #[error("`{path}` line {line}: unknown key `{key}`{}", hint(suggestion.as_deref()))]
    UnknownKey {
        /// The path of the file.
        path: String,
        /// The line of the key, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
        /// The closest key allowed at that place (MG-REQ-0359).
        suggestion: Option<String>,
    },
    /// A value has the wrong type, is invalid, or a required key is missing (MG-REQ-0316).
    #[error("`{path}` line {line}: key `{key}`: {message}{}", hint(suggestion.as_deref()))]
    InvalidValue {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
        /// What the schema expects.
        message: String,
        /// The closest allowed value (MG-REQ-0359).
        suggestion: Option<String>,
    },
    /// A string value contains `${`, `{{` or `{%` (MG-REQ-0364).
    #[error(
        "`{path}` line {line}: key `{key}` holds an expression; configuration values are literals"
    )]
    Expression {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
    },
    /// A string value is a literal secret (MG-REQ-0320, MG-REQ-0363). The value is never shown.
    #[error(
        "`{path}` line {line}: key `{key}` holds a literal secret; write a reference \
         `secret:<name>` instead"
    )]
    LiteralSecret {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
    },
    /// A float is NaN or infinite, which JSON cannot hold (MG-REQ-0365).
    #[error("`{path}` line {line}: key `{key}` is NaN or infinite")]
    NonFiniteFloat {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
    },
    /// A section of a folder `mergus.toml` names no model file of its folder (MG-REQ-0331).
    #[error(
        "`{path}` line {line}: section `{section}` names no model of this folder{}",
        hint(suggestion.as_deref())
    )]
    UnknownSection {
        /// The path of the file.
        path: String,
        /// The line of the section, from 1.
        line: usize,
        /// The section name.
        section: String,
        /// The closest model name of the folder (MG-REQ-0331).
        suggestion: Option<String>,
    },
    /// `ids.toml` is not TOML 1.0, or has a table, key or value that MG-REQ-0370.1 does not
    /// allow.
    #[error("`{path}` line {line}: {}{message}", key_prefix(key.as_deref()))]
    InvalidIds {
        /// The path of the file.
        path: String,
        /// The line of the problem, from 1.
        line: usize,
        /// The key, dotted from the root of the file, when the problem has one.
        key: Option<String>,
        /// What is wrong.
        message: String,
    },
    /// Two entries of `ids.toml` have the same identifier (MG-REQ-0370.3).
    #[error(
        "`{path}` line {line}: identifier `{identifier}` is given to both `{first}` and `{second}`"
    )]
    DuplicateIdsEntry {
        /// The path of the file.
        path: String,
        /// The line of the later entry, from 1.
        line: usize,
        /// The identifier.
        identifier: String,
        /// The dotted key of the earlier entry.
        first: String,
        /// The dotted key of the later entry.
        second: String,
    },
    /// Two objects resolve to the same identifier (MG-REQ-0371).
    #[error(
        "{first} (`{first_path}`) and {second} (`{path}`) have the same identifier `{identifier}`"
    )]
    DuplicateIdentifier {
        /// The identifier.
        identifier: String,
        /// The earlier object.
        first: String,
        /// The path of the file that declares the earlier object.
        first_path: String,
        /// The later object.
        second: String,
        /// The path of the file that declares the later object.
        path: String,
    },
    /// A `.sql` model reads a table the project does not know (MG-REQ-0307, MG-REQ-0373).
    #[error(
        "`{path}` line {line} column {column}: `{name}` is not a model, source table or seed \
         of the project{}",
        hint(suggestion.as_deref())
    )]
    UnknownReference {
        /// The path of the model.
        path: String,
        /// The line of the reference, from 1.
        line: usize,
        /// The column of the reference, from 1, in Unicode scalar values.
        column: usize,
        /// The reference as written, its parts joined by `.`.
        name: String,
        /// The closest known table (MG-REQ-0359).
        suggestion: Option<String>,
    },
    /// A `.sql` model is not one read-only `SELECT` (MG-REQ-0308, MG-REQ-0374).
    #[error(
        "`{path}` line {line} column {column}: {construct} is not allowed; a model is exactly \
         one read-only SELECT"
    )]
    ForbiddenSql {
        /// The path of the model.
        path: String,
        /// The line of the construct, from 1.
        line: usize,
        /// The column of the construct, from 1, in Unicode scalar values.
        column: usize,
        /// The rejected construct.
        construct: String,
    },
    /// The references of `.sql` models form a cycle (MG-REQ-0375).
    #[error(
        "models {} form a cycle: {}",
        cycle_names(models),
        cycle_references(models, references)
    )]
    ModelCycle {
        /// Every model of the cycle in reference order, from the first in name order.
        models: Vec<String>,
        /// For each model of `models`, the path of its file and the line, from 1, of its
        /// reference to the next model of the cycle.
        references: Vec<(String, usize)>,
    },
}

/// Formats the models of a cycle, back to the first.
fn cycle_names(models: &[String]) -> String {
    models
        .iter()
        .chain(models.first())
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// Formats each reference of a cycle with the model it reads.
fn cycle_references(models: &[String], references: &[(String, usize)]) -> String {
    let next = models.iter().skip(1).chain(models.first());
    references
        .iter()
        .zip(next)
        .map(|((path, line), name)| format!("`{path}` line {line} reads `{name}`"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Formats the key of an error, when there is one.
fn key_prefix(key: Option<&str>) -> String {
    key.map_or_else(String::new, |key| format!("key `{key}` "))
}

/// Formats the closest valid name of an error, when there is one.
fn hint(suggestion: Option<&str>) -> String {
    suggestion.map_or_else(String::new, |name| format!("; did you mean `{name}`?"))
}

impl ProjectError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::DuplicateModel { .. } => "MG-0301",
            Self::InvalidName { .. } => "MG-0302",
            Self::InvalidToml { .. } => "MG-0303",
            Self::UnknownKey { .. } => "MG-0304",
            Self::InvalidValue { .. } => "MG-0305",
            Self::Expression { .. } => "MG-0306",
            Self::NonFiniteFloat { .. } => "MG-0307",
            Self::UnknownSection { .. } => "MG-0308",
            Self::LiteralSecret { .. } => "MG-0309",
            Self::InvalidIds { .. } => "MG-0310",
            Self::DuplicateIdsEntry { .. } => "MG-0311",
            Self::DuplicateIdentifier { .. } => "MG-0312",
            Self::UnknownReference { .. } => "MG-0313",
            Self::ForbiddenSql { .. } => "MG-0314",
            Self::ModelCycle { .. } => "MG-0315",
        }
    }

    /// Returns the path that caused the error, relative to the project root.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::DuplicateModel { second, .. } => second,
            Self::InvalidName { path, .. }
            | Self::InvalidToml { path, .. }
            | Self::UnknownKey { path, .. }
            | Self::InvalidValue { path, .. }
            | Self::Expression { path, .. }
            | Self::NonFiniteFloat { path, .. }
            | Self::UnknownSection { path, .. }
            | Self::LiteralSecret { path, .. }
            | Self::InvalidIds { path, .. }
            | Self::DuplicateIdsEntry { path, .. }
            | Self::DuplicateIdentifier { path, .. }
            | Self::UnknownReference { path, .. }
            | Self::ForbiddenSql { path, .. } => path,
            Self::ModelCycle { references, .. } => references.first().map_or("", |(path, _)| path),
        }
    }

    /// Returns the line of the file that caused the error, from 1, when there is one.
    #[must_use]
    pub fn line(&self) -> Option<usize> {
        match self {
            Self::DuplicateModel { .. }
            | Self::InvalidName { .. }
            | Self::DuplicateIdentifier { .. } => None,
            Self::InvalidToml { line, .. }
            | Self::UnknownKey { line, .. }
            | Self::InvalidValue { line, .. }
            | Self::Expression { line, .. }
            | Self::NonFiniteFloat { line, .. }
            | Self::UnknownSection { line, .. }
            | Self::LiteralSecret { line, .. }
            | Self::InvalidIds { line, .. }
            | Self::DuplicateIdsEntry { line, .. }
            | Self::UnknownReference { line, .. }
            | Self::ForbiddenSql { line, .. } => Some(*line),
            Self::ModelCycle { references, .. } => references.first().map(|(_, line)| *line),
        }
    }

    /// Returns the column of the line that caused the error, from 1, when there is one.
    #[must_use]
    pub fn column(&self) -> Option<usize> {
        match self {
            Self::UnknownReference { column, .. } | Self::ForbiddenSql { column, .. } => {
                Some(*column)
            }
            _ => None,
        }
    }

    /// Returns the configuration key that caused the error, dotted from the root of its file.
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::DuplicateModel { .. }
            | Self::InvalidName { .. }
            | Self::InvalidToml { .. }
            | Self::DuplicateIdentifier { .. }
            | Self::UnknownReference { .. }
            | Self::ForbiddenSql { .. }
            | Self::ModelCycle { .. } => None,
            Self::InvalidIds { key, .. } => key.as_deref(),
            Self::UnknownKey { key, .. }
            | Self::InvalidValue { key, .. }
            | Self::Expression { key, .. }
            | Self::NonFiniteFloat { key, .. }
            | Self::UnknownSection { section: key, .. }
            | Self::LiteralSecret { key, .. }
            | Self::DuplicateIdsEntry { second: key, .. } => Some(key),
        }
    }
}

/// Why a project layout could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// The file system port failed.
    #[error(transparent)]
    Port(#[from] PortError),
    /// A bundled configuration schema cannot be compiled: a defect of Mergus itself.
    #[error("configuration schema `{0}` is invalid")]
    Schema(&'static str),
    /// The project has problems, every one of them, in walking order.
    #[error("the project has {} problem(s)", .0.len())]
    Invalid(Vec<ProjectError>),
}
